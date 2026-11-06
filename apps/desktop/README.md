# @gamescape/desktop

The GameScape Windows application.

**Stack:** Tauri v2 · Rust core (`src-tauri/`) · React + TypeScript + Vite (`src/`)

**Status:** Phase 3 underway — import pipeline (file picker + drag-and-drop, validation, SHA-256 dedupe, thumbnails), Personal Library grid with live import progress, wallpaper detail pages with interactive previews, plus the Phase 2 foundation (shell, themes, settings store, SQLite with migrations, seed catalogue). Tauri v2.12, React 19, Vite 8, react-router 7. Structure:

```
src/          React UI (app shell, features, components, lib)
src-tauri/    Rust core (commands, core services, desktop_host, storage, security)
```

Key subsystems (see `docs/engineering/` as it migrates in): desktop host (per-monitor wallpaper + widget windows), governor (fullscreen/battery pausing), library/import pipeline, download manager, wallpaper applier, layouts.

Do not run the app's desktop-host features on machines where you cannot afford shell experiments; development is safe (static mode is the fallback), but Phase 0 spikes are the designated test path.
