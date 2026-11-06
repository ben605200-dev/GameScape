# ADR-0002: Shell v0 styling — hand-authored CSS on the token system

- **Status:** Accepted
- **Date:** 2026-11 (Phase 2)

## Context

The plan (Document 6) specifies Tailwind CSS + a component kit (shadcn/ui) as the styling approach for the desktop app and website. Phase 2 delivered the first real screens: the app shell (sidebar, top bar, nine placeholder screens, Home dashboard with live data).

At this stage:

- The layout is still changing rapidly (navigation structure, card patterns, density).
- The design tokens (`packages/ui`) are the actual source of visual truth; Tailwind would consume the same tokens.
- Introducing Tailwind + a component kit mid-session would add configuration surface and dependency risk with no user-visible benefit while the layout is in flux.

## Decision

Ship shell v0 with hand-authored CSS built **exclusively on the `@gamescape/ui` design tokens** (no hard-coded colours). Adopt Tailwind CSS (and a component kit where it earns its place) when the first data-heavy feature screens are built (Phase 3), where utility classes pay off in volume and the card/grid patterns have stabilised.

## Consequences

- **Positive:** tokens remain the single source of truth; zero extra dependencies at a churn-heavy stage; every future styling layer starts from the same variables.
- **Negative:** some CSS will be rewritten (or wrapped) when Tailwind lands; contributors must follow the token rule for shell CSS in the meantime.
- **Guardrail:** `apps/desktop/src/styles/app.css` must not contain raw colour values — a design-review rule (not covered by the privacy scan).
