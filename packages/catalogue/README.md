# @gamescape/catalogue

The public wallpaper catalogue: the contract between the desktop app, the website, and the intake tooling.

**Status:** schema and tooling pending (Phase 3/7 in the roadmap).

Planned contents:

```
schema/        JSON Schema (source of truth for entry validation)
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
