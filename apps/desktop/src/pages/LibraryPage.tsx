import { Library } from "lucide-react";
import { EmptyState } from "../components/EmptyState";

export function LibraryPage() {
  return (
    <div className="page">
      <EmptyState
        icon={Library}
        title="Your personal library"
        description="Imported and downloaded wallpapers will be managed here — with metadata editing, collections and integrity checks. The import pipeline arrives in Phase 3."
      />
    </div>
  );
}
