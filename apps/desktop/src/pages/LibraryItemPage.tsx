import { useCallback, useEffect, useState } from "react";
import { ArrowLeft, FolderOpen, Monitor, Save, Trash2 } from "lucide-react";
import { Link, useNavigate, useParams } from "react-router-dom";
import {
  assetUrl,
  collectionCreate,
  collectionMemberships,
  collectionsList,
  collectionSetMembership,
  hostApplyMedia,
  hostStatus,
  hostTurnOff,
  IN_TAURI,
  libraryReveal,
  libraryUpdate,
  listLibrary,
  removeLibraryItem,
  type CollectionSummary,
  type LibraryItem,
} from "../lib/ipc";

function formatBytes(bytes: number | null | undefined): string {
  if (!bytes || bytes <= 0) return "—";
  if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${Math.round(bytes / 1024)} KB`;
}

function formatDuration(seconds: number | null | undefined): string {
  if (!seconds || seconds <= 0) return "—";
  const total = Math.round(seconds);
  return `${Math.floor(total / 60)}:${String(total % 60).padStart(2, "0")}`;
}

export function LibraryItemPage() {
  const { id } = useParams();
  const navigate = useNavigate();
  const itemId = Number(id);
  const reduceMotion =
    typeof window !== "undefined" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  const [item, setItem] = useState<LibraryItem | null>(null);
  const [checked, setChecked] = useState(false);
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [tags, setTags] = useState("");
  const [savedFlash, setSavedFlash] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [deleteFile, setDeleteFile] = useState(false);
  const [collections, setCollections] = useState<CollectionSummary[]>([]);
  const [memberships, setMemberships] = useState<Set<string>>(new Set());
  const [newCollection, setNewCollection] = useState("");
  const [applying, setApplying] = useState(false);
  const [applyResult, setApplyResult] = useState<string | null>(null);
  const [appliedId, setAppliedId] = useState<number | null>(null);
  const isApplied = appliedId === itemId;

  const load = useCallback(async () => {
    const list = await listLibrary();
    const found = (list ?? []).find((entry) => entry.id === itemId) ?? null;
    setItem(found);
    if (found) {
      setTitle(found.title);
      setDescription(found.description ?? "");
      try {
        setTags((JSON.parse(found.tags ?? "[]") as string[]).join(", "));
      } catch {
        setTags("");
      }
    }
    const [cols, mem] = await Promise.all([
      collectionsList(),
      collectionMemberships(),
    ]);
    setCollections(cols.ok ? cols.value : []);
    setMemberships(
      mem.ok
        ? new Set(mem.value.map((m) => `${m.collectionId}:${m.mediaId}`))
        : new Set(),
    );
    const report = await hostStatus();
    setAppliedId(report.applied?.mediaId ?? null);
    setChecked(true);
  }, [itemId]);

  useEffect(() => {
    void load();
  }, [load]);

  const save = async () => {
    setError(null);
    const tagList = tags
      .split(",")
      .map((tag) => tag.trim())
      .filter(Boolean);
    const result = await libraryUpdate(itemId, title, description, tagList);
    if (!result.ok) {
      setError(result.error);
      return;
    }
    setSavedFlash(true);
    window.setTimeout(() => setSavedFlash(false), 1800);
  };

  const toggleCollection = async (collectionId: number) => {
    const key = `${collectionId}:${itemId}`;
    const member = !memberships.has(key);
    const result = await collectionSetMembership(collectionId, itemId, member);
    if (!result.ok) {
      setError(result.error);
      return;
    }
    setMemberships((previous) => {
      const next = new Set(previous);
      if (member) next.add(key);
      else next.delete(key);
      return next;
    });
    setCollections((previous) =>
      previous.map((collection) =>
        collection.id === collectionId
          ? { ...collection, count: Math.max(0, collection.count + (member ? 1 : -1)) }
          : collection,
      ),
    );
  };

  const createCollection = async () => {
    const name = newCollection.trim();
    if (!name) return;
    setError(null);
    const result = await collectionCreate(name);
    if (!result.ok) {
      setError(result.error);
      return;
    }
    setNewCollection("");
    await collectionSetMembership(result.value, itemId, true);
    await load();
  };

  const reveal = async () => {
    const result = await libraryReveal(itemId);
    if (!result.ok) setError(result.error);
  };

  const applyToDesktop = async () => {
    setApplying(true);
    setError(null);
    const result = await hostApplyMedia(itemId);
    if (result.ok) {
      setAppliedId(itemId);
      setApplyResult(
        result.value === "static"
          ? "Applied as desktop wallpaper"
          : `Animating on your desktop (${result.value})`,
      );
      window.setTimeout(() => setApplyResult(null), 4000);
    } else {
      setError(result.error);
    }
    setApplying(false);
  };

  const turnOffDesktop = async () => {
    setApplying(true);
    setError(null);
    const result = await hostTurnOff();
    if (result.ok) {
      setAppliedId(null);
      setApplyResult("Desktop wallpaper turned off");
      window.setTimeout(() => setApplyResult(null), 4000);
    } else {
      setError(result.error);
    }
    setApplying(false);
  };

  const remove = async () => {
    if (!item) return;
    const message = deleteFile
      ? `Remove "${item.title}" and permanently delete its managed file?`
      : `Remove "${item.title}" from your library? The file stays on disk.`;
    if (!window.confirm(message)) return;
    await removeLibraryItem(item.id, deleteFile);
    navigate("/library");
  };

  if (!checked) {
    return (
      <div className="page">
        <p className="muted">Loading…</p>
      </div>
    );
  }

  if (!item) {
    return (
      <div className="page">
        <div className="card stack">
          <strong>Library item not found</strong>
          <span className="muted">It may have been removed.</span>
          <div>
            <Link className="button" to="/library">
              <ArrowLeft size={16} aria-hidden="true" /> Back to Library
            </Link>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="page">
      <div className="row">
        <Link className="button" to="/library">
          <ArrowLeft size={16} aria-hidden="true" /> Library
        </Link>
        <span className="chip">{item.format?.toUpperCase() ?? "file"}</span>
        {savedFlash && <span className="chip chip-success">Saved</span>}
        {applyResult && <span className="chip chip-success">{applyResult}</span>}
        {error && <span className="chip chip-danger">{error}</span>}
      </div>

      <div className="detail-grid">
        <div className="card stack">
          <div className="item-thumb">
            {item.format === "mp4" ? (
              <video
                className="item-media"
                src={assetUrl(item.filePath)}
                controls
                loop
                muted
                playsInline
                autoPlay={!reduceMotion}
                poster={item.thumbPath ? assetUrl(item.thumbPath) : undefined}
              />
            ) : item.format === "gif" ? (
              <img className="item-media" src={assetUrl(item.filePath)} alt="" />
            ) : item.thumbPath ? (
              <img src={assetUrl(item.thumbPath)} alt="" />
            ) : null}
          </div>
          <dl className="meta-list">
            <div>
              <dt>Dimensions</dt>
              <dd className="mono">
                {item.width && item.height ? `${item.width}×${item.height}` : "—"}
              </dd>
            </div>
            <div>
              <dt>Size</dt>
              <dd className="mono">{formatBytes(item.bytes)}</dd>
            </div>
            <div>
              <dt>Added</dt>
              <dd>{item.addedAt.slice(0, 10)}</dd>
            </div>
            {item.format === "mp4" && (
              <div>
                <dt>Duration</dt>
                <dd className="mono">
                  {formatDuration(item.durationS)}
                  {item.hasAudio ? "" : " · silent"}
                </dd>
              </div>
            )}
            {item.format === "gif" && item.frameCount ? (
              <div>
                <dt>Animation</dt>
                <dd className="mono">
                  {item.frameCount} frames · {formatDuration(item.durationS)} loop
                </dd>
              </div>
            ) : null}
            <div>
              <dt>SHA-256</dt>
              <dd className="mono" title={item.sha256 ?? undefined}>
                {(item.sha256 ?? "").slice(0, 12)}…
              </dd>
            </div>
          </dl>
          <span className="path-text">{item.filePath}</span>
          <div className="row">
            {isApplied ? (
              <button
                type="button"
                className="button"
                onClick={() => void turnOffDesktop()}
                disabled={applying || !IN_TAURI}
              >
                <Monitor size={15} aria-hidden="true" />
                {applying ? "Turning off…" : "Turn off desktop wallpaper"}
              </button>
            ) : (
              <button
                type="button"
                className="button button-primary"
                onClick={() => void applyToDesktop()}
                disabled={applying || !IN_TAURI}
              >
                <Monitor size={15} aria-hidden="true" />
                {applying ? "Applying…" : "Set as desktop wallpaper"}
              </button>
            )}
            <button type="button" className="button" onClick={() => void reveal()}>
              <FolderOpen size={15} aria-hidden="true" /> Show in Explorer
            </button>
          </div>
          <div className="row">
            <label className="collection-row" style={{ cursor: "pointer" }}>
              <input
                type="checkbox"
                checked={deleteFile}
                onChange={(event) => setDeleteFile(event.target.checked)}
              />
              <span className="muted">also delete the managed file (permanent)</span>
            </label>
            <button type="button" className="button" onClick={() => void remove()}>
              <Trash2 size={15} aria-hidden="true" /> Remove
            </button>
          </div>
        </div>

        <div className="card stack">
          <label className="field">
            <span>Title</span>
            <input
              className="input"
              value={title}
              maxLength={120}
              onChange={(event) => setTitle(event.target.value)}
            />
          </label>
          <label className="field">
            <span>Description</span>
            <textarea
              className="textarea"
              rows={3}
              maxLength={500}
              value={description}
              onChange={(event) => setDescription(event.target.value)}
            />
          </label>
          <label className="field">
            <span>Tags (comma separated)</span>
            <input
              className="input"
              value={tags}
              placeholder="cyberpunk, city, night"
              onChange={(event) => setTags(event.target.value)}
            />
          </label>
          <div>
            <button type="button" className="button button-primary" onClick={() => void save()}>
              <Save size={15} aria-hidden="true" /> Save changes
            </button>
          </div>
        </div>
      </div>

      <div className="card stack">
        <strong>Collections</strong>
        {collections.length === 0 && (
          <span className="muted">No collections yet — create one below.</span>
        )}
        {collections.map((collection) => (
          <label
            key={collection.id}
            className="collection-row"
            style={{ cursor: "pointer" }}
          >
            <input
              type="checkbox"
              checked={memberships.has(`${collection.id}:${itemId}`)}
              onChange={() => void toggleCollection(collection.id)}
            />
            <span>{collection.name}</span>
            <span className="muted" style={{ marginLeft: "auto" }}>
              {collection.count} item{collection.count === 1 ? "" : "s"}
            </span>
          </label>
        ))}
        <div className="row">
          <input
            className="input"
            style={{ maxWidth: 240 }}
            placeholder="New collection…"
            value={newCollection}
            onChange={(event) => setNewCollection(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") void createCollection();
            }}
          />
          <button type="button" className="button" onClick={() => void createCollection()}>
            Create &amp; add
          </button>
        </div>
      </div>
    </div>
  );
}
