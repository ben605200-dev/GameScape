# @gamescape/website

The public GameScape website (static, deployed to GitHub Pages).

**Stack:** Astro · React islands (gallery) · Tailwind CSS · content from `packages/catalogue`

**Status:** scaffold pending (Phase 8 in the roadmap; may be pulled earlier since it is independent).

Rules that always apply:

- The site is built **only** from this workspace plus approved public packages — never from local app data.
- No third-party scripts, fonts, or trackers (self-hosted assets only).
- The deploy artifact is validated by `scripts/privacy-scan.mjs` rules and the dist allowlist before deployment.
- Images/screenshots must be sanitised (fictional content, no personal data).
