import { useCallback, useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import {
  FolderOpen,
  ImagePlus,
  ShieldCheck,
  Trash2,
} from "lucide-react";
import { Link } from "react-router-dom";
import { EmptyState } from "../components/EmptyState";
import { generateMissingPosters } from "../lib/posters";
import {
  assetUrl,
  collectionCreate,
  collectionDelete,
  collectionMemberships,
  collectionsList,
  importFiles,
  IN_TAURI,
  libraryCheck,
  libraryOpenDir,
  libraryRelink,
  listLibrary,
  removeLibraryItem,
  type CollectionMembership,
  type CollectionSummary,
  type ImportProgressEvent,
  type ImportSummary,
  type IntegrityIssue,
  type LibraryItem,
} from "../lib/ipc";

// Both images and MP4 video are importable; keep the picker in sync with
// the pipeline (a missing extension here makes valid files unpickable).
const IMPORT_FILTER = {
  name: "Wallpapers (images and MP4 video)",
  extensions: ["jpg", "jpeg", "png", "webp", "gif", "mp4"],
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
  const [issues, setIssues] = useState<IntegrityIssue[] | null>(null);
  const [checking, setChecking] = useState(false);
  const [collections, setCollections] = useState<CollectionSummary[]>([]);
  const [memberships, setMemberships] = useState<CollectionMembership[]>([]);
  const [activeCollection, setActiveCollection] = useState<number | "all">("all");
  const [newCollection, setNewCollection] = useState("");
  const [barError, setBarError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    const [list, cols, mem] = await Promise.all([
      listLibrary(),
      collectionsList(),
      collectionMemberships(),
    ]);
    setItems(list ?? []);
    setCollections(cols.ok ? cols.value : []);
    setMemberships(mem.ok ? mem.value : []);
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  // Videos without thumbnails get a poster frame generated on first load.
  useEffect(() => {
    if (!items) return;
    generateMissingPosters(items);
  }, [items]);

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

  const visibleItems = useMemo(() => {
    if (!items) return [];
    if (activeCollection === "all") return items;
    const inCollection = new Set(
      memberships
        .filter((m) => m.collectionId === activeCollection)
        .map((m) => m.mediaId),
    );
    return items.filter((item) => inCollection.has(item.id));
  }, [items, memberships, activeCollection]);

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
    const selection = await open({ multiple: true, filters: [IMPORT_FILTER] });
    if (!selection) return;
    const paths = Array.isArray(selection) ? selection : [selection];
    await runImport(paths);
  }, [runImport]);

  const runCheck = useCallback(async () => {
    setChecking(true);
    setBarError(null);
    const result = await libraryCheck();
    if (result.ok) {
      setIssues(result.value);
    } else {
      setBarError(result.error);
    }
    setChecking(false);
  }, []);

  const relink = useCallback(
    async (issue: IntegrityIssue) => {
      if (!IN_TAURI) return;
      const selection = await open({ multiple: false, filters: [IMPORT_FILTER] });
      if (typeof selection !== "string") return;
      const result = await libraryRelink(issue.id, selection);
      if (!result.ok) {
        setBarError(result.error);
        return;
      }
      await refresh();
      await runCheck();
    },
    [refresh, runCheck],
  );

  const removeIssue = useCallback(
    async (issue: IntegrityIssue) => {
      if (!window.confirm(`Remove "${issue.title}" from your library?`)) return;
      await removeLibraryItem(issue.id, false);
      await refresh();
      setIssues((prev) => (prev ? prev.filter((i) => i.id !== issue.id) : prev));
    },
    [refresh],
  );

  const createCollection = useCallback(async () => {
    const name = newCollection.trim();
    if (!name) return;
    setBarError(null);
    const result = await collectionCreate(name);
    if (!result.ok) {
      setBarError(result.error);
      return;
    }
    setNewCollection("");
    await refresh();
  }, [newCollection, refresh]);

  const deleteActiveCollection = useCallback(async () => {
    if (activeCollection === "all") return;
    const name = collections.find((c) => c.id === activeCollection)?.name ?? "";
    if (
      !window.confirm(
        `Delete collection "${name}"? The wallpapers stay in your library.`,
      )
    ) {
      return;
    }
    await collectionDelete(activeCollection);
    setActiveCollection("all");
    await refresh();
  }, [activeCollection, collections, refresh]);

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

  const openFolder = useCallback(async () => {
    const result = await libraryOpenDir();
    if (!result.ok) setBarError(result.error);
  }, []);

  return (
    <div className="page">
      <div className="row" style={{ justifyContent: "space-between" }}>
        <div>
          <h1 className="page-title">Personal Library</h1>
          <p className="page-subtitle">
            Your own wallpapers — imports stay private on this PC.
          </p>
        </div>
        <div className="row">
          <button
            type="button"
            className="button"
            onClick={() => void runCheck()}
            disabled={checking || !IN_TAURI}
          >
            <ShieldCheck size={15} aria-hidden="true" />
            {checking ? "Checking…" : "Check integrity"}
          </button>
          <button
            type="button"
            className="button"
            onClick={() => void openFolder()}
            disabled={!IN_TAURI}
          >
            <FolderOpen size={15} aria-hidden="true" /> Open folder
          </button>
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
      </div>

      {IN_TAURI ? (
        <span className="muted" style={{ fontSize: 13 }}>
          Tip: you can also drag image files anywhere into this window.
        </span>
      ) : (
        <span className="chip">Browser preview — the library works inside the app.</span>
      )}

      {barError && <span className="chip chip-danger">{barError}</span>}

      {issues !== null && (
        <div className="card stack">
          {issues.length === 0 ? (
            <div className="row">
              <span className="chip chip-success">All files verified — nothing is missing.</span>
              <button type="button" className="button" onClick={() => setIssues(null)}>
                Dismiss
              </button>
            </div>
          ) : (
            <>
              <strong>
                {issues.length} file{issues.length === 1 ? "" : "s"} need attention
              </strong>
              {issues.map((issue) => (
                <div key={issue.id} className="import-row">
                  <span>{issue.title}</span>
                  <span
                    className={`chip chip-${issue.status === "missing" ? "danger" : "warning"}`}
                  >
                    {issue.status === "missing" ? "Missing" : "Changed"}
                  </span>
                  {issue.status === "missing" && (
                    <button
                      type="button"
                      className="button"
                      onClick={() => void relink(issue)}
                    >
                      Locate file…
                    </button>
                  )}
                  <button
                    type="button"
                    className="button"
                    onClick={() => void removeIssue(issue)}
                  >
                    Remove entry
                  </button>
                </div>
              ))}
              <div>
                <button type="button" className="button" onClick={() => setIssues(null)}>
                  Dismiss
                </button>
              </div>
            </>
          )}
        </div>
      )}

      {items !== null && items.length > 0 && (
        <div className="row">
          <button
            type="button"
            className={`chip-button${activeCollection === "all" ? " is-active" : ""}`}
            onClick={() => setActiveCollection("all")}
          >
            All ({items.length})
          </button>
          {collections.map((collection) => (
            <button
              key={collection.id}
              type="button"
              className={`chip-button${activeCollection === collection.id ? " is-active" : ""}`}
              onClick={() => setActiveCollection(collection.id)}
            >
              {collection.name} ({collection.count})
            </button>
          ))}
          {activeCollection !== "all" && (
            <button
              type="button"
              className="icon-button"
              title="Delete this collection"
              aria-label="Delete this collection"
              onClick={() => void deleteActiveCollection()}
            >
              <Trash2 size={14} />
            </button>
          )}
          <span className="row" style={{ marginLeft: "auto" }}>
            <input
              className="input"
              style={{ maxWidth: 180, padding: "6px 10px" }}
              placeholder="New collection…"
              value={newCollection}
              onChange={(event) => setNewCollection(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter") void createCollection();
              }}
            />
            <button type="button" className="button" onClick={() => void createCollection()}>
              Add
            </button>
          </span>
        </div>
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
          description="Import your own wallpapers — JPG, PNG, WEBP, GIF and MP4 are validated, de-duplicated and thumbnailed locally. Nothing is ever uploaded."
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
      ) : visibleItems.length === 0 ? (
        <EmptyState
          icon={FolderOpen}
          title="Nothing in this collection yet"
          description="Open a library item and tick this collection to add it."
        />
      ) : (
        <div className="card-grid">
          {visibleItems.map((item) => (
            <Link
              key={item.id}
              to={`/library/${item.id}`}
              className="card media-card media-card-link"
            >
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
                  onClick={(event) => {
                    event.preventDefault();
                    event.stopPropagation();
                    void remove(item);
                  }}
                >
                  <Trash2 size={15} />
                </button>
              </div>
            </Link>
          ))}
        </div>
      )}
    </div>
  );
}
