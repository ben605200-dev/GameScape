import { useEffect, useMemo, useState } from "react";
import { Image, Search } from "lucide-react";
import { EmptyState } from "../components/EmptyState";
import { WallpaperCard } from "../components/WallpaperCard";
import {
  DEFAULT_CATALOGUE_FILTERS,
  filterCatalogue,
  gamesOf,
  hasActiveFilters,
  type CatalogueFilters,
} from "../lib/catalogue";
import { loadCatalogue, type CatalogueIndex } from "../lib/ipc";

export function WallpapersPage() {
  const [catalogue, setCatalogue] = useState<CatalogueIndex | null>(null);
  const [checked, setChecked] = useState(false);
  const [filters, setFilters] = useState<CatalogueFilters>(
    DEFAULT_CATALOGUE_FILTERS,
  );

  useEffect(() => {
    let cancelled = false;
    void loadCatalogue().then((result) => {
      if (cancelled) return;
      setCatalogue(result);
      setChecked(true);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  const games = useMemo(
    () => (catalogue ? gamesOf(catalogue.entries) : []),
    [catalogue],
  );
  const visible = useMemo(
    () => (catalogue ? filterCatalogue(catalogue.entries, filters) : []),
    [catalogue, filters],
  );
  const active = hasActiveFilters(filters);

  const update = (patch: Partial<CatalogueFilters>) =>
    setFilters((previous) => ({ ...previous, ...patch }));

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
          Search and filters are live; downloads arrive with the hosting phase.
        </span>
      </div>

      <div className="toolbar card">
        <label className="search-field">
          <Search size={15} aria-hidden="true" />
          <input
            type="search"
            value={filters.query}
            placeholder="Search wallpapers, games, tags…"
            aria-label="Search catalogue"
            onChange={(event) => update({ query: event.target.value })}
          />
        </label>

        <select
          aria-label="Filter by game"
          value={filters.game}
          onChange={(event) => update({ game: event.target.value })}
        >
          <option value="all">All games</option>
          {games.map((game) => (
            <option key={game} value={game}>
              {game}
            </option>
          ))}
        </select>

        <select
          aria-label="Filter by type"
          value={filters.mediaType}
          onChange={(event) =>
            update({ mediaType: event.target.value as CatalogueFilters["mediaType"] })
          }
        >
          <option value="all">All types</option>
          <option value="static">Static</option>
          <option value="gif">GIF</option>
          <option value="mp4">MP4</option>
        </select>

        <select
          aria-label="Filter by orientation"
          value={filters.orientation}
          onChange={(event) =>
            update({
              orientation: event.target.value as CatalogueFilters["orientation"],
            })
          }
        >
          <option value="all">All shapes</option>
          <option value="landscape">Landscape</option>
          <option value="portrait">Portrait</option>
          <option value="ultrawide">Ultrawide</option>
        </select>

        <select
          aria-label="Filter by resolution"
          value={filters.resolution}
          onChange={(event) =>
            update({
              resolution: event.target.value as CatalogueFilters["resolution"],
            })
          }
        >
          <option value="all">All resolutions</option>
          <option value="1080p">1080p</option>
          <option value="1440p">1440p</option>
          <option value="4k">4K</option>
        </select>

        <select
          aria-label="Sort"
          value={filters.sort}
          onChange={(event) =>
            update({ sort: event.target.value as CatalogueFilters["sort"] })
          }
        >
          <option value="newest">Newest first</option>
          <option value="alpha">A → Z</option>
        </select>

        <span className="muted toolbar-count">
          {visible.length} of {catalogue.entries.length}
        </span>

        {active && (
          <button
            type="button"
            className="button"
            onClick={() => setFilters(DEFAULT_CATALOGUE_FILTERS)}
          >
            Clear
          </button>
        )}
      </div>

      {visible.length === 0 ? (
        <EmptyState
          icon={Image}
          title="No wallpapers match"
          description="Try removing a filter or searching for something else."
        >
          <button
            type="button"
            className="button"
            onClick={() => setFilters(DEFAULT_CATALOGUE_FILTERS)}
          >
            Clear filters
          </button>
        </EmptyState>
      ) : (
        <div className="card-grid">
          {visible.map((entry) => (
            <WallpaperCard key={entry.id} entry={entry} />
          ))}
        </div>
      )}
    </div>
  );
}
