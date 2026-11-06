# @gamescape/catalogue

The public wallpaper catalogue: the contract between the desktop app, the website, and the intake tooling.

**Status:** schema v0.1 landed (`schema/catalogue.schema.json`). The app currently bundles a seed copy at `apps/desktop/src-tauri/assets/catalogue-seed.json`; Phase 3 moves the canonical seed here and adds the build/validation tooling, then the website consumes the generated index in Phase 8.

Planned contents:

```
schema/        JSON Schema (source of truth for entry validation)  ← v0.1 present
seed/          canonical seed index for the app and site
wallpapers/    one JSON entry per published wallpaper
thumbnails/    generated WebP/JPEG derivatives (grid, preview, OG, LQIP)
index/         generated index for app + website (committed, reviewable)
scripts/       validate, build index, hash media, generate derivatives
```

Hard rules:

- Only content with **documented rights** (original, CC0/CC BY-family, verified permission). No game art without a licence.
- Every entry carries licence metadata and, where required, an attribution string.
- Rights evidence (permission correspondence, personal data) lives **outside** the repository; entries reference verification state only.
- CI validates schema, uniqueness, thumbnails, and duplicate hashes on every change.

Field notes:

- `interactive` is reserved for cursor-reactive wallpapers (technique validated in the Phase 0 spike harness; ships with the desktop host in Phase 5).
- `schemaVersion` is a `major.minor` pair; breaking changes bump `major` and are coordinated with the app's minimum-version check.
