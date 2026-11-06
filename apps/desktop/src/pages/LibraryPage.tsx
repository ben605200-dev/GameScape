import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen, ImagePlus, Trash2 } from "lucide-react";
import { EmptyState } from "../components/EmptyState";
import {
  assetUrl,
  importFiles,
  IN_TAURI,
  listLibrary,
  removeLibraryItem,
  type ImportProgressEvent,
  type ImportSummary,
  type LibraryItem,
} from "../lib/ipc";

const IMAGE_FILTER = {
  name: "Images",
  extensions: ["jpg", "jpeg", "png", "webp", "gif"],
};

const STATUS_LABELS: Record<string, string> = {
  imported: "Imported",
  duplicate: "Duplicate",
  unsupported: "Unsupported",
  invalid: "Invalid",
  corrupted: "Corrupted",
  failed: "Failed",
};

function statusTone(status: string): string {
  if (status === "imported") return "success";
  if (status === "duplicate") return "warning";
  return "danger";
}

export function LibraryPage() {
  const [items, setItems] = useState<LibraryItem[] | null>(null);
  const [progress, setProgress] = useState<ImportProgressEvent[]>([]);
  const [summary, setSummary] = useState<ImportSummary | null>(null);
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    const list = await listLibrary();
    setItems(list ?? []);
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    const onRefresh = () => {
      void refresh();
    };
    window.addEventListener("gamescape:library-refresh", onRefresh);
    return () => {
      window.removeEventListener("gamescape:library-refresh", onRefresh);
    };
  }, [refresh]);

  useEffect(() => {
    if (!IN_TAURI) return;
    let unlisten: (() => void) | undefined;
    void listen<ImportProgressEvent>("imports:progress", (event) => {
      setProgress((prev) => [...prev, event.payload]);
    }).then((fn) => {
      unlisten = fn;
    });
    return () => unlisten?.();
  }, []);

  const runImport = useCallback(
    async (paths: string[]) => {
      if (paths.length === 0) return;
      setBusy(true);
      setProgress([]);
      setSummary(null);
      try {
        const result = await importFiles(paths);
        if (result) setSummary(result);
        await refresh();
      } finally {
        setBusy(false);
      }
    },
    [refresh],
  );

  const pickFiles = useCallback(async () => {
    if (!IN_TAURI) return;
    const selection = await open({ multiple: true, filters: [IMAGE_FILTER] });
    if (!selection) return;
    const paths = Array.isArray(selection) ? selection : [selection];
    await runImport(paths);
  }, [runImport]);

  const remove = useCallback(
    async (item: LibraryItem) => {
      const confirmed = window.confirm(
        `Remove "${item.title}" from your library?\n\nThe managed copy stays on disk — nothing is deleted.`,
      );
      if (!confirmed) return;
      await removeLibraryItem(item.id, false);
      await refresh();
    },
    [refresh],
  );

  return (
    <div className="page">
      <div className="row" style={{ justifyContent: "space-between" }}>
        <div>
          <h1 className="page-title">Personal Library</h1>
          <p className="page-subtitle">
            Your own wallpapers — imports stay private on this PC.
          </p>
        </div>
        <button
          type="button"
          className="button button-primary"
          onClick={() => void pickFiles()}
          disabled={busy || !IN_TAURI}
        >
          <ImagePlus size={16} aria-hidden="true" />
          {busy ? "Importing…" : "Import wallpapers"}
        </button>
      </div>

      {IN_TAURI ? (
        <span className="muted" style={{ fontSize: 13 }}>
          Tip: you can also drag image files anywhere into this window.
        </span>
      ) : (
        <span className="chip">Browser preview — importing works inside the app.</span>
      )}

      {progress.length > 0 && (
        <div className="card stack">
          <strong>Import results</strong>
          {summary && (
            <div className="row">
              <span className="chip chip-success">{summary.imported} imported</span>
              {summary.duplicates > 0 && (
                <span className="chip chip-warning">
                  {summary.duplicates} duplicate
                </span>
              )}
              {summary.failed > 0 && (
                <span className="chip chip-danger">{summary.failed} skipped</span>
              )}
              <button
                type="button"
                className="button"
                onClick={() => {
                  setProgress([]);
                  setSummary(null);
                }}
              >
                Clear
              </button>
            </div>
          )}
          <div className="import-list">
            {progress.map((entry, index) => (
              <div key={`${entry.fileName}-${index}`} className="import-row">
                <span className="mono">{entry.fileName}</span>
                <span className={`chip chip-${statusTone(entry.status)}`}>
                  {STATUS_LABELS[entry.status] ?? entry.status}
                </span>
                {entry.detail && <span className="muted">{entry.detail}</span>}
              </div>
            ))}
          </div>
        </div>
      )}

      {items === null ? (
        <p className="muted">Loading library…</p>
      ) : items.length === 0 ? (
        <EmptyState
          icon={FolderOpen}
          title="Your personal library"
          description="Import your own wallpapers — JPG, PNG, WEBP and GIF are validated, de-duplicated and thumbnailed locally. Nothing is ever uploaded."
        >
          <button
            type="button"
            className="button button-primary"
            onClick={() => void pickFiles()}
            disabled={!IN_TAURI}
          >
            <ImagePlus size={16} aria-hidden="true" /> Import wallpapers
          </button>
        </EmptyState>
      ) : (
        <div className="card-grid">
          {items.map((item) => (
            <div key={item.id} className="card media-card">
              <div className="library-thumb">
                {item.thumbPath ? (
                  <img src={assetUrl(item.thumbPath)} alt="" loading="lazy" />
                ) : null}
              </div>
              <div className="media-title">{item.title}</div>
              <div className="media-meta">
                <span className="mono">
                  {item.width && item.height ? `${item.width}×${item.height}` : ""}{" "}
                  {item.format?.toUpperCase() ?? ""}
                </span>
                <span className="chip">Imported</span>
              </div>
              <div className="media-meta">
                <span className="muted">Added {item.addedAt.slice(0, 10)}</span>
                <button
                  type="button"
                  className="icon-button"
                  title="Remove from library"
                  aria-label={`Remove ${item.title} from library`}
                  onClick={() => void remove(item)}
                >
                  <Trash2 size={15} />
                </button>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
