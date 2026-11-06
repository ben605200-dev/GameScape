//! Desktop host: applies wallpapers to the actual Windows desktop.
//!
//! Two mechanisms:
//!  - **Static images** go through the documented `IDesktopWallpaper` COM API
//!    (per-monitor paths, position modes, previous-wallpaper restore).
//!  - **Animated media (GIF/MP4)** render in per-monitor WebView windows
//!    re-parented into the shell's desktop layer (technique validated in the
//!    Phase 0 spikes S1/S2 — classic and raised topologies).
//!
//! Self-healing: a layer monitor watches the shell parent handle; explorer
//! restarts destroy it and every surface re-attaches automatically. The
//! spike learned that `GetParent` is unreliable on raised desktops, so we
//! monitor the parent's existence (`IsWindow`) instead.

use std::collections::HashMap;
use std::fs;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Monitor, WebviewUrl, WebviewWindowBuilder};

use windows::core::PCWSTR;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, RECT, WPARAM};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::Shell::{DesktopWallpaper, IDesktopWallpaper, DWPOS_FILL};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, FindWindowExW, FindWindowW, GetWindow, GetWindowLongPtrW, GetWindowRect,
    IsWindow, SendMessageTimeoutW, SetLayeredWindowAttributes, SetParent, SetWindowLongPtrW,
    SetWindowPos, ShowWindow, GWL_EXSTYLE, GWL_STYLE, GW_CHILD, GW_HWNDNEXT, LWA_ALPHA,
    SMTO_NORMAL, SWP_NOACTIVATE, SWP_SHOWWINDOW, SW_SHOWNOACTIVATE, WS_CHILD, WS_EX_LAYERED,
    WS_EX_NOREDIRECTIONBITMAP, WS_POPUP,
};

use crate::storage::{self, AppState};

/// Undocumented shell message that (re)creates the wallpaper WorkerW layer.
const WM_SPAWN_WORKER: u32 = 0x052C;

/* ------------------------------------------------------------------ */
/* State                                                               */
/* ------------------------------------------------------------------ */

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostSurface {
    pub monitor_index: u32,
    pub kind: String,
    pub title: String,
    pub paused: bool,
}

struct Surface {
    monitor_index: u32,
    label: String,
    kind: String,
    title: String,
    media_path: String,
    paused: bool,
}

pub struct HostState {
    inner: Mutex<HostInner>,
}

struct HostInner {
    surfaces: HashMap<u32, Surface>,
    /// Raw handle value (HWND is not Send, so it is stored as isize).
    parent: Option<isize>,
}

impl HostState {
    pub fn new() -> Self {
        HostState {
            inner: Mutex::new(HostInner {
                surfaces: HashMap::new(),
                parent: None,
            }),
        }
    }
}

/* ------------------------------------------------------------------ */
/* Logging (visible to the app console and local harnesses)            */
/* ------------------------------------------------------------------ */

pub fn log_line(message: &str) {
    println!("[host] {message}");
    if let Ok(path) = std::env::var("GAMESCAPE_SPIKE_OUT") {
        use std::io::Write as _;
        if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(file, "{message}");
        }
    }
}

/* ------------------------------------------------------------------ */
/* Shell layer discovery (classic + raised topologies)                 */
/* ------------------------------------------------------------------ */

struct LayerHost {
    parent: HWND,
    defview: Option<HWND>,
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn hwnd_from_raw(raw: isize) -> HWND {
    HWND(raw as *mut core::ffi::c_void)
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

struct EnumTarget {
    defview_host: Option<HWND>,
    workerw_after: Option<HWND>,
}

unsafe extern "system" fn enum_windows_proc(hwnd: HWND, lparam: LPARAM) -> windows::core::BOOL {
    let target = unsafe { &mut *(lparam.0 as *mut EnumTarget) };
    if target.defview_host.is_some() {
        return windows::core::BOOL(1);
    }
    if unsafe { find_child(hwnd, None, "SHELLDLL_DefView") }.is_some() {
        target.defview_host = Some(hwnd);
        target.workerw_after = unsafe { find_top_level_after(hwnd, "WorkerW") };
    }
    windows::core::BOOL(1)
}

fn discover_layer() -> Result<LayerHost, String> {
    let progman = unsafe { find_window("Progman") }.ok_or("Progman not found")?;
    let ex_style = unsafe { GetWindowLongPtrW(progman, GWL_EXSTYLE) } as u32;
    let raised = ex_style & WS_EX_NOREDIRECTIONBITMAP.0 != 0;

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

    let mut target = EnumTarget {
        defview_host: None,
        workerw_after: None,
    };
    let target_ptr: *mut EnumTarget = &mut target;
    let _ = unsafe { EnumWindows(Some(enum_windows_proc), LPARAM(target_ptr as isize)) };

    let defview_host = target.defview_host.ok_or("SHELLDLL_DefView host not found")?;
    if raised {
        log_line("desktop topology: raised (Win11 24H2+)");
        Ok(LayerHost {
            parent: progman,
            defview: Some(defview_host),
        })
    } else {
        let workerw = target
            .workerw_after
            .ok_or("WorkerW wallpaper layer not found")?;
        log_line("desktop topology: classic");
        Ok(LayerHost {
            parent: workerw,
            defview: None,
        })
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

fn attach_surface(hwnd: HWND, layer: &LayerHost, rect: (i32, i32, u32, u32)) -> Result<bool, String> {
    let mut parent_rect = RECT::default();
    unsafe { GetWindowRect(layer.parent, &mut parent_rect) }
        .map_err(|error| format!("GetWindowRect failed: {error}"))?;

    let style = unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) };
    let new_style = (style & !(WS_POPUP.0 as isize)) | (WS_CHILD.0 as isize);
    unsafe { SetWindowLongPtrW(hwnd, GWL_STYLE, new_style) };

    let ex = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) };
    let new_ex = ex | (WS_EX_LAYERED.0 as isize);
    unsafe { SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_ex) };
    let _ = unsafe { SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA) };

    let _ = unsafe { SetParent(hwnd, Some(layer.parent)) };

    let (mx, my, mw, mh) = rect;
    let child_x = mx - parent_rect.left;
    let child_y = my - parent_rect.top;
    let _ = unsafe {
        SetWindowPos(
            hwnd,
            layer.defview,
            child_x,
            child_y,
            mw as i32,
            mh as i32,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        )
    };
    unsafe { let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE); };

    Ok(child_chain_contains(layer.parent, hwnd))
}

/* ------------------------------------------------------------------ */
/* Static wallpapers (IDesktopWallpaper)                               */
/* ------------------------------------------------------------------ */

fn with_desktop_wallpaper<T>(
    action: impl FnOnce(&IDesktopWallpaper) -> Result<T, String>,
) -> Result<T, String> {
    unsafe {
        let hresult = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let outcome = (|| {
            let desktop: IDesktopWallpaper = CoCreateInstance(&DesktopWallpaper, None, CLSCTX_ALL)
                .map_err(|error| format!("cannot reach the Windows wallpaper API: {error}"))?;
            action(&desktop)
        })();
        if hresult.is_ok() {
            CoUninitialize();
        }
        outcome
    }
}

/// Convert formats Windows cannot set (WEBP) into a cached PNG.
fn ensure_static_ready(path: &str, format: Option<&str>) -> Result<String, String> {
    let needs_conversion = matches!(
        format.map(|value| value.to_ascii_lowercase()).as_deref(),
        Some("webp")
    );
    if !needs_conversion {
        return Ok(path.to_string());
    }

    let bytes = fs::read(path).map_err(|error| format!("cannot read the wallpaper: {error}"))?;
    let hash = crate::library::sha256_hex(&bytes);
    let applied_dir = storage::cache_dir()?.join("applied");
    fs::create_dir_all(&applied_dir).map_err(|e| format!("cannot create the cache: {e}"))?;
    let target = applied_dir.join(format!("apply-{}.png", &hash[..16]));
    if !target.exists() {
        let decoded =
            image::open(path).map_err(|_| "the image could not be decoded".to_string())?;
        decoded
            .save_with_format(&target, image::ImageFormat::Png)
            .map_err(|error| format!("cannot convert the wallpaper: {error}"))?;
    }
    Ok(target.to_string_lossy().to_string())
}

/// Apply a static image to every monitor (Fill). The wallpaper in place
/// before the first apply is remembered per monitor for restore.
pub fn apply_static(state: &AppState, library_path: &str, format: Option<&str>) -> Result<String, String> {
    let ready_path = ensure_static_ready(library_path, format)?;
    let ready_wide = wide(&ready_path);

    let previous = with_desktop_wallpaper(|desktop| unsafe {
        let count = desktop
            .GetMonitorDevicePathCount()
            .map_err(|error| format!("cannot list monitors: {error}"))?;
        let _ = desktop.SetPosition(DWPOS_FILL);

        let mut previous: Vec<(String, String)> = Vec::new();
        for index in 0..count {
            let monitor_id = desktop
                .GetMonitorDevicePathAt(index)
                .map_err(|error| format!("cannot read monitor {index}: {error}"))?;
            let id_string = monitor_id
                .to_string()
                .map_err(|error| format!("cannot read a monitor id: {error}"))?;
            let id_wide = wide(&id_string);

            let prior = desktop
                .GetWallpaper(PCWSTR(id_wide.as_ptr()))
                .ok()
                .and_then(|value| value.to_string().ok())
                .filter(|value| !value.is_empty());
            if let Some(prior) = prior {
                previous.push((id_string.clone(), prior));
            }

            desktop
                .SetWallpaper(PCWSTR(id_wide.as_ptr()), PCWSTR(ready_wide.as_ptr()))
                .map_err(|error| format!("cannot set the wallpaper on monitor {index}: {error}"))?;
        }
        Ok(previous)
    })?;

    if let Ok(conn) = state.db.lock() {
        for (monitor_id, prior) in &previous {
            let known: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM wallpaper_state WHERE monitor_id = ?1",
                    [monitor_id],
                    |row| row.get(0),
                )
                .unwrap_or(0);
            if known == 0 {
                let _ = conn.execute(
                    "INSERT INTO wallpaper_state (monitor_id, previous_path, applied_path)
                     VALUES (?1, ?2, ?3)",
                    rusqlite::params![monitor_id, prior, ready_path],
                );
            } else {
                let _ = conn.execute(
                    "UPDATE wallpaper_state SET applied_path = ?1, applied_at = datetime('now')
                     WHERE monitor_id = ?2",
                    rusqlite::params![ready_path, monitor_id],
                );
            }
        }
    }
    Ok(ready_path)
}

/// Restore the wallpapers that were in place before GameScape applied any.
pub fn restore_previous(state: &AppState) -> Result<(), String> {
    let rows: Vec<(String, Option<String>)> = {
        let conn = state
            .db
            .lock()
            .map_err(|e| format!("database lock poisoned: {e}"))?;
        let mut statement = conn
            .prepare("SELECT monitor_id, previous_path FROM wallpaper_state")
            .map_err(|error| format!("cannot read wallpaper state: {error}"))?;
        let mapped = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                ))
            })
            .map_err(|error| format!("cannot read wallpaper state: {error}"))?;
        let mut rows = Vec::new();
        for row in mapped {
            rows.push(row.map_err(|error| format!("cannot read wallpaper state: {error}"))?);
        }
        rows
    };

    if rows.is_empty() {
        return Err("no previous wallpaper has been recorded yet".to_string());
    }

    with_desktop_wallpaper(|desktop| unsafe {
        for (monitor_id, previous) in &rows {
            let Some(previous) = previous else { continue };
            if !std::path::Path::new(previous).exists() {
                continue;
            }
            let id_wide = wide(monitor_id);
            let path_wide = wide(previous);
            desktop
                .SetWallpaper(PCWSTR(id_wide.as_ptr()), PCWSTR(path_wide.as_ptr()))
                .map_err(|error| format!("cannot restore monitor {monitor_id}: {error}"))?;
        }
        Ok(())
    })?;

    if let Ok(conn) = state.db.lock() {
        let _ = conn.execute("UPDATE wallpaper_state SET applied_path = NULL", []);
    }
    log_line("previous wallpaper restored");
    Ok(())
}

/* ------------------------------------------------------------------ */
/* Animated surfaces                                                   */
/* ------------------------------------------------------------------ */

fn url_encode(input: &str) -> String {
    let mut out = String::new();
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn create_surface(
    app: &AppHandle,
    monitor: &Monitor,
    index: usize,
    media_path: &str,
    kind: &str,
    title: &str,
    layer: &LayerHost,
) -> Result<Surface, String> {
    let label = format!("gs-wallpaper-{index}");
    if let Some(existing) = app.get_webview_window(&label) {
        let _ = existing.destroy();
    }

    let position = monitor.position();
    let size = monitor.size();
    let page = format!(
        "wallpaper-host.html?kind={}&path={}&title={}",
        url_encode(kind),
        url_encode(media_path),
        url_encode(title)
    );

    let window = WebviewWindowBuilder::new(app, label.as_str(), WebviewUrl::App(page.into()))
        .title("GameScape Wallpaper")
        .decorations(false)
        .resizable(false)
        .skip_taskbar(true)
        .focused(false)
        .visible(false)
        .build()
        .map_err(|error| format!("cannot create a wallpaper surface: {error}"))?;

    let hwnd = window
        .hwnd()
        .map_err(|error| format!("surface has no window handle: {error}"))?;

    match attach_surface(
        hwnd,
        layer,
        (position.x, position.y, size.width, size.height),
    ) {
        Ok(true) => {}
        Ok(false) => {
            let _ = window.destroy();
            return Err("surface did not attach to the desktop layer".to_string());
        }
        Err(error) => {
            let _ = window.destroy();
            return Err(error);
        }
    }

    // The window handle is simply dropped here; the runtime/manager keeps the
    // window alive until `destroy` (there is no Drop that closes it).

    log_line(&format!(
        "host-surface[{index}] attached hwnd=0x{:X} kind={kind} title=\"{title}\"",
        hwnd.0 as usize
    ));

    Ok(Surface {
        monitor_index: index as u32,
        label,
        kind: kind.to_string(),
        title: title.to_string(),
        media_path: media_path.to_string(),
        paused: false,
    })
}

/// Replace every surface with fresh ones for the given media (all monitors).
pub fn apply_animated(
    app: &AppHandle,
    host: &HostState,
    media_path: &str,
    kind: &str,
    title: &str,
) -> Result<(), String> {
    stop_all(app, host)?;

    let monitors = app
        .available_monitors()
        .map_err(|error| format!("cannot list monitors: {error}"))?;
    if monitors.is_empty() {
        return Err("no monitors reported".to_string());
    }
    let layer = discover_layer()?;

    let mut created: Vec<Surface> = Vec::new();
    for (index, monitor) in monitors.iter().enumerate() {
        match create_surface(app, monitor, index, media_path, kind, title, &layer) {
            Ok(surface) => created.push(surface),
            Err(error) => {
                for surface in &created {
                    if let Some(window) = app.get_webview_window(&surface.label) {
                        let _ = window.destroy();
                    }
                }
                return Err(error);
            }
        }
    }

    let mut inner = host
        .inner
        .lock()
        .map_err(|e| format!("host state lock poisoned: {e}"))?;
    inner.surfaces = created
        .into_iter()
        .map(|surface| (surface.monitor_index, surface))
        .collect();
    inner.parent = Some(layer.parent.0 as isize);
    let _ = app.emit("host:state", ());
    Ok(())
}

/// Destroy every surface (the OS wallpaper beneath remains).
pub fn stop_all(app: &AppHandle, host: &HostState) -> Result<(), String> {
    let labels: Vec<String> = {
        let mut inner = host
            .inner
            .lock()
            .map_err(|e| format!("host state lock poisoned: {e}"))?;
        let labels = inner
            .surfaces
            .values()
            .map(|surface| surface.label.clone())
            .collect();
        inner.surfaces.clear();
        inner.parent = None;
        labels
    };
    for label in labels {
        if let Some(window) = app.get_webview_window(&label) {
            let _ = window.destroy();
        }
    }
    let _ = app.emit("host:state", ());
    Ok(())
}

pub fn status(host: &HostState) -> Vec<HostSurface> {
    host.inner
        .lock()
        .map(|inner| {
            let mut surfaces: Vec<HostSurface> = inner
                .surfaces
                .values()
                .map(|surface| HostSurface {
                    monitor_index: surface.monitor_index,
                    kind: surface.kind.clone(),
                    title: surface.title.clone(),
                    paused: surface.paused,
                })
                .collect();
            surfaces.sort_by_key(|surface| surface.monitor_index);
            surfaces
        })
        .unwrap_or_default()
}

pub fn set_paused(app: &AppHandle, host: &HostState, paused: bool) -> Result<(), String> {
    let labels: Vec<String> = {
        let mut inner = host
            .inner
            .lock()
            .map_err(|e| format!("host state lock poisoned: {e}"))?;
        for surface in inner.surfaces.values_mut() {
            surface.paused = paused;
        }
        inner
            .surfaces
            .values()
            .map(|surface| surface.label.clone())
            .collect()
    };
    for label in labels {
        if let Some(window) = app.get_webview_window(&label) {
            let _ = window.eval(&format!(
                "window.__gsHost && window.__gsHost.setPaused({paused});"
            ));
        }
    }
    let _ = app.emit("host:state", ());
    Ok(())
}

/* ------------------------------------------------------------------ */
/* Self-healing layer monitor                                          */
/* ------------------------------------------------------------------ */

pub fn init(app: &AppHandle) {
    let handle = app.clone();
    thread::spawn(move || loop {
        thread::sleep(Duration::from_secs(2));

        let host = handle.state::<HostState>();
        let dead = {
            let Ok(inner) = host.inner.lock() else { continue };
            match inner.parent {
                Some(raw) => !unsafe { IsWindow(Some(hwnd_from_raw(raw))) }.as_bool(),
                None => false,
            }
        };
        if !dead {
            continue;
        }

        log_line("desktop layer lost (explorer restart?) - re-attaching");
        let specs: Vec<(String, String, String)> = {
            let Ok(inner) = host.inner.lock() else { continue };
            inner
                .surfaces
                .values()
                .map(|surface| {
                    (
                        surface.media_path.clone(),
                        surface.kind.clone(),
                        surface.title.clone(),
                    )
                })
                .collect()
        };
        if specs.is_empty() {
            if let Ok(mut inner) = host.inner.lock() {
                inner.parent = None;
            }
            continue;
        }

        if let Err(error) = stop_all(&handle, &host) {
            log_line(&format!("re-attach cleanup failed: {error}"));
        }
        let Ok(layer) = discover_layer() else {
            log_line("re-attach discovery failed - will retry");
            continue;
        };
        let monitors = handle.available_monitors().unwrap_or_default();
        let mut created: Vec<Surface> = Vec::new();
        for (index, monitor) in monitors.iter().enumerate() {
            if let Some((path, kind, title)) = specs.get(index) {
                match create_surface(&handle, monitor, index, path, kind, title, &layer) {
                    Ok(surface) => created.push(surface),
                    Err(error) => log_line(&format!("re-attach failed on monitor {index}: {error}")),
                }
            }
        }
        if let Ok(mut inner) = host.inner.lock() {
            inner.surfaces = created
                .into_iter()
                .map(|surface| (surface.monitor_index, surface))
                .collect();
            inner.parent = Some(layer.parent.0 as isize);
        }
        log_line("surfaces re-attached");
        let _ = handle.emit("host:state", ());
    });
}

/* ------------------------------------------------------------------ */
/* Shutdown                                                            */
/* ------------------------------------------------------------------ */

pub fn shutdown(app: &AppHandle) {
    let host = app.state::<HostState>();
    if let Err(error) = stop_all(app, &host) {
        log_line(&format!("shutdown cleanup failed: {error}"));
    }
    log_line("desktop host stopped");
}
