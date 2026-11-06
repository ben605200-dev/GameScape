import { Download } from "lucide-react";
import { EmptyState } from "../components/EmptyState";

export function DownloadsPage() {
  return (
    <div className="page">
      <EmptyState
        icon={Download}
        title="Downloads"
        description="The download manager — queue, pause/resume, checksum verification — arrives in Phase 3 with the catalogue pipeline."
      />
    </div>
  );
}
