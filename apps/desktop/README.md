# @gamescape/desktop

The GameScape Windows application.

**Stack:** Tauri v2 · Rust core (`src-tauri/`) · React + TypeScript + Vite (`src/`)

**Status:** Phase 6 underway — the desktop widget layer builds on the same self-healing desktop foundation as the wallpapers: Clock, Calendar, Countdown, Text (custom font and size), System monitor and Wallpaper status widgets with per-monitor positions, edge-clean rendering and mouse dragging on the desktop, managed from the Widgets page. Catalogue entries render into real, applicable wallpaper art on demand. Phase 5 is complete: static + animated wallpapers always fitted to the screen (previous wallpaper and fit restorable), an auto-pause governor for fullscreen apps and battery, scheduled rotation, startup restore, per-monitor assignment, one-click turn off, a system tray, and a per-user installer (NSIS, v0.5.0). The download manager remains deferred to the hosting phase. Tauri v2.12, React 19, Vite 8, react-router 7. Structure:

```
src/          React UI (app shell, features, components, lib)
src-tauri/    Rust core (commands, core services, desktop_host, storage, security)
```

Key subsystems (see `docs/engineering/` as it migrates in): desktop host (per-monitor wallpaper + widget windows), governor (fullscreen/battery pausing), library/import pipeline, download manager, wallpaper applier, layouts.

Do not run the app's desktop-host features on machines where you cannot afford shell experiments; development is safe (static mode is the fallback), but Phase 0 spikes are the designated test path.
