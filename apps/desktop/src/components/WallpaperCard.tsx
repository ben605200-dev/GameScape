import { Sparkles } from "lucide-react";
import { Link } from "react-router-dom";
import { effectStyleOf, MEDIA_LABELS } from "../lib/catalogue";
import type { CatalogueEntry } from "../lib/ipc";
import { FavouriteButton } from "./FavouriteButton";

/** Shared catalogue card: star toggle + gradient placeholder + metadata. */
export function WallpaperCard({ entry }: { entry: CatalogueEntry }) {
  return (
    <Link
      to={`/wallpapers/${entry.id}`}
      className="card media-card media-card-link"
    >
      <div className="card-thumb-wrap">
        <div
          className="gradient-thumb"
          style={{
            background: `linear-gradient(135deg, ${entry.placeholder.dominantHex} 0%, ${entry.placeholder.accentHex} 100%)`,
          }}
          aria-hidden="true"
        />
        <FavouriteButton id={entry.id} className="card-star" />
      </div>

      <div className="media-title">{entry.title}</div>

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
        {entry.interactive !== false ? (
          <span className="chip chip-accent">
            <Sparkles size={12} aria-hidden="true" /> {effectStyleOf(entry)}
          </span>
        ) : (
          <span className="muted">{entry.licenceId}</span>
        )}
      </div>
    </Link>
  );
}
