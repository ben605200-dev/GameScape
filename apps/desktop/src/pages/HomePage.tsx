import { useEffect, useState } from "react";
import { Database, Image, Layers, Monitor, Sparkles } from "lucide-react";
import {
  getDbHealth,
  IN_TAURI,
  loadCatalogue,
  type CatalogueIndex,
  type DbHealth,
} from "../lib/ipc";

const MEDIA_LABELS: Record<string, string> = {
  static: "Static",
  gif: "GIF",
  mp4: "MP4",
};

export function HomePage() {
  const [health, setHealth] = useState<DbHealth | null>(null);
  const [catalogue, setCatalogue] = useState<CatalogueIndex | null>(null);
  const [checked, setChecked] = useState(false);

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
          <div className="stat-value">Verified</div>
          <div className="stat-note">
            Spike S1 passed — host integration arrives in Phase 5
          </div>
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
              <div key={entry.id} className="card media-card">
                <div
                  className="gradient-thumb"
                  style={{
                    background: `linear-gradient(135deg, ${entry.placeholder.dominantHex} 0%, ${entry.placeholder.accentHex} 100%)`,
                  }}
                  aria-hidden="true"
                />
                <div className="media-title">{entry.title}</div>
                <div className="media-meta">
                  <span>{entry.game ?? "GameScape Original"}</span>
                  <span className="chip">{MEDIA_LABELS[entry.mediaType] ?? entry.mediaType}</span>
                </div>
              </div>
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
