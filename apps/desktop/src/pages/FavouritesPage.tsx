import { Star } from "lucide-react";
import { EmptyState } from "../components/EmptyState";

export function FavouritesPage() {
  return (
    <div className="page">
      <EmptyState
        icon={Star}
        title="Favourites"
        description="Star wallpapers once the catalogue browser lands in Phase 3 — your list stays local, like everything else."
      />
    </div>
  );
}
