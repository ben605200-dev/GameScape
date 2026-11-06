# Spike S1 / S2-lite Results — Desktop-layer attach (WebView2 behind desktop icons)

- **Status:** ✅ Passed (classic topology, per-monitor, negative monitor coordinates)
- **Date:** 2026-11-06
- **Phase:** 0 (verification gates)
- **Related:** ADR-0001, Phase 0 spike plan (S1–S3)

## Question

Can a Tauri/WebView2 window render *behind desktop icons* by being re-parented into the shell's wallpaper layer (`WorkerW`) — including one window per monitor, positioned correctly on a multi-monitor arrangement that includes negative coordinates?

## Environment

- Windows 11 23H2 (build 22631) — **classic** desktop topology
  (`Progman` without `WS_EX_NOREDIRECTIONBITMAP`)
- Two 1920×1080 monitors: primary at (0,0); secondary at (−1920,5)
- Tauri 2.12.5 / wry 0.57.1 / WebView2 (Evergreen runtime)
- Rust stable (MSVC), dev profile

## Method

1. `GAMESCAPE_SPIKE_S1=1` creates one borderless WebView2 window per monitor
   (the app's main window stays hidden).
2. Shell discovery: `Progman` → topology detection → `SHELLDLL_DefView` host →
   sibling `WorkerW` wallpaper layer.
3. Each window: `WS_POPUP`→`WS_CHILD` style swap, `WS_EX_LAYERED` with full
   alpha (WebView2 compositing), `SetParent`, then `SetWindowPos` at the
   monitor rectangle translated into the layer's client coordinates.
4. **Mechanical verification:** walk the layer's actual child chain and confirm
   each window is present (pixels can be obscured; hierarchy cannot).
5. Harness captures two desktop screenshots ~4 s apart, then tears everything
   down.

> Methodology note: the first attempt attached only to the primary monitor,
> which was fully covered by other application windows at capture time —
> making the visual evidence inconclusive. The test now attaches to all
> monitors, which is both the correct product architecture and guarantees an
> uncovered surface for evidence.

## Results

| Check | Result |
|---|---|
| Attach to wallpaper layer (both monitors) | ✅ `parent_child_check=true` |
| Monitor rectangle translation (incl. negative x) | ✅ client (0,5) and (1920,0) |
| Layered compositing style applied | ✅ `WS_EX_LAYERED` present |
| Renders behind desktop icons (visual) | ✅ icons + taskbar visible above the animated surface |
| Surface is live (not a frozen frame) | ✅ clock advanced between captures |
| Clean teardown (no orphan processes; desktop restored) | ✅ |

Abridged step log (identifiers trimmed):

    window[0] created (hwnd=…, rect=-1920,5 1920x1080)
    window[1] created (hwnd=…, rect=0,0 1920x1080)
    Progman=…; topology=classic
    attaching 2 window(s): parent=… origin=(-1920, 0) raised=false
    window[0] rect=(0,5 1920x1080) child=true layered=true
    window[1] rect=(1920,0 1920x1080) child=true layered=true
    ATTACHED_OK

Evidence screenshots are kept locally (`.dev-data/spike-s1/`, git-ignored) and
are deliberately **not committed** — they show a real desktop.

## What this de-risks

- The most uncertain product feature — animated wallpapers behind icons via
  WebView2 — works on the classic topology, including the multi-monitor
  negative-coordinate case explicitly flagged in the plan (S2).
- Per-monitor windows with client-space translation are validated as the shape
  for the real `desktop_host` subsystem.

## Known limitations and follow-ups

- **Raised topology (Windows 11 24H2+) not yet exercised on real hardware.**
  The code path exists (parent to `Progman`, slot below the icon layer) but
  requires a 24H2+ machine to validate — test-matrix item.
- **Explorer-restart self-healing not yet tested** (S1 criterion: re-attach
  ≤ 5 s) — scheduled as the next spike test.
- Input forwarding (mouse/keyboard into the wallpaper surface) is intentionally
  out of scope; wallpapers are non-interactive by design (ADR-0004 direction).
- The spike positions windows using `GetWindowRect` origin math; production
  code should move to `MapWindowPoints` for exactness across unusual shells
  and DPI configurations.
