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
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{window::Color, AppHandle, Emitter, Manager, Monitor, WebviewUrl, WebviewWindowBuilder};

use windows::core::PCWSTR;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    ClientToScreen, CreateRectRgn, GetMonitorInfoW, MonitorFromWindow, ScreenToClient,
    SetWindowRgn, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_APARTMENTTHREADED,
};
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};
use windows::Win32::UI::Shell::{
    DesktopWallpaper, IDesktopWallpaper, SHQueryUserNotificationState, DESKTOP_WALLPAPER_POSITION,
    DWPOS_CENTER, DWPOS_FILL, DWPOS_FIT, DWPOS_SPAN, DWPOS_STRETCH, QUNS_BUSY,
    QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, FindWindowExW, FindWindowW, GetClassNameW, GetClientRect, GetCursorPos,
    GetForegroundWindow, GetWindow, GetWindowLongPtrW, GetWindowRect, GetWindowThreadProcessId,
    IsWindow, PostMessageW, SendMessageTimeoutW, SetLayeredWindowAttributes, SetParent,
    SetWindowLongPtrW, SetWindowPos, ShowWindow, WindowFromPoint, GWL_EXSTYLE, GWL_STYLE, GW_CHILD,
    GW_HWNDNEXT, HWND_BOTTOM, LWA_ALPHA, SMTO_NORMAL, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER,
    SWP_SHOWWINDOW, SW_SHOWNOACTIVATE, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WS_CAPTION,
    WS_CHILD, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW,
    WS_POPUP,
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
    pub media_id: i64,
    pub kind: String,
    pub title: String,
    pub paused: bool,
    /// Why playback is paused, when it is ("paused by you",
    /// "fullscreen app", "on battery").
    pub paused_reason: Option<String>,
}

/// The wallpaper GameScape currently has applied (for UI state and toggles).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppliedWallpaper {
    pub kind: String,
    pub media_id: i64,
    pub title: String,
}

/// A monitor as seen by the host (for per-monitor assignment).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostMonitor {
    pub index: u32,
    pub width: u32,
    pub height: u32,
}

/// Everything the UI needs to render desktop-host state.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostReport {
    pub surfaces: Vec<HostSurface>,
    pub applied: Option<AppliedWallpaper>,
    pub monitors: Vec<HostMonitor>,
}

struct Surface {
    monitor_index: u32,
    media_id: i64,
    label: String,
    kind: String,
    title: String,
    media_path: String,
    /// Fit mode passed to the renderer page ("fill", "fit", ...).
    fit: String,
}

pub struct HostState {
    inner: Mutex<HostInner>,
    /// Serializes widget command bodies (database write + desktop sync) and
    /// background syncs, so concurrent operations can never rebuild the
    /// desktop from a stale row snapshot.
    serial: Mutex<()>,
}

struct HostInner {
    surfaces: HashMap<u32, Surface>,
    /// Raw handle value (HWND is not Send, so it is stored as isize).
    parent: Option<isize>,
    /// Pause requested from the UI (Pause button on Home).
    manual_paused: bool,
    /// Pause requested by the governor (fullscreen app / on battery).
    governor_paused: bool,
    governor_reason: Option<String>,
    /// When the wallpaper last changed (rotation scheduling).
    last_rotation: Option<Instant>,
    /// Live widget windows by widget id.
    widgets: HashMap<i64, WidgetWindow>,
}

/// A live widget window on the desktop layer.
#[derive(Clone)]
struct WidgetWindow {
    label: String,
    kind: String,
    monitor: u32,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    settings: Option<String>,
}

impl WidgetWindow {
    fn matches(&self, row: &storage::WidgetRow) -> bool {
        self.kind == row.kind
            && self.monitor == row.monitor
            && self.x == row.x
            && self.y == row.y
            && self.width == row.width
            && self.height == row.height
            && self.settings == row.settings
    }
}

impl HostState {
    /// Serialization guard shared by widget commands and syncs.
    pub fn serial_lock(&self) -> Result<std::sync::MutexGuard<'_, ()>, String> {
        self.serial
            .lock()
            .map_err(|e| format!("host serial lock poisoned: {e}"))
    }

    pub fn new() -> Self {
        HostState {
            serial: Mutex::new(()),
            inner: Mutex::new(HostInner {
                surfaces: HashMap::new(),
                parent: None,
                manual_paused: false,
                governor_paused: false,
                governor_reason: None,
                last_rotation: Some(Instant::now()),
                widgets: HashMap::new(),
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
/* Wallpaper fit                                                       */
/* ------------------------------------------------------------------ */

/// Maps the stored setting to the Windows position and a display name.
/// The default (and the point of the feature): wallpapers always fit the
/// screen — filled edge to edge.
fn fit_mode(value: Option<&str>) -> (&'static str, DESKTOP_WALLPAPER_POSITION) {
    match value.map(|v| v.to_ascii_lowercase()).as_deref() {
        Some("fit") => ("fit", DWPOS_FIT),
        Some("stretch") => ("stretch", DWPOS_STRETCH),
        Some("center") => ("center", DWPOS_CENTER),
        Some("span") => ("span", DWPOS_SPAN),
        _ => ("fill", DWPOS_FILL),
    }
}

fn position_name(value: i32) -> &'static str {
    match value {
        0 => "center",
        1 => "tile",
        2 => "stretch",
        3 => "fit",
        4 => "fill",
        5 => "span",
        _ => "unknown",
    }
}

/// The fit mode in effect for the current user settings.
pub fn configured_fit(state: &AppState) -> (&'static str, DESKTOP_WALLPAPER_POSITION) {
    let value = state
        .settings
        .lock()
        .map(|settings| settings.wallpaper_fit.clone())
        .unwrap_or(None);
    fit_mode(value.as_deref())
}

/* ------------------------------------------------------------------ */
/* Governor: pause on fullscreen apps and battery                      */
/* ------------------------------------------------------------------ */

/// Does this foreground window look like a fullscreen app (game, video)?
/// A normal maximized window has a caption; games (exclusive or borderless)
/// do not. Our own windows and shell surfaces never count.
fn should_pause_for_fullscreen(
    covers_monitor: bool,
    has_caption: bool,
    own_process: bool,
    shell_surface: bool,
) -> bool {
    covers_monitor && !has_caption && !own_process && !shell_surface
}

/// Pause reason priority: fullscreen first, then battery.
fn governor_reason(fullscreen: bool, on_battery: bool) -> Option<&'static str> {
    if fullscreen {
        Some("fullscreen app")
    } else if on_battery {
        Some("on battery")
    } else {
        None
    }
}

fn foreground_covers_screen() -> bool {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return false;
        }

        let mut class_buffer = [0u16; 128];
        let length = GetClassNameW(hwnd, &mut class_buffer);
        let class = if length > 0 {
            String::from_utf16_lossy(&class_buffer[..length as usize])
        } else {
            String::new()
        };
        let shell_surface = matches!(
            class.as_str(),
            "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
        );

        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        let own_process = pid == std::process::id();

        let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        let has_caption = style & WS_CAPTION.0 != 0;

        let mut rect = RECT::default();
        let covers = if GetWindowRect(hwnd, &mut rect).is_ok() {
            let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
            let mut info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            if GetMonitorInfoW(monitor, &mut info).as_bool() {
                let bounds = info.rcMonitor;
                rect.left <= bounds.left
                    && rect.top <= bounds.top
                    && rect.right >= bounds.right
                    && rect.bottom >= bounds.bottom
            } else {
                false
            }
        } else {
            false
        };

        should_pause_for_fullscreen(covers, has_caption, own_process, shell_surface)
    }
}

/// The shell's own "is a fullscreen app running" notification state —
/// catches exclusive-fullscreen games the rectangle check might miss.
fn shell_reports_fullscreen() -> bool {
    match unsafe { SHQueryUserNotificationState() } {
        Ok(state) => {
            state == QUNS_RUNNING_D3D_FULL_SCREEN
                || state == QUNS_BUSY
                || state == QUNS_PRESENTATION_MODE
        }
        Err(_) => false,
    }
}

fn on_battery() -> bool {
    let mut status = SYSTEM_POWER_STATUS::default();
    if unsafe { GetSystemPowerStatus(&mut status) }.is_ok() {
        // 0 = offline (running on battery); 255 = unknown.
        status.ACLineStatus == 0
    } else {
        false
    }
}

/// Applies the combined (manual + governor) pause state to every surface.
fn apply_pause_state(app: &AppHandle, host: &HostState) -> Result<(), String> {
    let (effective, labels) = {
        let inner = host
            .inner
            .lock()
            .map_err(|e| format!("host state lock poisoned: {e}"))?;
        let effective = inner.manual_paused || inner.governor_paused;
        let labels: Vec<String> = inner
            .surfaces
            .values()
            .map(|surface| surface.label.clone())
            .collect();
        (effective, labels)
    };
    for label in labels {
        if let Some(window) = app.get_webview_window(&label) {
            let _ = window.eval(&format!(
                "window.__gsHost && window.__gsHost.setPaused({effective});"
            ));
        }
    }
    let _ = app.emit("host:state", ());
    Ok(())
}

/// One governor tick: watch fullscreen foreground windows and battery state.
fn governor_tick(app: &AppHandle, host: &HostState) {
    let fullscreen = foreground_covers_screen() || shell_reports_fullscreen();
    let battery = on_battery();
    let reason = governor_reason(fullscreen, battery);

    let changed = {
        let Ok(mut inner) = host.inner.lock() else {
            return;
        };
        let new_reason = reason.map(|value| value.to_string());
        if new_reason != inner.governor_reason {
            inner.governor_paused = new_reason.is_some();
            inner.governor_reason = new_reason;
            true
        } else {
            false
        }
    };

    if !changed {
        return;
    }
    match reason {
        Some(value) => log_line(&format!("governor: paused ({value})")),
        None => log_line("governor: resumed"),
    }
    if let Err(error) = apply_pause_state(app, host) {
        log_line(&format!("governor: cannot apply pause state: {error}"));
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

    // The window class keeps an invisible non-client border after the style
    // change; its pixels were showing as pale lines at the edges. Grow the
    // window by that border so the client area equals the intended
    // rectangle exactly, then clip the window to its client area — the
    // edges are then pixel-exact with nothing showing around them.
    unsafe {
        let mut client = RECT::default();
        let mut window_rect = RECT::default();
        let mut origin = POINT { x: 0, y: 0 };
        if GetClientRect(hwnd, &mut client).is_ok()
            && GetWindowRect(hwnd, &mut window_rect).is_ok()
            && ClientToScreen(hwnd, &mut origin).as_bool()
        {
            let offset_x = origin.x - window_rect.left;
            let offset_y = origin.y - window_rect.top;
            let chrome_w = (window_rect.right - window_rect.left) - client.right;
            let chrome_h = (window_rect.bottom - window_rect.top) - client.bottom;
            if chrome_w != 0 || chrome_h != 0 {
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    child_x - offset_x,
                    child_y - offset_y,
                    mw as i32 + chrome_w,
                    mh as i32 + chrome_h,
                    SWP_NOACTIVATE | SWP_NOZORDER | SWP_SHOWWINDOW,
                );
            }

            // Re-measure after the resize, then clip to the client area.
            let mut client = RECT::default();
            let mut window_rect = RECT::default();
            let mut origin = POINT { x: 0, y: 0 };
            if GetClientRect(hwnd, &mut client).is_ok()
                && GetWindowRect(hwnd, &mut window_rect).is_ok()
                && ClientToScreen(hwnd, &mut origin).as_bool()
            {
                let offset_x = origin.x - window_rect.left;
                let offset_y = origin.y - window_rect.top;
                let region = CreateRectRgn(
                    offset_x,
                    offset_y,
                    offset_x + client.right,
                    offset_y + client.bottom,
                );
                // On success the system owns the region; a rare failure
                // just leaks it until process exit.
                let _ = SetWindowRgn(hwnd, Some(region), true);
                log_line(&format!(
                    "attach: client {}x{} at +{},{} in window {}x{}",
                    client.right,
                    client.bottom,
                    offset_x,
                    offset_y,
                    window_rect.right - window_rect.left,
                    window_rect.bottom - window_rect.top,
                ));
            }
        }
    }

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

/// Apply a static image to every monitor using the configured fit mode
/// (default: fill the screen edge to edge). The wallpaper in place before
/// the first apply is remembered per monitor — including its fit mode —
/// so restore puts everything back exactly as it was.
pub fn apply_static(
    state: &AppState,
    library_path: &str,
    format: Option<&str>,
    monitor: Option<u32>,
) -> Result<String, String> {
    let ready_path = ensure_static_ready(library_path, format)?;
    let ready_wide = wide(&ready_path);
    let (fit_name, fit_position) = configured_fit(state);

    let (previous, previous_position) = with_desktop_wallpaper(|desktop| unsafe {
        let count = desktop
            .GetMonitorDevicePathCount()
            .map_err(|error| format!("cannot list monitors: {error}"))?;

        let previous_position = desktop
            .GetPosition()
            .map(|position| position.0)
            .unwrap_or(-1);
        desktop
            .SetPosition(fit_position)
            .map_err(|error| format!("cannot set the wallpaper fit mode: {error}"))?;
        log_line(&format!(
            "wallpaper fit: {fit_name} (was {})",
            position_name(previous_position)
        ));

        let indices: Vec<u32> = match monitor {
            Some(target) if target < count => vec![target],
            Some(target) => {
                return Err(format!(
                    "monitor {} does not exist ({count} monitor(s) reported)",
                    target + 1
                ))
            }
            None => (0..count).collect(),
        };

        let mut previous: Vec<(String, String)> = Vec::new();
        for index in indices {
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
        Ok((previous, previous_position))
    })?;

    let prior_position_db: Option<i64> = if (0..=5).contains(&previous_position) {
        Some(previous_position as i64)
    } else {
        None
    };

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
                    "INSERT INTO wallpaper_state
                        (monitor_id, previous_path, applied_path, previous_position)
                     VALUES (?1, ?2, ?3, ?4)",
                    rusqlite::params![monitor_id, prior, ready_path, prior_position_db],
                );
            } else {
                let _ = conn.execute(
                    "UPDATE wallpaper_state
                     SET applied_path = ?1,
                         previous_position = COALESCE(previous_position, ?2),
                         applied_at = datetime('now')
                     WHERE monitor_id = ?3",
                    rusqlite::params![ready_path, prior_position_db, monitor_id],
                );
            }
        }
    }
    Ok(ready_path)
}

/// Restore the wallpapers — and the fit mode — that were in place before
/// GameScape applied any.
pub fn restore_previous(state: &AppState) -> Result<(), String> {
    let rows: Vec<(String, Option<String>, Option<i64>)> = {
        let conn = state
            .db
            .lock()
            .map_err(|e| format!("database lock poisoned: {e}"))?;
        let mut statement = conn
            .prepare("SELECT monitor_id, previous_path, previous_position FROM wallpaper_state")
            .map_err(|error| format!("cannot read wallpaper state: {error}"))?;
        let mapped = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<i64>>(2)?,
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

    let previous_position = rows
        .iter()
        .find_map(|(_, _, position)| position.map(|value| value as i32))
        .filter(|position| (0..=5).contains(position));

    with_desktop_wallpaper(|desktop| unsafe {
        if let Some(position) = previous_position {
            desktop
                .SetPosition(DESKTOP_WALLPAPER_POSITION(position))
                .map_err(|error| format!("cannot restore the wallpaper fit: {error}"))?;
        }
        for (monitor_id, previous, _) in &rows {
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
    if let Ok(mut settings) = state.settings.lock() {
        settings.last_wallpaper = None;
        let _ = storage::save_settings(&settings);
    }
    match previous_position {
        Some(position) => log_line(&format!(
            "previous wallpaper restored (fit: {})",
            position_name(position)
        )),
        None => log_line("previous wallpaper restored"),
    }
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

/// Destroys a window by label and waits (bounded) until the manager has
/// released the label — destroys are dispatched to the event loop, so an
/// immediate rebuild with the same label would race it.
fn release_window_label(app: &AppHandle, label: &str) {
    if let Some(existing) = app.get_webview_window(label) {
        let _ = existing.destroy();
        for _ in 0..50 {
            if app.get_webview_window(label).is_none() {
                return;
            }
            thread::sleep(Duration::from_millis(40));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn create_surface(
    app: &AppHandle,
    monitor: &Monitor,
    index: usize,
    media_id: i64,
    media_path: &str,
    kind: &str,
    title: &str,
    fit: &str,
    start_paused: bool,
    layer: &LayerHost,
) -> Result<Surface, String> {
    let label = format!("gs-wallpaper-{index}");
    release_window_label(app, &label);

    let position = monitor.position();
    let size = monitor.size();
    // The paused flag rides in the URL: the page starts paused without a
    // race against an eval that could arrive before the page loads.
    let page = format!(
        "wallpaper-host.html?kind={}&path={}&title={}&fit={}&paused={}",
        url_encode(kind),
        url_encode(media_path),
        url_encode(title),
        url_encode(fit),
        if start_paused { 1 } else { 0 }
    );

    let window = WebviewWindowBuilder::new(app, label.as_str(), WebviewUrl::App(page.into()))
        .title("GameScape Wallpaper")
        .decorations(false)
        .resizable(false)
        .skip_taskbar(true)
        .focused(false)
        .visible(false)
        // Black behind the media: no white flash while the page loads.
        .background_color(Color(0, 0, 0, 255))
        // Surface pages report diagnostics through their document title
        // (used by local verification harnesses); mirror it on the window.
        .on_document_title_changed(|window, title| {
            let _ = window.set_title(&title);
        })
        .build()
        .map_err(|error| format!("cannot create a wallpaper surface: {error}"))?;

    let hwnd = window
        .hwnd()
        .map_err(|error| format!("surface has no window handle: {error}"))?;

    // Over-scan past every monitor edge so window chrome (the webview
    // content is inset ~8px inside the window frame on this class) always
    // lands off-screen and never shows as lines at the screen sides.
    const OVERSCAN: i32 = 12;
    let rect = (
        position.x - OVERSCAN,
        position.y - OVERSCAN,
        (size.width as i32 + OVERSCAN * 2) as u32,
        (size.height as i32 + OVERSCAN * 2) as u32,
    );

    match attach_surface(hwnd, layer, rect) {
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
        media_id,
        label,
        kind: kind.to_string(),
        title: title.to_string(),
        media_path: media_path.to_string(),
        fit: fit.to_string(),
    })
}

fn pause_state(host: &HostState) -> bool {
    host.inner
        .lock()
        .map(|inner| inner.manual_paused || inner.governor_paused)
        .unwrap_or(false)
}

/// Destroys the surface for one monitor; other monitors keep animating.
fn remove_surface(app: &AppHandle, host: &HostState, monitor: u32) -> Result<(), String> {
    let label = {
        let mut inner = host
            .inner
            .lock()
            .map_err(|e| format!("host state lock poisoned: {e}"))?;
        inner.surfaces.remove(&monitor).map(|surface| surface.label)
    };
    if let Some(label) = label {
        if let Some(window) = app.get_webview_window(&label) {
            let _ = window.destroy();
        }
    }
    let _ = app.emit("host:state", ());
    Ok(())
}

/// Applies animated media to every monitor, or replaces just one monitor's
/// surface (per-monitor assignment) while the others keep playing.
pub fn apply_animated(
    app: &AppHandle,
    host: &HostState,
    media_path: &str,
    kind: &str,
    title: &str,
    media_id: i64,
    fit: &str,
    monitor: Option<u32>,
) -> Result<(), String> {
    let monitors = app
        .available_monitors()
        .map_err(|error| format!("cannot list monitors: {error}"))?;
    if monitors.is_empty() {
        return Err("no monitors reported".to_string());
    }
    if let Some(target) = monitor {
        if target as usize >= monitors.len() {
            return Err(format!(
                "monitor {} does not exist ({} monitor(s) reported)",
                target + 1,
                monitors.len()
            ));
        }
    }

    let layer = discover_layer()?;

    match monitor {
        Some(target) => {
            // Single monitor: swap just that surface; leave the rest alone.
            let existing_label = {
                let inner = host
                    .inner
                    .lock()
                    .map_err(|e| format!("host state lock poisoned: {e}"))?;
                inner.surfaces.get(&target).map(|surface| surface.label.clone())
            };
            if let Some(label) = existing_label {
                if let Some(window) = app.get_webview_window(&label) {
                    let _ = window.destroy();
                }
            }
            let start_paused = pause_state(host);
            let surface = create_surface(
                app,
                &monitors[target as usize],
                target as usize,
                media_id,
                media_path,
                kind,
                title,
                fit,
                start_paused,
                &layer,
            )?;
            let mut inner = host
                .inner
                .lock()
                .map_err(|e| format!("host state lock poisoned: {e}"))?;
            inner.surfaces.insert(target, surface);
            inner.parent = Some(layer.parent.0 as isize);
        }
        None => {
            stop_all(app, host)?;
            let start_paused = pause_state(host);

            let mut created: Vec<Surface> = Vec::new();
            for (index, monitor) in monitors.iter().enumerate() {
                match create_surface(
                    app,
                    monitor,
                    index,
                    media_id,
                    media_path,
                    kind,
                    title,
                    fit,
                    start_paused,
                    &layer,
                ) {
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
        }
    }

    let (effective_paused, pause_reason) = {
        let inner = host
            .inner
            .lock()
            .map_err(|e| format!("host state lock poisoned: {e}"))?;
        let effective = inner.manual_paused || inner.governor_paused;
        let reason = if inner.governor_paused {
            inner.governor_reason.clone()
        } else if inner.manual_paused {
            Some("paused by you".to_string())
        } else {
            None
        };
        (effective, reason)
    };
    if effective_paused {
        log_line(&format!(
            "new surfaces start paused ({})",
            pause_reason.as_deref().unwrap_or("paused")
        ));
    }
    // Push the effective state to the fresh pages (they already start from
    // the URL flag; this covers state changing while they were created).
    apply_pause_state(app, host)?;
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
        // Stopping clears a manual pause (new applies start playing); the
        // governor's environmental pause is not user intent and stays.
        inner.manual_paused = false;
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
            let effective = inner.manual_paused || inner.governor_paused;
            let reason: Option<String> = if inner.governor_paused {
                inner.governor_reason.clone()
            } else if inner.manual_paused {
                Some("paused by you".to_string())
            } else {
                None
            };
            let mut surfaces: Vec<HostSurface> = inner
                .surfaces
                .values()
                .map(|surface| HostSurface {
                    monitor_index: surface.monitor_index,
                    media_id: surface.media_id,
                    kind: surface.kind.clone(),
                    title: surface.title.clone(),
                    paused: effective,
                    paused_reason: reason.clone(),
                })
                .collect();
            surfaces.sort_by_key(|surface| surface.monitor_index);
            surfaces
        })
        .unwrap_or_default()
}

/// Surfaces + the last applied wallpaper — what the UI renders from.
pub fn report(app: &AppHandle, host: &HostState, state: &AppState) -> HostReport {
    let applied = state
        .settings
        .lock()
        .ok()
        .and_then(|settings| settings.last_wallpaper.clone())
        .map(|last| AppliedWallpaper {
            kind: last.kind,
            media_id: last.media_id,
            title: last.title,
        });
    let monitors = app
        .available_monitors()
        .unwrap_or_default()
        .iter()
        .enumerate()
        .map(|(index, monitor)| HostMonitor {
            index: index as u32,
            width: monitor.size().width,
            height: monitor.size().height,
        })
        .collect();
    HostReport {
        surfaces: status(host),
        applied,
        monitors,
    }
}

/// Applies a library item: static images through the Windows API, animated
/// media through the desktop-layer surfaces. Used by the UI command, the
/// rotation scheduler and anything else that needs to set a wallpaper.
pub fn apply_library_item(
    app: &AppHandle,
    state: &AppState,
    host: &HostState,
    media_id: i64,
    monitor: Option<u32>,
) -> Result<String, String> {
    let (path, format, title) = {
        let conn = state
            .db
            .lock()
            .map_err(|e| format!("database lock poisoned: {e}"))?;
        conn.query_row(
            "SELECT path, format, title FROM media WHERE id = ?1",
            [media_id],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .map_err(|e| format!("library item not found: {e}"))?
    };
    let path = path.ok_or("this entry has no file on disk")?;
    let format_value = format.unwrap_or_default().to_ascii_lowercase();

    let kind = match format_value.as_str() {
        "mp4" => "video",
        "gif" => "gif",
        _ => "static",
    };

    let scope = match monitor {
        Some(target) => format!(" [monitor {}]", target + 1),
        None => String::new(),
    };

    if kind == "static" {
        match monitor {
            Some(target) => remove_surface(app, host, target)?,
            None => stop_all(app, host)?,
        }
        let applied = apply_static(state, &path, Some(&format_value), monitor)?;
        remember_last_wallpaper(
            state,
            host,
            "static",
            media_id,
            &path,
            Some(&format_value),
            &title,
            monitor,
        );
        log_line(&format!("HOST_APPLY_OK static {applied}{scope}"));
    } else {
        let (fit_name, _) = configured_fit(state);
        apply_animated(app, host, &path, kind, &title, media_id, fit_name, monitor)?;
        remember_last_wallpaper(
            state,
            host,
            kind,
            media_id,
            &path,
            Some(&format_value),
            &title,
            monitor,
        );
        log_line(&format!("HOST_APPLY_OK {kind} {title}{scope}"));
    }
    Ok(kind.to_string())
}

fn sanitize_hex(value: &str) -> Option<String> {
    let hex = value.trim().trim_start_matches('#').to_ascii_lowercase();
    (hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit())).then_some(hex)
}

/// Applies a cursor-reactive catalogue wallpaper: live surfaces render the
/// effect and follow the cursor bridge.
pub fn apply_interactive(
    app: &AppHandle,
    state: &AppState,
    host: &HostState,
    media_id: i64,
    effect: &str,
    dominant: &str,
    accent: &str,
) -> Result<(), String> {
    let effect = match effect {
        "ribbon" | "grid" | "orbit" => effect,
        _ => "orbit",
    };
    let dominant = sanitize_hex(dominant).ok_or("invalid dominant colour")?;
    let accent = sanitize_hex(accent).ok_or("invalid accent colour")?;
    let token = format!("{effect}|{dominant}|{accent}");
    let title = format!("Interactive · {effect}");
    let (fit_name, _) = configured_fit(state);
    apply_animated(
        app,
        host,
        &token,
        "interactive",
        &title,
        media_id,
        fit_name,
        None,
    )?;
    remember_last_wallpaper(
        state,
        host,
        "interactive",
        media_id,
        &token,
        None,
        &title,
        None,
    );
    log_line(&format!("HOST_APPLY_OK interactive {effect}"));
    Ok(())
}

/// Remembers the wallpaper just applied (startup restore) and restarts the
/// rotation clock so scheduled changes do not fire right after a manual one.
pub fn remember_last_wallpaper(
    state: &AppState,
    host: &HostState,
    kind: &str,
    media_id: i64,
    path: &str,
    format: Option<&str>,
    title: &str,
    monitor: Option<u32>,
) {
    if let Ok(mut settings) = state.settings.lock() {
        settings.last_wallpaper = Some(storage::LastWallpaper {
            kind: kind.to_string(),
            media_id,
            path: path.to_string(),
            format: format.map(|value| value.to_string()),
            title: title.to_string(),
            monitor,
        });
        settings.wallpaper_enabled = Some(true);
        let _ = storage::save_settings(&settings);
    }
    if let Ok(mut inner) = host.inner.lock() {
        inner.last_rotation = Some(Instant::now());
    }
}

/// Turn everything off: stop animations, put the previous wallpaper back,
/// and remember that the user turned wallpapers off (no startup restore).
pub fn turn_off(app: &AppHandle, state: &AppState, host: &HostState) -> Result<(), String> {
    stop_all(app, host)?;
    if let Err(error) = restore_previous(state) {
        log_line(&format!("turn off: nothing to restore ({error})"));
    }
    if let Ok(mut settings) = state.settings.lock() {
        settings.last_wallpaper = None;
        settings.wallpaper_enabled = Some(false);
        let _ = storage::save_settings(&settings);
    }
    log_line("wallpaper turned off");
    Ok(())
}

/// Manual pause from the UI. The governor layers on top of this, so a
/// surface stays paused while both request it and resumes when neither does.
pub fn set_paused(app: &AppHandle, host: &HostState, paused: bool) -> Result<(), String> {
    {
        let mut inner = host
            .inner
            .lock()
            .map_err(|e| format!("host state lock poisoned: {e}"))?;
        inner.manual_paused = paused;
    }
    apply_pause_state(app, host)
}

/* ------------------------------------------------------------------ */
/* Widget drag watcher                                                 */
/* ------------------------------------------------------------------ */

/// Mouse polling cadence: relaxed while idle, display-rate while interactive
/// wallpaper surfaces are live (they need the cursor to follow the mouse
/// smoothly) or a drag is in progress.
const DRAG_POLL_MS: u64 = 40;
const DRAG_POLL_FAST_MS: u64 = 12;

/// Movement past this many pixels turns a press into a drag; anything less
/// (and a quick release) is treated as a click into the widget content.
const DRAG_THRESHOLD_PX: i32 = 7;

fn has_interactive_surfaces(host: &HostState) -> bool {
    host.inner
        .lock()
        .map(|inner| {
            inner
                .surfaces
                .values()
                .any(|surface| surface.kind == "interactive")
        })
        .unwrap_or(false)
}

#[derive(Clone, Copy)]
struct DragState {
    widget_id: i64,
    offset_x: i32,
    offset_y: i32,
    monitor: u32,
    width: u32,
    height: u32,
}

/// Is the topmost window at the point one of our own widget windows?
fn point_over_widget_window(app: &AppHandle, host: &HostState, cursor: POINT) -> bool {
    let hwnd = unsafe { WindowFromPoint(cursor) };
    if hwnd.0.is_null() {
        return false;
    }
    let mine: Vec<isize> = {
        let Ok(inner) = host.inner.lock() else {
            return false;
        };
        inner
            .widgets
            .values()
            .filter_map(|widget| {
                app.get_webview_window(&widget.label)
                    .and_then(|window| window.hwnd().ok())
            })
            .map(|handle| handle.0 as isize)
            .collect()
    };
    mine.contains(&(hwnd.0 as isize))
}

/// True when the topmost window at the point belongs to the desktop shell —
/// i.e. the click really lands on the desktop and not on an app window.
fn cursor_owns_desktop(cursor: POINT) -> bool {
    unsafe {
        let hwnd = WindowFromPoint(cursor);
        if hwnd.0.is_null() {
            return false;
        }
        let mut class_buffer = [0u16; 128];
        let length = GetClassNameW(hwnd, &mut class_buffer);
        let class = if length > 0 {
            String::from_utf16_lossy(&class_buffer[..length as usize])
        } else {
            String::new()
        };
        matches!(
            class.as_str(),
            "SHELLDLL_DefView" | "SysListView32" | "Progman" | "WorkerW"
        )
    }
}

fn begin_drag(app: &AppHandle, host: &HostState, cursor: POINT) -> Option<DragState> {
    // Desktop clicks and clicks on our own (top-level) widgets both drag.
    if !cursor_owns_desktop(cursor) && !point_over_widget_window(app, host, cursor) {
        return None;
    }
    let monitors = app.available_monitors().unwrap_or_default();
    let inner = host.inner.lock().ok()?;
    for (id, widget) in inner.widgets.iter() {
        let monitor = monitors.get(widget.monitor as usize)?;
        let position = monitor.position();
        let left = position.x + widget.x;
        let top = position.y + widget.y;
        if cursor.x >= left
            && cursor.x < left + widget.width as i32
            && cursor.y >= top
            && cursor.y < top + widget.height as i32
        {
            return Some(DragState {
                widget_id: *id,
                offset_x: cursor.x - left,
                offset_y: cursor.y - top,
                monitor: widget.monitor,
                width: widget.width,
                height: widget.height,
            });
        }
    }
    None
}

fn drag_move(app: &AppHandle, host: &HostState, drag: &DragState, cursor: POINT) {
    let (parent_raw, label) = {
        let Ok(inner) = host.inner.lock() else {
            return;
        };
        let Some(parent) = inner.parent else {
            return;
        };
        let Some(widget) = inner.widgets.get(&drag.widget_id) else {
            return;
        };
        (parent, widget.label.clone())
    };
    let parent = hwnd_from_raw(parent_raw);
    let mut parent_rect = RECT::default();
    if unsafe { GetWindowRect(parent, &mut parent_rect) }.is_err() {
        return;
    }
    let Some(window) = app.get_webview_window(&label) else {
        return;
    };
    let Ok(hwnd) = window.hwnd() else {
        return;
    };
    let screen_x = cursor.x - drag.offset_x;
    let screen_y = cursor.y - drag.offset_y;
    let _ = unsafe {
        SetWindowPos(
            hwnd,
            None,
            screen_x - parent_rect.left,
            screen_y - parent_rect.top,
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        )
    };
}

fn finish_drag(app: &AppHandle, host: &HostState, state: &AppState, drag: &DragState, cursor: POINT) {
    let monitors = app.available_monitors().unwrap_or_default();
    let Some(monitor) = monitors.get(drag.monitor as usize) else {
        return;
    };
    let position = monitor.position();
    let size = monitor.size();
    let max_x = (size.width as i32 - drag.width as i32).max(0);
    let max_y = (size.height as i32 - drag.height as i32).max(0);
    let new_x = (cursor.x - drag.offset_x - position.x).clamp(0, max_x);
    let new_y = (cursor.y - drag.offset_y - position.y).clamp(0, max_y);

    // Persist and keep the live map in step so the next sync reuses the
    // window exactly where it was dropped.
    if let Ok(conn) = state.db.lock() {
        let _ = conn.execute(
            "UPDATE widgets SET x = ?1, y = ?2 WHERE id = ?3",
            rusqlite::params![new_x, new_y, drag.widget_id],
        );
    }
    if let Ok(mut inner) = host.inner.lock() {
        if let Some(widget) = inner.widgets.get_mut(&drag.widget_id) {
            widget.x = new_x;
            widget.y = new_y;
        }
    }
    log_line(&format!(
        "widget drag: id={} moved to {},{}",
        drag.widget_id, new_x, new_y
    ));
    let _ = app.emit("host:state", ());
}

/// Depth-first search for the WebView2 input child (render widget host).
fn find_webview_input_hwnd(root: HWND) -> Option<HWND> {
    let mut best: Option<(HWND, u32)> = None;
    let mut stack: Vec<HWND> = vec![root];
    while let Some(parent) = stack.pop() {
        let mut child = unsafe { GetWindow(parent, GW_CHILD) }.ok();
        let mut walked = 0;
        while let Some(current) = child {
            let mut class_buffer = [0u16; 128];
            let length = unsafe { GetClassNameW(current, &mut class_buffer) };
            if length > 0 {
                let class = String::from_utf16_lossy(&class_buffer[..length as usize]);
                let rank = match class.as_str() {
                    "Chrome_RenderWidgetHostHWND" => 3,
                    "Chrome_WidgetWin_0" | "Chrome_WidgetWin_1" => 2,
                    _ => 0,
                };
                if rank > 0 && best.map(|(_, best_rank)| rank > best_rank).unwrap_or(true) {
                    best = Some((current, rank));
                }
            }
            stack.push(current);
            walked += 1;
            if walked > 2048 {
                break;
            }
            child = unsafe { GetWindow(current, GW_HWNDNEXT) }.ok();
        }
    }
    best.map(|(hwnd, _)| hwnd)
}

/// Forwards a short click to the widget's webview so its content (buttons,
/// links) reacts — widgets sit behind the icons layer and never receive
/// real input directly.
fn forward_click(app: &AppHandle, host: &HostState, widget_id: i64, cursor: POINT) {
    // Bare (top-level) widgets receive real clicks natively — never send a
    // duplicate synthetic one.
    if point_over_widget_window(app, host, cursor) {
        return;
    }
    let label = {
        let Ok(inner) = host.inner.lock() else {
            return;
        };
        inner
            .widgets
            .get(&widget_id)
            .map(|widget| widget.label.clone())
    };
    let Some(label) = label else {
        return;
    };
    let Some(window) = app.get_webview_window(&label) else {
        return;
    };
    let Ok(widget_hwnd) = window.hwnd() else {
        return;
    };
    let Some(target) = find_webview_input_hwnd(widget_hwnd) else {
        log_line(&format!(
            "widget click: no webview input window for id={widget_id}"
        ));
        return;
    };
    let mut point = cursor;
    if !unsafe { ScreenToClient(target, &mut point) }.as_bool() {
        return;
    }
    let lparam = ((point.y as isize & 0xFFFF) << 16) | (point.x as isize & 0xFFFF);
    unsafe {
        let _ = PostMessageW(Some(target), WM_MOUSEMOVE, WPARAM(0), LPARAM(lparam));
        thread::sleep(Duration::from_millis(40));
        let _ = PostMessageW(Some(target), WM_LBUTTONDOWN, WPARAM(0x0001), LPARAM(lparam));
        thread::sleep(Duration::from_millis(30));
        let _ = PostMessageW(Some(target), WM_LBUTTONUP, WPARAM(0), LPARAM(lparam));
    }
    log_line(&format!(
        "widget click: forwarded id={widget_id} at {},{}",
        point.x, point.y
    ));
}

/// Forwards the cursor to interactive wallpaper surfaces (physical,
/// monitor-relative coordinates). Skips work when the cursor has not moved.
fn bridge_cursor(
    app: &AppHandle,
    host: &HostState,
    cursor: POINT,
    last: &mut Option<POINT>,
    monitors: &mut Option<(Instant, Vec<(i32, i32, u32, u32)>)>,
) {
    if let Some(previous) = last {
        if previous.x == cursor.x && previous.y == cursor.y {
            return;
        }
    }
    *last = Some(cursor);

    let surfaces: Vec<(String, u32)> = {
        let Ok(inner) = host.inner.lock() else {
            return;
        };
        inner
            .surfaces
            .values()
            .filter(|surface| surface.kind == "interactive")
            .map(|surface| (surface.label.clone(), surface.monitor_index))
            .collect()
    };
    if surfaces.is_empty() {
        return;
    }

    let stale = monitors
        .as_ref()
        .map(|(at, _)| at.elapsed().as_secs() >= 3)
        .unwrap_or(true);
    if stale {
        let list = app
            .available_monitors()
            .unwrap_or_default()
            .iter()
            .map(|monitor| {
                let position = monitor.position();
                let size = monitor.size();
                (position.x, position.y, size.width, size.height)
            })
            .collect();
        *monitors = Some((Instant::now(), list));
    }
    let Some((_, list)) = monitors.as_ref() else {
        return;
    };

    for (label, index) in surfaces {
        let Some((x, y, width, height)) = list.get(index as usize) else {
            continue;
        };
        let rel_x = cursor.x - x;
        let rel_y = cursor.y - y;
        if rel_x < 0 || rel_y < 0 || rel_x >= *width as i32 || rel_y >= *height as i32 {
            continue;
        }
        if let Some(window) = app.get_webview_window(&label) {
            let _ = window.eval(&format!(
                "window.__gsHost && window.__gsHost.cursor({rel_x}, {rel_y});"
            ));
        }
    }
}

/// Polls the mouse: drags widget windows when the desktop itself is clicked
/// — widgets live behind the icons layer, so they cannot receive clicks
/// directly — and feeds the cursor to interactive wallpaper surfaces.
fn drag_watcher(app: AppHandle) {
    thread::spawn(move || {
        let mut pressing: Option<(DragState, POINT, Instant)> = None;
        let mut dragging: Option<DragState> = None;
        let mut button_was_down = false;
        let mut cursor_last: Option<POINT> = None;
        let mut monitor_cache: Option<(Instant, Vec<(i32, i32, u32, u32)>)> = None;
        let mut interval = DRAG_POLL_MS;
        loop {
            thread::sleep(Duration::from_millis(interval));
            let host = app.state::<HostState>();
            let state = app.state::<AppState>();
            let button_down =
                (unsafe { GetAsyncKeyState(VK_LBUTTON.0 as i32) } as u16 & 0x8000) != 0;
            let mut cursor = POINT::default();
            if unsafe { GetCursorPos(&mut cursor) }.is_err() {
                continue;
            }
            bridge_cursor(&app, &host, cursor, &mut cursor_last, &mut monitor_cache);

            if button_down && !button_was_down {
                if dragging.is_none() && pressing.is_none() {
                    if let Some(drag) = begin_drag(&app, &host, cursor) {
                        pressing = Some((drag, cursor, Instant::now()));
                    }
                }
            } else if button_down {
                if let Some(active) = dragging {
                    drag_move(&app, &host, &active, cursor);
                } else if let Some((drag, start, _)) = pressing {
                    if (cursor.x - start.x).abs() + (cursor.y - start.y).abs() > DRAG_THRESHOLD_PX {
                        dragging = Some(drag);
                        pressing = None;
                        drag_move(&app, &host, &drag, cursor);
                    }
                }
            } else if !button_down && button_was_down {
                if let Some(active) = dragging.take() {
                    finish_drag(&app, &host, &state, &active, cursor);
                } else if let Some((drag, start, at)) = pressing.take() {
                    let moved = (cursor.x - start.x).abs() + (cursor.y - start.y).abs()
                        > DRAG_THRESHOLD_PX;
                    // A quick press that barely moved is a click on the
                    // widget's content, not a drag.
                    if !moved && at.elapsed().as_millis() < 900 {
                        forward_click(&app, &host, drag.widget_id, cursor);
                    }
                }
            }
            button_was_down = button_down;
            interval = if dragging.is_some()
                || pressing.is_some()
                || has_interactive_surfaces(&host)
            {
                DRAG_POLL_FAST_MS
            } else {
                DRAG_POLL_MS
            };
        }
    });
}

/* ------------------------------------------------------------------ */
/* Widgets                                                             */
/* ------------------------------------------------------------------ */

fn create_widget_window(
    app: &AppHandle,
    layer: &LayerHost,
    monitor: &Monitor,
    row: &storage::WidgetRow,
) -> Result<WidgetWindow, String> {
    let label = format!("gs-widget-{}", row.id);
    release_window_label(app, &label);

    // Clock and text widgets float without a card: the page paints on pure
    // black, which becomes a colour key so only the content shows.
    let bare = row.kind == "clock" || row.kind == "text";

    let page = format!("widget.html?kind={}&id={}", url_encode(&row.kind), row.id);
    let window = WebviewWindowBuilder::new(app, label.as_str(), WebviewUrl::App(page.into()))
        .title("GameScape Widget")
        .decorations(false)
        .resizable(false)
        .skip_taskbar(true)
        .focused(false)
        .visible(false)
        // Transparent windows need the webview's own background cleared.
        .transparent(bare)
        .background_color(if bare {
            Color(0, 0, 0, 0)
        } else {
            Color(10, 15, 20, 255)
        })
        .build()
        .map_err(|error| format!("cannot create a widget window: {error}"))?;

    let hwnd = window
        .hwnd()
        .map_err(|error| format!("widget has no window handle: {error}"))?;

    let position = monitor.position();
    let x = position.x + row.x;
    let y = position.y + row.y;
    let width = row.width.max(80);
    let height = row.height.max(60);

    if bare {
        // Boxless widgets: a top-level window with no redirection bitmap —
        // WebView2 composites it with per-pixel alpha (the page background
        // is transparent). Anchored at the bottom of the z-order: above the
        // wallpaper, below every normal window, and natively clickable.
        unsafe {
            let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            SetWindowLongPtrW(
                hwnd,
                GWL_EXSTYLE,
                ex | (WS_EX_NOREDIRECTIONBITMAP.0 as isize)
                    | (WS_EX_TOOLWINDOW.0 as isize)
                    | (WS_EX_NOACTIVATE.0 as isize),
            );
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_BOTTOM),
                x,
                y,
                width as i32,
                height as i32,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        }
    } else {
        match attach_surface(hwnd, layer, (x, y, width, height)) {
            Ok(true) => {}
            Ok(false) => {
                let _ = window.destroy();
                return Err("widget did not attach to the desktop layer".to_string());
            }
            Err(error) => {
                let _ = window.destroy();
                return Err(error);
            }
        }
    }

    log_line(&format!(
        "widget[{}] id={} hwnd=0x{:X} placed monitor={} rect={},{} {}x{}{}",
        row.kind,
        row.id,
        hwnd.0 as usize,
        row.monitor + 1,
        row.x,
        row.y,
        row.width,
        row.height,
        if bare { " bare" } else { "" }
    ));

    Ok(WidgetWindow {
        label,
        kind: row.kind.clone(),
        monitor: row.monitor,
        x: row.x,
        y: row.y,
        width: row.width,
        height: row.height,
        settings: row.settings.clone(),
    })
}

/// Brings the desktop layer in line with the widgets table. Takes the
/// serial lock so it cannot interleave with widget commands.
pub fn sync_widgets(app: &AppHandle, host: &HostState, state: &AppState) -> Result<usize, String> {
    let _serial = host.serial_lock()?;
    let rows: Vec<storage::WidgetRow> = {
        let conn = state
            .db
            .lock()
            .map_err(|e| format!("database lock poisoned: {e}"))?;
        storage::list_widgets(&conn)?
    };
    sync_widgets_rows(app, host, &rows)
}

/// Syncs from an already-read row snapshot; the caller must hold the serial
/// lock (widget commands take it around their write + this call).
pub fn sync_widgets_rows(
    app: &AppHandle,
    host: &HostState,
    rows: &[storage::WidgetRow],
) -> Result<usize, String> {
    let enabled: Vec<storage::WidgetRow> = rows
        .iter()
        .filter(|row| row.enabled)
        .cloned()
        .collect();

    let monitors = app
        .available_monitors()
        .map_err(|error| format!("cannot list monitors: {error}"))?;
    let layer = if enabled.is_empty() {
        None
    } else {
        Some(discover_layer()?)
    };

    let existing = {
        let mut inner = host
            .inner
            .lock()
            .map_err(|e| format!("host state lock poisoned: {e}"))?;
        std::mem::take(&mut inner.widgets)
    };

    let mut next: HashMap<i64, WidgetWindow> = HashMap::new();
    for row in &enabled {
        if row.monitor as usize >= monitors.len() {
            log_line(&format!(
                "widget[{}] id={} kept off: monitor {} does not exist",
                row.kind,
                row.id,
                row.monitor + 1
            ));
            continue;
        }
        let window = match existing.get(&row.id) {
            Some(window) if window.matches(row) => window.clone(),
            Some(stale) => {
                if let Some(window) = app.get_webview_window(&stale.label) {
                    let _ = window.destroy();
                }
                create_widget_window(app, layer.as_ref().unwrap(), &monitors[row.monitor as usize], row)?
            }
            None => {
                create_widget_window(app, layer.as_ref().unwrap(), &monitors[row.monitor as usize], row)?
            }
        };
        next.insert(row.id, window);
    }

    for (id, window) in &existing {
        if !next.contains_key(id) {
            if let Some(stale) = app.get_webview_window(&window.label) {
                let _ = stale.destroy();
            }
        }
    }

    let placed = next.len();
    {
        let mut inner = host
            .inner
            .lock()
            .map_err(|e| format!("host state lock poisoned: {e}"))?;
        inner.widgets = next;
    }
    log_line(&format!("widgets synced: {placed} on the desktop"));
    let _ = app.emit("host:state", ());
    Ok(placed)
}

/* ------------------------------------------------------------------ */
/* Rotation                                                            */
/* ------------------------------------------------------------------ */

/// Next position in a rotating list; wraps around, restarts when the
/// current item is not in the list.
fn next_rotation_position(len: usize, current: Option<usize>) -> usize {
    match current {
        Some(position) if position + 1 < len => position + 1,
        _ => 0,
    }
}

/// The rotation interval in seconds; local harnesses may shorten it via
/// `GAMESCAPE_ROTATION_SECONDS` (dev-only, never set in normal use).
fn rotation_interval_seconds(minutes: u32) -> u64 {
    if let Ok(seconds) = std::env::var("GAMESCAPE_ROTATION_SECONDS") {
        if let Ok(value) = seconds.parse::<u64>() {
            if value > 0 {
                return value;
            }
        }
    }
    u64::from(minutes.clamp(1, 24 * 60)) * 60
}

/// One rotation check: when enabled and due, apply the next library item.
fn rotation_tick(app: &AppHandle, host: &HostState, state: &AppState) {
    let (enabled, minutes, wallpapers_on, current_id) = {
        let Ok(settings) = state.settings.lock() else {
            return;
        };
        (
            settings.rotation_enabled.unwrap_or(false),
            settings.rotation_minutes.unwrap_or(15),
            settings.wallpaper_enabled.unwrap_or(true),
            settings.last_wallpaper.as_ref().map(|last| last.media_id),
        )
    };
    if !enabled {
        return;
    }
    // The master switch (Turn off) silences rotation too.
    if !wallpapers_on {
        return;
    }

    let interval = rotation_interval_seconds(minutes);
    let due = {
        let Ok(inner) = host.inner.lock() else {
            return;
        };
        match inner.last_rotation {
            Some(last) => last.elapsed().as_secs() >= interval,
            None => false,
        }
    };
    if !due {
        return;
    }

    let candidates: Vec<(i64, String)> = {
        let Ok(conn) = state.db.lock() else {
            return;
        };
        let Ok(mut statement) =
            conn.prepare("SELECT id, path FROM media WHERE path IS NOT NULL ORDER BY id")
        else {
            return;
        };
        let Ok(rows) = statement
            .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)))
        else {
            return;
        };
        rows.filter_map(|row| row.ok())
            .filter(|(_, path)| std::path::Path::new(path).exists())
            .collect()
    };
    if candidates.len() < 2 {
        // Nothing to rotate through; check again next interval.
        if let Ok(mut inner) = host.inner.lock() {
            inner.last_rotation = Some(Instant::now());
        }
        return;
    }

    let position = current_id.and_then(|id| candidates.iter().position(|(cid, _)| *cid == id));
    let next = next_rotation_position(candidates.len(), position);
    match apply_library_item(app, state, host, candidates[next].0, None) {
        Ok(kind) => log_line(&format!(
            "rotation: applied {kind} ({} of {})",
            next + 1,
            candidates.len()
        )),
        Err(error) => {
            log_line(&format!("rotation: apply failed: {error}"));
            if let Ok(mut inner) = host.inner.lock() {
                inner.last_rotation = Some(Instant::now());
            }
        }
    }
}

/* ------------------------------------------------------------------ */
/* Self-healing layer monitor                                          */
/* ------------------------------------------------------------------ */

pub fn init(app: &AppHandle) {
    // Log the monitor list once: harnesses map screen coordinates to the
    // app's monitor indexes from these lines.
    if let Ok(monitors) = app.available_monitors() {
        for (index, monitor) in monitors.iter().enumerate() {
            let position = monitor.position();
            let size = monitor.size();
            log_line(&format!(
                "monitor[{index}]: origin {},{} size {}x{}",
                position.x, position.y, size.width, size.height
            ));
        }
    }

    let handle = app.clone();
    drag_watcher(handle.clone());
    thread::spawn(move || {
        log_line("governor: active (fullscreen + battery watch)");
        loop {
            thread::sleep(Duration::from_secs(2));

            let host = handle.state::<HostState>();
            let state = handle.state::<AppState>();
            governor_tick(&handle, &host);
            rotation_tick(&handle, &host, &state);

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
            let specs: Vec<(i64, String, String, String, String)> = {
                let Ok(inner) = host.inner.lock() else { continue };
                inner
                    .surfaces
                    .values()
                    .map(|surface| {
                        (
                            surface.media_id,
                            surface.media_path.clone(),
                            surface.kind.clone(),
                            surface.title.clone(),
                            surface.fit.clone(),
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
            let start_paused = host
                .inner
                .lock()
                .map(|inner| inner.manual_paused || inner.governor_paused)
                .unwrap_or(false);
            let mut created: Vec<Surface> = Vec::new();
            for (index, monitor) in monitors.iter().enumerate() {
                if let Some((media_id, path, kind, title, fit)) = specs.get(index) {
                    match create_surface(
                        &handle,
                        monitor,
                        index,
                        *media_id,
                        path,
                        kind,
                        title,
                        fit,
                        start_paused,
                        &layer,
                    ) {
                        Ok(surface) => created.push(surface),
                        Err(error) => {
                            log_line(&format!("re-attach failed on monitor {index}: {error}"))
                        }
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
            let _ = apply_pause_state(&handle, &host);

            // Widgets live on the same layer; recreate them from the DB.
            if let Ok(mut inner) = host.inner.lock() {
                inner.widgets.clear();
            }
            if let Err(error) = sync_widgets(&handle, &host, &state) {
                log_line(&format!("widget re-attach failed: {error}"));
            }
        }
    });
}

/* ------------------------------------------------------------------ */
/* Startup restore                                                     */
/* ------------------------------------------------------------------ */

/// True when local test harnesses drive this instance — they must not
/// inherit a "restore the last wallpaper" surprise at launch.
fn dev_environment() -> bool {
    if std::env::var("GAMESCAPE_DEV_ALLOW_RESTORE").is_ok() {
        return false;
    }
    [
        "GAMESCAPE_START_HASH",
        "GAMESCAPE_DEV_SCRIPT",
        "GAMESCAPE_DEV_IMPORT",
        "GAMESCAPE_SPIKE_OUT",
        "GAMESCAPE_SPIKE_S1",
        "GAMESCAPE_SPIKE_INTERACTIVE",
    ]
    .iter()
    .any(|name| std::env::var(name).map(|value| !value.is_empty()).unwrap_or(false))
}

/// Re-applies the last wallpaper a few seconds after launch (unless the
/// user turned wallpapers off or a test harness is driving the app).
pub fn startup_restore(app: &AppHandle) {
    let handle = app.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(4));
        if dev_environment() {
            return;
        }

        let state = handle.state::<AppState>();
        let host = handle.state::<HostState>();

        // Widgets are independent of wallpaper state; place them first.
        if let Err(error) = sync_widgets(&handle, &host, &state) {
            log_line(&format!("startup widget sync failed: {error}"));
        }

        let (enabled, last) = {
            let Ok(settings) = state.settings.lock() else {
                return;
            };
            (
                settings.wallpaper_enabled.unwrap_or(true),
                settings.last_wallpaper.clone(),
            )
        };
        if !enabled {
            log_line("startup restore: skipped (wallpapers turned off)");
            return;
        }
        let Some(last) = last else {
            return;
        };
        if last.kind != "interactive" && !std::path::Path::new(&last.path).exists() {
            log_line("startup restore: skipped (file missing)");
            return;
        }

        let result = if last.kind == "static" {
            apply_static(state.inner(), &last.path, last.format.as_deref(), last.monitor).map(|_| ())
        } else {
            let (fit_name, _) = configured_fit(state.inner());
            apply_animated(
                &handle,
                host.inner(),
                &last.path,
                &last.kind,
                &last.title,
                last.media_id,
                fit_name,
                last.monitor,
            )
        };
        match result {
            Ok(()) => log_line(&format!("startup restore: {} {}", last.kind, last.title)),
            Err(error) => log_line(&format!("startup restore failed: {error}")),
        }
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

/* ------------------------------------------------------------------ */
/* Tests                                                               */
/* ------------------------------------------------------------------ */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fullscreen_pause_needs_cover_without_caption() {
        // Borderless game covering the monitor: pause.
        assert!(should_pause_for_fullscreen(true, false, false, false));
        // Normal maximized app (has a caption): never pause.
        assert!(!should_pause_for_fullscreen(true, true, false, false));
        // Windowed, not covering: no pause.
        assert!(!should_pause_for_fullscreen(false, false, false, false));
        // Our own window and shell surfaces never count.
        assert!(!should_pause_for_fullscreen(true, false, true, false));
        assert!(!should_pause_for_fullscreen(true, false, false, true));
    }

    #[test]
    fn governor_reason_priority() {
        assert_eq!(governor_reason(true, true), Some("fullscreen app"));
        assert_eq!(governor_reason(false, true), Some("on battery"));
        assert_eq!(governor_reason(false, false), None);
    }

    #[test]
    fn fit_mode_defaults_to_fill_screen() {
        assert_eq!(fit_mode(None).0, "fill");
        assert_eq!(fit_mode(Some("FILL")).0, "fill");
        assert_eq!(fit_mode(Some("fit")).0, "fit");
        assert_eq!(fit_mode(Some("stretch")).0, "stretch");
        assert_eq!(fit_mode(Some("center")).0, "center");
        assert_eq!(fit_mode(Some("span")).0, "span");
        assert_eq!(fit_mode(Some("bogus")).0, "fill");
        assert_eq!(fit_mode(None).1, DWPOS_FILL);
        assert_eq!(position_name(4), "fill");
        assert_eq!(position_name(-1), "unknown");
    }

    #[test]
    fn rotation_wraps_and_restarts() {
        assert_eq!(next_rotation_position(3, None), 0);
        assert_eq!(next_rotation_position(3, Some(0)), 1);
        assert_eq!(next_rotation_position(3, Some(2)), 0);
        assert_eq!(next_rotation_position(3, Some(9)), 0);
        assert_eq!(next_rotation_position(0, Some(0)), 0);
    }
}
