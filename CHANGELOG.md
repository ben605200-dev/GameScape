# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Phase 1 foundations: monorepo workspace (pnpm + Turborepo), root hygiene files.
- Privacy gate: `scripts/privacy-scan.mjs` and CI workflow.
- Community standards gate: `scripts/community-standards-check.mjs` and CI workflow.
- Community files: README, LICENSE (Apache-2.0), NOTICE, CONTRIBUTING, CODE_OF_CONDUCT (Contributor Covenant 2.1), SECURITY, SUPPORT, issue and PR templates.
- Desktop app scaffold: Tauri v2 + React 19 + Vite 8 (`apps/desktop`), branded for GameScape and integrated into the monorepo. Rust side compiles clean (`cargo check`); frontend builds clean.
- Phase 0 spike S1/S2-lite **passed** — WebView2 surfaces re-parented into the Windows desktop layer (behind desktop icons): classic topology, two monitors with a negative-coordinate arrangement, live rendering visually verified, clean teardown. See `docs/engineering/spike-s1-results.md`.
- Design tokens package (`@gamescape/ui`) — dark/light themes, typography, spacing, radius and motion tokens consumed by the app.
- Desktop app shell: sidebar navigation (9 sections), top bar with theme switcher, Home dashboard with live status cards; theme preference persisted through the settings store.
- Local storage foundation: JSON settings store and a SQLite database with versioned migrations under `%APPDATA%\GameScape`.
- Bundled seed catalogue (fictional preview data) with Rust-side validation, powering the Home shelf and the Wallpapers grid.
- ADR-0002: shell styling approach (tokens-first; Tailwind adopted with the Phase 3 screens).
- Interactive wallpaper prototype **validated on the desktop**: a cursor-reactive light field renders behind desktop icons, driven by a global-cursor bridge in the core (no input capture, no focus stealing). Evidence and limitations in `docs/engineering/spike-s1-results.md`.
- Wallpaper detail pages: clickable cards, full metadata, and a live pointer-reactive preview for interactive entries. The `interactive` flag now lives in the catalogue schema, the Rust model, and the seed.
- Fixed: the spike launch gate now recognises the interactive prototype mode.
- Personal Library **import pipeline** (Phase 3 start): file-picker and drag-and-drop imports for JPG/PNG/WEBP/GIF with magic-byte validation, SHA-256 duplicate detection, decode probing, managed copies in the library folder, JPEG thumbnails, and per-file result reporting (imported / duplicate / unsupported / invalid / corrupted / failed).
- Library page: real grid with thumbnails served through the Tauri asset protocol, per-item removal, and live import progress rows.
- Dev automation hooks (`GAMESCAPE_DEV_IMPORT`, `GAMESCAPE_DEV_FAVOURITE`, `GAMESCAPE_START_HASH`) for local test harnesses — never active in normal use.
- Catalogue **search, filters and sorting**: instant search across titles/games/tags; filters for game, media type, orientation and resolution (both derived from dimensions); newest/A-Z sorting; live result count and clear-all.
- **Favourites**: star toggle on cards and detail pages, persisted in SQLite, with a working Favourites page.
- Seed catalogue expanded from 5 to 14 fictional entries across 4 games and all three media types — including portrait and ultrawide layouts so filters have real variety.
- Shared `WallpaperCard` component now used by Wallpapers, Favourites and the Home shelf.
- Interactive previews are now **per-wallpaper**: an `effect` descriptor (`orbit` / `ribbon` / `grid`) rides in the catalogue schema (validated in the Rust model) and each effect renders tinted with the wallpaper's own placeholder palette. Fixes a gap where every interactive preview rendered the same hardcoded effect.
- **Fixed: the content area could not scroll** — tall pages (e.g. the wallpapers grid) were clipped with no scrollbar. Root cause: a flex/grid `min-height: auto` trap where the main column grew to content height instead of staying viewport-bound. The layout chain now sets `min-height: 0`; the sidebar also scrolls in short windows. Verified in-window with a scrolled capture.
- Dev hook `GAMESCAPE_DEV_SCRIPT` for window-state verification harnesses — never active in normal use.
- **All wallpapers are now interactive**: every catalogue entry carries an effect descriptor (seed mix: orbit ×5, ribbon ×5, grid ×4), the detail view renders the live cursor-reactive preview for **every** wallpaper (explicit opt-out via `interactive: false` remains), cards show each wallpaper's effect style instead of a generic badge, and entries without explicit effect data fall back to a deterministic style.
- **Metadata editing**: library item detail page — rename imports, edit descriptions and tags (validated server-side), with file info (dimensions, size, SHA-256, path), *Show in Explorer*, and a safe remove with an optional permanent file deletion.
- **Collections**: create collections, filter the library by collection, and tick memberships from any item — counts update live; deleting a collection keeps the wallpapers.
- **Integrity tools**: *Check integrity* scans every managed file (missing / changed detection) with per-item actions — *Locate file…* (relink: re-validated, re-copied and re-thumbnailed like an import) and *Remove entry*.
- **Library location setting**: view, change or reset the managed library folder (new imports go there; existing files stay put).
- Fixed: settings saves now **merge** instead of replacing — toggling the theme can no longer wipe the library directory setting.
- **Phase 4 media pipeline**: MP4 imports with container probing (duration, dimensions, audio-track detection via a pure-Rust MP4 parser); GIF analysis (frame count + loop duration); schema v2 (`frame_count`, `has_audio` columns); real **video playback** (controls, loop, muted autoplay) and **animated GIF playback** on library item pages; **automatic poster-frame generation** for videos (first playable frame captured in the WebView, validated and stored by the core); reduced-motion respect; and the **first Rust unit tests** (GIF analysis, MP4 detection, duration formatting).
- Fixed: video poster frames now capture correctly — cross-origin asset-protocol media **tainted the capture canvas** and blocked `toDataURL`. Posters now load through a same-origin, size-capped data pipeline from the core (`library_read_data`), and the library grid refreshes the moment a poster lands.
- **Phase 5 desktop host (first slice)**: real wallpaper application — static images via the documented `IDesktopWallpaper` COM API (per-monitor, Fill position, WebP→PNG conversion, previous-wallpaper restore stored in schema v3); animated GIF/MP4 wallpapers render in **per-monitor host windows on the desktop layer** (classic + raised topologies) with self-healing re-attach after explorer restarts and clean teardown on exit; **"Set as desktop wallpaper"** on every library item; live Desktop-layer card on Home with Pause/Resume, Stop and Restore-previous-wallpaper controls.
