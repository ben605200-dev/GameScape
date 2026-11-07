# @gamescape/desktop

The GameScape Windows application.

**Stack:** Tauri v2 · Rust core (`src-tauri/`) · React + TypeScript + Vite (`src/`)

**Status:** Phase 5 — the desktop host applies wallpapers for real: static images through the documented Windows wallpaper API (always fitted to the screen; previous wallpaper and fit restorable), animated GIF/MP4 through per-monitor desktop-layer surfaces with self-healing, an auto-pause governor for fullscreen apps and battery, plus everything from Phases 2–4 (library, media pipeline, interactive previews). A per-user installer builds with `tauri build` (NSIS, v0.5.0), the wallpaper last applied is restored on launch, rotation can cycle your library on a schedule, every wallpaper can be turned off with one click, the app lives in the system tray, and every screen can run its own wallpaper (per-monitor assignment). Phase 5's desktop host is feature-complete; the next phase takes on the widget layer, and the download manager remains deferred to the hosting phase. Tauri v2.12, React 19, Vite 8, react-router 7. Structure:

```
src/          React UI (app shell, features, components, lib)
src-tauri/    Rust core (commands, core services, desktop_host, storage, security)
```

Key subsystems (see `docs/engineering/` as it migrates in): desktop host (per-monitor wallpaper + widget windows), governor (fullscreen/battery pausing), library/import pipeline, download manager, wallpaper applier, layouts.

Do not run the app's desktop-host features on machines where you cannot afford shell experiments; development is safe (static mode is the fallback), but Phase 0 spikes are the designated test path.
