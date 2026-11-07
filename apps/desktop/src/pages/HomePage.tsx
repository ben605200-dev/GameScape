import { useEffect, useState } from "react";
import { Database, Image, Layers, Monitor, Sparkles } from "lucide-react";
import { WallpaperCard } from "../components/WallpaperCard";
import {
  getDbHealth,
  getSettings,
  hostRestorePrevious,
  hostSetPaused,
  hostStatus,
  hostTurnOff,
  IN_TAURI,
  loadCatalogue,
  type CatalogueIndex,
  type DbHealth,
  type HostReport,
} from "../lib/ipc";

export function HomePage() {
  const [health, setHealth] = useState<DbHealth | null>(null);
  const [catalogue, setCatalogue] = useState<CatalogueIndex | null>(null);
  const [checked, setChecked] = useState(false);
  const [report, setReport] = useState<HostReport>({
    surfaces: [],
    applied: null,
    monitors: [],
  });
  const [busy, setBusy] = useState(false);
  const [rotation, setRotation] = useState<{ enabled: boolean; minutes: number } | null>(null);

  useEffect(() => {
    let cancelled = false;
    void Promise.all([getDbHealth(), loadCatalogue()]).then(([h, c]) => {
      if (cancelled) return;
      setHealth(h);
      setCatalogue(c);
      setChecked(true);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  // Live desktop-host state (polled; cheap).
  useEffect(() => {
    let cancelled = false;
    const poll = async () => {
      const next = await hostStatus();
      if (!cancelled) setReport(next);
    };
    void poll();
    const timer = window.setInterval(() => void poll(), 4000);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, []);

  // Rotation state for the desktop card note (loaded once).
  useEffect(() => {
    let cancelled = false;
    void getSettings().then((settings) => {
      if (!cancelled) {
        setRotation({
          enabled: settings.rotationEnabled ?? false,
          minutes: settings.rotationMinutes ?? 15,
        });
      }
    });
    return () => {
      cancelled = true;
    };
  }, []);

  const turnOffWallpaper = async () => {
    setBusy(true);
    await hostTurnOff();
    setReport(await hostStatus());
    setBusy(false);
  };

  const togglePaused = async () => {
    const next = !(report.surfaces[0]?.paused ?? false);
    setBusy(true);
    await hostSetPaused(next);
    setReport(await hostStatus());
    setBusy(false);
  };

  const restoreWallpaper = async () => {
    setBusy(true);
    await hostRestorePrevious();
    setReport(await hostStatus());
    setBusy(false);
  };

  const surfaces = report.surfaces;
  const applied = report.applied;
  const pausedSuffix =
    surfaces.length > 0 && surfaces[0].paused
      ? ` · paused${surfaces[0].pausedReason ? ` (${surfaces[0].pausedReason})` : ""}`
      : "";
  const distinctTitles = new Set(surfaces.map((surface) => surface.title)).size;
  const hostNote =
    surfaces.length > 0
      ? distinctTitles > 1
        ? `${distinctTitles} wallpapers on ${surfaces.length} monitors${pausedSuffix}`
        : `${surfaces[0].title} · ${surfaces.length} monitor${surfaces.length === 1 ? "" : "s"}${pausedSuffix}`
      : applied
        ? `${applied.title} · ${applied.kind} · fitted edge to edge`
        : "Nothing applied — set a wallpaper from your library.";
  const rotationNote = rotation?.enabled ? ` · rotates every ${rotation.minutes} min` : "";
  const featured = catalogue?.entries.slice(0, 5) ?? [];

  return (
    <div className="page">
      <div>
        <h1 className="page-title">Welcome to GameScape</h1>
        <p className="page-subtitle">
          Wallpapers that live on your desktop — static and animated, and they
          pause by themselves while you game.
        </p>
      </div>

      {!IN_TAURI && (
        <div className="row">
          <span className="chip">Browser preview — desktop data unavailable</span>
        </div>
      )}

      <div className="card-grid">
        <div className="card stat-card">
          <div className="stat-head">
            <Database size={15} aria-hidden="true" /> Storage
          </div>
          <div className="stat-value">
            {health ? `v${health.schemaVersion}` : checked ? "—" : "…"}
          </div>
          <div className="stat-note">
            {health
              ? `Database ready · ${health.mediaCount} library items`
              : "Local database connection"}
          </div>
        </div>

        <div className="card stat-card">
          <div className="stat-head">
            <Image size={15} aria-hidden="true" /> Catalogue
          </div>
          <div className="stat-value">
            {catalogue ? catalogue.entries.length : checked ? "—" : "…"}
          </div>
          <div className="stat-note">
            {catalogue
              ? `Seed index · schema ${catalogue.schemaVersion}`
              : "Catalogue loading"}
          </div>
        </div>

        <div className="card stat-card">
          <div className="stat-head">
            <Monitor size={15} aria-hidden="true" /> Desktop layer
          </div>
          <div className="stat-value">
            {surfaces.length > 0 ? "Animating" : applied ? "Wallpaper set" : "Idle"}
          </div>
          <div className="stat-note">
            {hostNote}
            {rotationNote}
          </div>
          {IN_TAURI && (
            <div className="row">
              {surfaces.length > 0 && (
                <button
                  type="button"
                  className="button"
                  onClick={() => void togglePaused()}
                  disabled={busy}
                >
                  {surfaces[0].paused ? "Resume" : "Pause"}
                </button>
              )}
              {(surfaces.length > 0 || applied) && (
                <button
                  type="button"
                  className="button"
                  onClick={() => void turnOffWallpaper()}
                  disabled={busy}
                >
                  Turn off
                </button>
              )}
              <button
                type="button"
                className="button"
                onClick={() => void restoreWallpaper()}
                disabled={busy}
              >
                Restore previous wallpaper
              </button>
            </div>
          )}
        </div>

        <div className="card stat-card">
          <div className="stat-head">
            <Layers size={15} aria-hidden="true" /> Build
          </div>
          <div className="stat-value">0.5.0</div>
          <div className="stat-note">Pre-alpha · Phase 5</div>
        </div>
      </div>

      {featured.length > 0 && (
        <div className="stack">
          <div className="row">
            <Sparkles size={15} aria-hidden="true" />
            <strong>From the seed catalogue</strong>
            <span className="chip chip-accent">preview data</span>
          </div>
          <div className="card-grid">
            {featured.map((entry) => (
              <WallpaperCard key={entry.id} entry={entry} />
            ))}
          </div>
        </div>
      )}

      <div className="card stack">
        <strong>What&apos;s next</strong>
        <div className="muted">
          Discovery, search and imports (Phase 3) · GIF &amp; video playback
          (Phase 4) · the animated desktop host (Phase 5) · widgets (Phase
          6–7) · website and community (Phase 8+).
        </div>
      </div>
    </div>
  );
}
