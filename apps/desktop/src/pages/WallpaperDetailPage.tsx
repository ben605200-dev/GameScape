import { useEffect, useState } from "react";
import { ArrowLeft, Sparkles } from "lucide-react";
import { Link, useParams } from "react-router-dom";
import { FavouriteButton } from "../components/FavouriteButton";
import { InteractivePreview } from "../components/InteractivePreview";
import { loadCatalogue, type CatalogueEntry } from "../lib/ipc";

const MEDIA_LABELS: Record<string, string> = {
  static: "Static",
  gif: "GIF",
  mp4: "MP4",
};

export function WallpaperDetailPage() {
  const { id } = useParams();
  const [entry, setEntry] = useState<CatalogueEntry | null>(null);
  const [checked, setChecked] = useState(false);

  useEffect(() => {
    let cancelled = false;
    void loadCatalogue().then((catalogue) => {
      if (cancelled) return;
      setEntry(catalogue?.entries.find((item) => item.id === id) ?? null);
      setChecked(true);
    });
    return () => {
      cancelled = true;
    };
  }, [id]);

  if (!checked) {
    return (
      <div className="page">
        <p className="muted">Loading…</p>
      </div>
    );
  }

  if (!entry) {
    return (
      <div className="page">
        <div className="card stack">
          <strong>Wallpaper not found</strong>
          <span className="muted">
            This entry is not in the catalogue index.
          </span>
          <div>
            <Link className="button" to="/wallpapers">
              <ArrowLeft size={16} aria-hidden="true" /> Back to Wallpapers
            </Link>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="page">
      <div className="row">
        <Link className="button" to="/wallpapers">
          <ArrowLeft size={16} aria-hidden="true" /> Wallpapers
        </Link>
        {entry.interactive && (
          <span className="chip chip-accent">
            <Sparkles size={13} aria-hidden="true" /> Interactive preview
          </span>
        )}
      </div>

      <div className="detail-grid">
        <div className="card stack">
          {entry.interactive ? (
            <>
              <InteractivePreview
                className="preview-canvas"
                effectName={entry.effect?.style ?? "orbit"}
                dominant={entry.placeholder.dominantHex}
                accent={entry.placeholder.accentHex}
              />
              <p className="muted" style={{ fontSize: 13 }}>
                Move your mouse over the preview — the effect follows your
                cursor, tinted with this wallpaper&apos;s own palette. On the
                desktop, the same effect runs behind your icons (technique
                validated in the Phase 0 spike; the desktop host ships it in
                Phase 5).
              </p>
            </>
          ) : (
            <>
              <div
                className="gradient-thumb"
                style={{
                  background: `linear-gradient(135deg, ${entry.placeholder.dominantHex} 0%, ${entry.placeholder.accentHex} 100%)`,
                }}
                aria-hidden="true"
              />
              <p className="muted" style={{ fontSize: 13 }}>
                Playback preview arrives with the media pipeline (Phase 4).
              </p>
            </>
          )}
        </div>

        <div className="card stack">
          <div className="row" style={{ justifyContent: "space-between" }}>
            <h2 style={{ fontSize: 18 }}>{entry.title}</h2>
            <FavouriteButton id={entry.id} />
          </div>
          {entry.description && <p className="muted">{entry.description}</p>}

          <dl className="meta-list">
            <div>
              <dt>Game</dt>
              <dd>{entry.game ?? "GameScape Original"}</dd>
            </div>
            <div>
              <dt>Type</dt>
              <dd>{MEDIA_LABELS[entry.mediaType] ?? entry.mediaType}</dd>
            </div>
            <div>
              <dt>Resolution</dt>
              <dd className="mono">
                {entry.resolution.width}×{entry.resolution.height}
              </dd>
            </div>
            <div>
              <dt>Licence</dt>
              <dd>{entry.licenceId}</dd>
            </div>
            <div>
              <dt>Added</dt>
              <dd>{entry.addedAt}</dd>
            </div>
          </dl>

          {entry.tags.length > 0 && (
            <div className="chip-row">
              {entry.tags.map((tag) => (
                <span key={tag} className="chip">
                  {tag}
                </span>
              ))}
            </div>
          )}

          <div className="muted" style={{ fontSize: 12.5 }}>
            Downloading, applying and full-screen preview arrive with the
            catalogue pipeline (Phase 3) and the desktop host (Phase 5).
          </div>
        </div>
      </div>
    </div>
  );
}
