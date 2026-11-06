import { LayoutTemplate } from "lucide-react";
import { EmptyState } from "../components/EmptyState";

export function LayoutsPage() {
  return (
    <div className="page">
      <EmptyState
        icon={LayoutTemplate}
        title="Layout Editor"
        description="Save and switch complete desktop setups — wallpaper, widgets, appearance — in one click. Arrives in Phase 7."
      />
    </div>
  );
}
