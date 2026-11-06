import { useEffect, useState } from "react";
import { Database, Image, Layers, Monitor, Sparkles } from "lucide-react";
import { WallpaperCard } from "../components/WallpaperCard";
import {
  getDbHealth,
  hostRestorePrevious,
  hostSetPaused,
  hostStatus,
  hostStop,
  IN_TAURI,
  loadCatalogue,
  type CatalogueIndex,
  type DbHealth,
  type HostSurface,
} from "../lib/ipc";

export function HomePage() {
  const [health, setHealth] = useState<DbHealth | null>(null);
  const [catalogue, setCatalogue] = useState<CatalogueIndex | null>(null);
  const [checked, setChecked] = useState(false);
  const [surfaces, setSurfaces] = useState<HostSurface[]>([]);
  const [busy, setBusy] = useState(false);

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
      const list = await hostStatus();
      if (!cancelled) setSurfaces(list);
    };
    void poll();
    const timer = window.setInterval(() => void poll(), 4000);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, []);

  const stopAnimation = async () => {
    setBusy(true);
    await hostStop();
    setSurfaces(await hostStatus());
    setBusy(false);
  };

  const togglePaused = async () => {
    const next = !(surfaces[0]?.paused ?? false);
    setBusy(true);
    await hostSetPaused(next);
    setSurfaces(await hostStatus());
    setBusy(false);
  };

  const restoreWallpaper = async () => {
    setBusy(true);
    await hostRestorePrevious();
    setBusy(false);
  };

  const featured = catalogue?.entries.slice(0, 5) ?? [];

  return (
    <div className="page">
      <div>
        <h1 className="page-title">Welcome to GameScape</h1>
        <p className="page-subtitle">
          The foundation build — shell, storage and the first data are coming
          online.
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
            {surfaces.length > 0 ? "Animating" : "Idle"}
          </div>
          <div className="stat-note">
            {surfaces.length > 0
              ? `${surfaces[0].title} · ${surfaces.length} monitor${surfaces.length === 1 ? "" : "s"}${surfaces[0].paused ? " · paused" : ""}`
              : "Nothing animating — set a wallpaper from your library."}
          </div>
          {IN_TAURI && (
            <div className="row">
              {surfaces.length > 0 && (
                <>
                  <button
                    type="button"
                    className="button"
                    onClick={() => void togglePaused()}
                    disabled={busy}
                  >
                    {surfaces[0].paused ? "Resume" : "Pause"}
                  </button>
                  <button
                    type="button"
                    className="button"
                    onClick={() => void stopAnimation()}
                    disabled={busy}
                  >
                    Stop
                  </button>
                </>
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
          <div className="stat-value">0.0.0</div>
          <div className="stat-note">Pre-alpha · Phase 2 foundation</div>
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
