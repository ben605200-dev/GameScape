import { useEffect, useMemo, useState } from "react";
import { Star } from "lucide-react";
import { useFavourites } from "../app/FavouritesProvider";
import { EmptyState } from "../components/EmptyState";
import { WallpaperCard } from "../components/WallpaperCard";
import { loadCatalogue, type CatalogueIndex } from "../lib/ipc";

export function FavouritesPage() {
  const [catalogue, setCatalogue] = useState<CatalogueIndex | null>(null);
  const [checked, setChecked] = useState(false);
  const { ids, ready } = useFavourites();

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

  const favourites = useMemo(
    () =>
      catalogue
        ? catalogue.entries.filter((entry) => ids.has(entry.id))
        : [],
    [catalogue, ids],
  );

  if (!checked || !ready) {
    return (
      <div className="page">
        <p className="muted">Loading favourites…</p>
      </div>
    );
  }

  if (favourites.length === 0) {
    return (
      <div className="page">
        <EmptyState
          icon={Star}
          title="Star wallpapers to find them here"
          description="Tap the star on any wallpaper card or detail page — your favourites stay on this PC."
        />
      </div>
    );
  }

  return (
    <div className="page">
      <div className="row">
        <span className="chip chip-accent">
          {favourites.length} favourite{favourites.length === 1 ? "" : "s"}
        </span>
        <span className="muted">Stored locally — nothing leaves this PC.</span>
      </div>

      <div className="card-grid">
        {favourites.map((entry) => (
          <WallpaperCard key={entry.id} entry={entry} />
        ))}
      </div>
    </div>
  );
}
