import { LayoutGrid } from "lucide-react";
import { EmptyState } from "../components/EmptyState";

export function WidgetsPage() {
  return (
    <div className="page">
      <EmptyState
        icon={LayoutGrid}
        title="Widgets are on the way"
        description="Clocks, calendars and the full edit mode arrive in Phases 6–7. The desktop-layer foundation they depend on is already verified (Spike S1)."
      />
    </div>
  );
}
