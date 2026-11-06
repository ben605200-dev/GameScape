//! Spike S1 (+S2-lite) — Phase 0: re-parent WebView2 windows into the Windows
//! desktop layer so they render *behind desktop icons*, one window per monitor.
//! Mechanically verifies parentage (child-chain walk) and logs per-monitor
//! results.
//!
//! TEMPORARY — discovery code for the desktop-host gates (ADR-0001, Phase 0
//! spikes S1–S3). Replaced by the real `desktop_host` subsystem once verified.
//!
//! Activated when `GAMESCAPE_SPIKE_S1=1`:
//!   1. Creates a borderless, taskbar-less WebView2 window per monitor
//!      (all showing `spike.html`).
//!   2. Detects the desktop topology (classic vs raised desktop).
//!   3. Locates the shell's wallpaper layer (WorkerW / Progman).
//!   4. Re-parents every window into it at the correct monitor rectangle,
//!      translated into the layer's client coordinates (handles monitor
//!      arrangements with negative/offset origins).
//!   5. Applies `WS_EX_LAYERED` (required for WebView2 compositing behind
//!      icons on raised desktops) and proves parentage by walking the
//!      parent's actual child chain.
//!   6. Appends a step log to the file named by `GAMESCAPE_SPIKE_OUT`,
//!      ending in `ATTACHED_OK` or `ATTACH_FAILED: reason`.
//!
//! Interactive prototype: set `GAMESCAPE_SPIKE_INTERACTIVE=1` to serve
//! `spike-interactive.html` and additionally stream the global cursor
//! position into every surface — the cursor-reactive wallpaper technique
//! (passive: no clicks, no focus, no interference).
//!
//! Known wrapper quirks (verified against windows 0.62.2 source):
//!   - `SetParent` reports `Err` whenever the window's previous parent was
//!     NULL (the very first attach succeeds but looks like an error).
//!   - `GetWindow` reports `Err` for invalid handles (so the chain walk ends
//!     on Err).
//! Both are handled explicitly below.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

use windows::core::{BOOL, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, POINT, RECT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, FindWindowExW, FindWindowW, GetCursorPos, GetWindow, GetWindowLongPtrW,
    GetWindowRect, SendMessageTimeoutW, SetLayeredWindowAttributes, SetParent, SetWindowLongPtrW,
    SetWindowPos, ShowWindow, GWL_EXSTYLE, GWL_STYLE, GW_CHILD, GW_HWNDNEXT, LWA_ALPHA,
    SMTO_NORMAL, SWP_NOACTIVATE, SWP_SHOWWINDOW, SW_SHOWNOACTIVATE, WS_CHILD, WS_EX_LAYERED,
    WS_EX_NOREDIRECTIONBITMAP, WS_POPUP,
};

const SPIKE_WINDOW_PREFIX: &str = "spike-s1";

/// Undocumented shell message that (re)creates the wallpaper WorkerW layer.
/// Both parameter sets are used in the wild; raised desktop builds need 0xD/0x1.
const WM_SPAWN_WORKER: u32 = 0x052C;

/// Raw window handle + physical monitor rectangle (x, y, width, height).
#[derive(Clone, Copy)]
struct HostSpec {
    hwnd_raw: usize,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
}

pub fn run(app: AppHandle) {
    // Reduce desktop disruption while the spike runs.
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.hide();
    }

    let interactive = std::env::var("GAMESCAPE_SPIKE_INTERACTIVE")
        .map(|value| value == "1")
        .unwrap_or(false);
    emit(&format!(
        "spike starting (mode={})",
        if interactive { "interactive" } else { "attach" }
    ));

    let monitors = collect_monitors(&app);
    emit(&format!("{} monitor(s) detected", monitors.len()));

    let mut specs: Vec<HostSpec> = Vec::new();
    let mut bridge_targets: Vec<(String, i32, i32, u32, u32)> = Vec::new();
    let mut handles = Vec::new();
    let page = if interactive {
        "spike-interactive.html"
    } else {
        "spike.html"
    };

    for (index, (x, y, w, h)) in monitors.iter().enumerate() {
        let label = format!("{SPIKE_WINDOW_PREFIX}-{index}");
        match WebviewWindowBuilder::new(&app, label.as_str(), WebviewUrl::App(page.into()))
            .title("GameScape Spike S1")
            .decorations(false)
            .resizable(false)
            .skip_taskbar(true)
            .focused(false)
            .visible(false)
            .build()
        {
            Ok(window) => match window.hwnd() {
                Ok(hwnd) => {
                    let hwnd_raw = hwnd.0 as usize;
                    emit(&format!(
                        "window[{index}] created (hwnd=0x{hwnd_raw:X}, rect={x},{y} {w}x{h})"
                    ));
                    specs.push(HostSpec {
                        hwnd_raw,
                        x: *x,
                        y: *y,
                        w: *w,
                        h: *h,
                    });
                    bridge_targets.push((label.clone(), *x, *y, *w, *h));
                    handles.push(window);
                }
                Err(error) => emit(&format!("window[{index}] hwnd unavailable: {error}")),
            },
            Err(error) => emit(&format!("window[{index}] creation failed: {error}")),
        }
    }

    if specs.is_empty() {
        emit("ATTACH_FAILED: no windows could be created");
        return;
    }

    // Keep the window handles alive for the process lifetime; the harness
    // terminates this process once evidence is captured.
    for window in handles {
        std::mem::forget(window);
    }

    let app_for_thread = app.clone();
    thread::spawn(move || {
        // Give the dev-server pages a moment to load. The real subsystem will
        // hook a page-load event instead of sleeping.
        thread::sleep(Duration::from_secs(4));
        match attach_all(&specs) {
            Ok(()) => {
                emit("ATTACHED_OK");
                if interactive {
                    thread::spawn(move || {
                        cursor_bridge(app_for_thread, bridge_targets);
                    });
                }
            }
            Err(error) => {
                for spec in &specs {
                    let hwnd = HWND(spec.hwnd_raw as *mut core::ffi::c_void);
                    unsafe { let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE); };
                }
                emit(&format!("ATTACH_FAILED: {error}"));
            }
        }
    });
}

fn collect_monitors(app: &AppHandle) -> Vec<(i32, i32, u32, u32)> {
    if let Ok(list) = app.available_monitors() {
        let collected: Vec<(i32, i32, u32, u32)> = list
            .iter()
            .map(|monitor| {
                let position = monitor.position();
                let size = monitor.size();
                (position.x, position.y, size.width, size.height)
            })
            .collect();
        if !collected.is_empty() {
            return collected;
        }
    }
    if let Ok(Some(monitor)) = app.primary_monitor() {
        let position = monitor.position();
        let size = monitor.size();
        return vec![(position.x, position.y, size.width, size.height)];
    }
    emit("monitor query unavailable; assuming single 1920x1080");
    vec![(0, 0, 1920, 1080)]
}

fn attach_all(specs: &[HostSpec]) -> Result<(), String> {
    // --- Shell discovery (once for all windows) ---
    let progman = unsafe { find_window("Progman") }.ok_or("Progman not found")?;
    let ex_style = unsafe { GetWindowLongPtrW(progman, GWL_EXSTYLE) } as u32;
    let raised = ex_style & WS_EX_NOREDIRECTIONBITMAP.0 != 0;
    emit(&format!(
        "Progman=0x{:X}; topology={}",
        progman.0 as usize,
        if raised { "raised (Win11 24H2+)" } else { "classic" }
    ));

    // Ask the shell to (re)create its wallpaper WorkerW layer.
    for (wparam, lparam) in [(0usize, 0isize), (0xDusize, 1isize)] {
        let mut result: usize = 0;
        let result_ptr: *mut usize = &mut result;
        let _ = unsafe {
            SendMessageTimeoutW(
                progman,
                WM_SPAWN_WORKER,
                WPARAM(wparam),
                LPARAM(lparam),
                SMTO_NORMAL,
                1000,
                Some(result_ptr),
            )
        };
    }

    // Find the window hosting the desktop icons (SHELLDLL_DefView).
    let mut target = EnumTarget {
        defview_host: None,
        workerw_after: None,
    };
    let target_ptr: *mut EnumTarget = &mut target;
    let _ = unsafe { EnumWindows(Some(enum_windows_proc), LPARAM(target_ptr as isize)) };

    let defview_host = target
        .defview_host
        .ok_or("SHELLDLL_DefView host not found")?;
    let (parent, insert_after) = if raised {
        // Raised topology: parent to Progman and slot directly below the icon
        // layer (the technique current WebView2 wallpaper tools use on 24H2+).
        (progman, Some(defview_host))
    } else {
        let workerw = target
            .workerw_after
            .ok_or("WorkerW wallpaper layer not found")?;
        (workerw, None)
    };

    // The layer's screen origin — needed to translate monitor rectangles into
    // its client space (a monitor left of / above primary yields negative
    // coordinates, so this cannot be assumed to be (0,0)).
    let mut parent_rect = RECT::default();
    unsafe { GetWindowRect(parent, &mut parent_rect) }
        .map_err(|error| format!("GetWindowRect failed: {error}"))?;
    emit(&format!(
        "attaching {} window(s): parent=0x{:X} origin=({}, {}) raised={raised}",
        specs.len(),
        parent.0 as usize,
        parent_rect.left,
        parent_rect.top
    ));

    let mut all_ok = true;
    for (index, spec) in specs.iter().enumerate() {
        let hwnd = HWND(spec.hwnd_raw as *mut core::ffi::c_void);

        // Child styles: never combine WS_POPUP and WS_CHILD.
        let style = unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) };
        let new_style = (style & !(WS_POPUP.0 as isize)) | (WS_CHILD.0 as isize);
        unsafe { SetWindowLongPtrW(hwnd, GWL_STYLE, new_style) };

        // Layered + full alpha: required for the WebView2 (DirectComposition)
        // surface to composite behind the icons instead of showing black.
        let ex = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) };
        let new_ex = ex | (WS_EX_LAYERED.0 as isize);
        unsafe { SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_ex) };
        let _ = unsafe { SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA) };

        // Wrapper quirk (see module docs): ignore the Result; verify by effect.
        let _ = unsafe { SetParent(hwnd, Some(parent)) };

        let child_x = spec.x - parent_rect.left;
        let child_y = spec.y - parent_rect.top;
        let _ = unsafe {
            SetWindowPos(
                hwnd,
                insert_after,
                child_x,
                child_y,
                spec.w as i32,
                spec.h as i32,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            )
        };
        unsafe { let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE); };

        // Mechanical proof of parentage: pixels can be obscured; this cannot.
        let found = child_chain_contains(parent, hwnd);
        emit(&format!(
            "window[{index}] rect=({child_x},{child_y} {}x{}) child={found} layered={}",
            spec.w,
            spec.h,
            new_ex & WS_EX_LAYERED.0 as isize != 0
        ));
        all_ok &= found;
    }

    if all_ok {
        Ok(())
    } else {
        Err("not all windows present in the parent's child chain".to_string())
    }
}

fn child_chain_contains(parent: HWND, needle: HWND) -> bool {
    let mut walked = 0usize;
    let mut cursor = unsafe { GetWindow(parent, GW_CHILD) }.ok();
    while let Some(current) = cursor {
        if current == needle {
            return true;
        }
        walked += 1;
        if walked > 4096 {
            break;
        }
        cursor = unsafe { GetWindow(current, GW_HWNDNEXT) }.ok();
    }
    false
}

/// Streams the global cursor position into every attached wallpaper surface.
///
/// Wallpapers never receive real input (a documented property of the desktop
/// layer) — but they don't need it: the core reads the cursor globally via
/// `GetCursorPos` and pushes per-monitor local coordinates into each page,
/// which renders a passive light field that follows the pointer. No clicks,
/// no focus stealing, no interference with whatever the user is doing.
fn cursor_bridge(app: AppHandle, monitors: Vec<(String, i32, i32, u32, u32)>) {
    emit(&format!("cursor bridge started ({} surface(s))", monitors.len()));
    let mut last = POINT {
        x: i32::MIN,
        y: i32::MIN,
    };

    loop {
        let mut point = POINT { x: 0, y: 0 };
        let ok = unsafe { GetCursorPos(&mut point) }.is_ok();

        if ok && (point.x != last.x || point.y != last.y) {
            last = point;
            for (label, origin_x, origin_y, width, height) in &monitors {
                let local_x = point.x - origin_x;
                let local_y = point.y - origin_y;
                if local_x >= 0
                    && local_y >= 0
                    && local_x < *width as i32
                    && local_y < *height as i32
                {
                    if let Some(window) = app.get_webview_window(label) {
                        let script = format!(
                            "window.__gsCursor && window.__gsCursor({local_x}, {local_y});"
                        );
                        let _ = window.eval(&script);
                    }
                }
            }
        }

        thread::sleep(Duration::from_millis(16));
    }
}

struct EnumTarget {
    defview_host: Option<HWND>,
    workerw_after: Option<HWND>,
}

unsafe extern "system" fn enum_windows_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let target = unsafe { &mut *(lparam.0 as *mut EnumTarget) };
    if target.defview_host.is_some() {
        return BOOL(1);
    }
    if unsafe { find_child(hwnd, None, "SHELLDLL_DefView") }.is_some() {
        target.defview_host = Some(hwnd);
        target.workerw_after = unsafe { find_top_level_after(hwnd, "WorkerW") };
    }
    BOOL(1)
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe fn find_window(class: &str) -> Option<HWND> {
    let class_w = wide(class);
    unsafe { FindWindowW(PCWSTR(class_w.as_ptr()), PCWSTR(std::ptr::null())) }.ok()
}

unsafe fn find_child(parent: HWND, after: Option<HWND>, class: &str) -> Option<HWND> {
    let class_w = wide(class);
    unsafe {
        FindWindowExW(
            Some(parent),
            after,
            PCWSTR(class_w.as_ptr()),
            PCWSTR(std::ptr::null()),
        )
    }
    .ok()
}

unsafe fn find_top_level_after(after: HWND, class: &str) -> Option<HWND> {
    let class_w = wide(class);
    unsafe {
        FindWindowExW(
            None,
            Some(after),
            PCWSTR(class_w.as_ptr()),
            PCWSTR(std::ptr::null()),
        )
    }
    .ok()
}

fn log_path() -> Option<PathBuf> {
    std::env::var("GAMESCAPE_SPIKE_OUT").ok().map(PathBuf::from)
}

fn emit(msg: &str) {
    println!("[spike-s1] {msg}");
    if let Some(path) = log_path() {
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(file, "{msg}");
        }
    }
}
