import { useEffect, useState } from "react";
import { Image } from "lucide-react";
import { Link } from "react-router-dom";
import { EmptyState } from "../components/EmptyState";
import { loadCatalogue, type CatalogueIndex } from "../lib/ipc";

const MEDIA_LABELS: Record<string, string> = {
  static: "Static",
  gif: "GIF",
  mp4: "MP4",
};

export function WallpapersPage() {
  const [catalogue, setCatalogue] = useState<CatalogueIndex | null>(null);
  const [checked, setChecked] = useState(false);

  useEffect(() => {
    let cancelled = false;
    void loadCatalogue().then((c) => {
      if (cancelled) return;
      setCatalogue(c);
      setChecked(true);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  if (!checked) {
    return (
      <div className="page">
        <p className="muted">Loading catalogue…</p>
      </div>
    );
  }

  if (!catalogue || catalogue.entries.length === 0) {
    return (
      <div className="page">
        <EmptyState
          icon={Image}
          title="The catalogue will live here"
          description="Browse, search and download wallpapers once the catalogue pipeline and website land (Phases 3 and 8)."
        />
      </div>
    );
  }

  return (
    <div className="page">
      <div className="row">
        <span className="chip chip-accent">
          Seed catalogue · v{catalogue.schemaVersion}
        </span>
        <span className="muted">
          Preview data — search and downloads arrive in Phase 3. Click a card for
          details.
        </span>
      </div>

      <div className="card-grid">
        {catalogue.entries.map((entry) => (
          <Link
            key={entry.id}
            to={`/wallpapers/${entry.id}`}
            className="card media-card media-card-link"
          >
            <div
              className="gradient-thumb"
              style={{
                background: `linear-gradient(135deg, ${entry.placeholder.dominantHex} 0%, ${entry.placeholder.accentHex} 100%)`,
              }}
              aria-hidden="true"
            />
            <div className="media-title">{entry.title}</div>
            {entry.description && (
              <div className="muted" style={{ fontSize: 13 }}>
                {entry.description}
              </div>
            )}
            <div className="media-meta">
              <span>{entry.game ?? "GameScape Original"}</span>
              <span className="chip">
                {MEDIA_LABELS[entry.mediaType] ?? entry.mediaType}
              </span>
            </div>
            <div className="media-meta">
              <span className="mono">
                {entry.resolution.width}×{entry.resolution.height}
              </span>
              {entry.interactive ? (
                <span className="chip chip-accent">Interactive</span>
              ) : (
                <span className="muted">{entry.licenceId}</span>
              )}
            </div>
          </Link>
        ))}
      </div>
    </div>
  );
}
