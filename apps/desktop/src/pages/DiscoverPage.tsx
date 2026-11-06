import { Compass } from "lucide-react";
import { EmptyState } from "../components/EmptyState";

export function DiscoverPage() {
  return (
    <div className="page">
      <EmptyState
        icon={Compass}
        title="Discover"
        description="Featured and trending shelves arrive with the public catalogue and website (Phase 8), powered by real download signals."
      />
    </div>
  );
}
