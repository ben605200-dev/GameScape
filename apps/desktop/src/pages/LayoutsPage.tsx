import { useCallback, useEffect, useState } from "react";
import {
  Check,
  Copy,
  LayoutTemplate,
  Pencil,
  RefreshCw,
  Rocket,
  Trash2,
  X,
} from "lucide-react";
import { isThemeName } from "@gamescape/ui";
import { useTheme } from "../app/ThemeProvider";
import { EmptyState } from "../components/EmptyState";
import {
  assetUrl,
  IN_TAURI,
  layoutApply,
  layoutDuplicate,
  layoutRemove,
  layoutRename,
  layoutSave,
  layoutsList,
  type LayoutEntry,
} from "../lib/ipc";

function summarise(entry: LayoutEntry): string {
  const parts: string[] = [
    `${entry.widgetCount} widget${entry.widgetCount === 1 ? "" : "s"}`,
  ];
  if (entry.wallpaperTitle) {
    parts.push(`wallpaper "${entry.wallpaperTitle}"`);
  } else {
    parts.push("no wallpaper");
  }
  if (entry.fit) parts.push(`${entry.fit} fit`);
  return parts.join(" · ");
}

/** SQLite stores UTC "YYYY-MM-DD HH:MM:SS"; show it as saved. */
function updatedLabel(value: string): string {
  return value.length >= 16 ? value.slice(0, 16) : value;
}

export function LayoutsPage() {
  const [entries, setEntries] = useState<LayoutEntry[]>([]);
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [renamingId, setRenamingId] = useState<number | null>(null);
  const [renameText, setRenameText] = useState("");
  const { setTheme } = useTheme();

  const load = useCallback(async () => {
    const result = await layoutsList();
    if (result.ok) setEntries(result.value);
    else setError(result.error);
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const saveNew = async () => {
    setBusy(true);
    setError(null);
    setStatus(null);
    const result = await layoutSave(null, name);
    if (result.ok) {
      setStatus(`Saved the current desktop as "${name.trim()}".`);
      setName("");
    } else {
      setError(result.error);
    }
    await load();
    setBusy(false);
  };

  const update = async (entry: LayoutEntry) => {
    setBusy(true);
    setError(null);
    setStatus(null);
    const result = await layoutSave(entry.id, entry.name);
    if (result.ok) {
      setStatus(`"${entry.name}" now matches the current desktop.`);
    } else {
      setError(result.error);
    }
    await load();
    setBusy(false);
  };

  const apply = async (entry: LayoutEntry) => {
    setBusy(true);
    setError(null);
    setStatus(null);
    const result = await layoutApply(entry.id);
    if (result.ok) {
      setStatus(
        `Applied "${result.value}" — wallpaper, widgets and appearance restored.`,
      );
      if (entry.theme && isThemeName(entry.theme)) {
        setTheme(entry.theme);
      }
    } else {
      setError(result.error);
    }
    await load();
    setBusy(false);
  };

  const rename = async (entry: LayoutEntry) => {
    setBusy(true);
    setError(null);
    setStatus(null);
    const result = await layoutRename(entry.id, renameText);
    if (result.ok) {
      setStatus(`Renamed to "${renameText.trim()}".`);
      setRenamingId(null);
    } else {
      setError(result.error);
    }
    await load();
    setBusy(false);
  };

  const duplicate = async (entry: LayoutEntry) => {
    setBusy(true);
    setError(null);
    setStatus(null);
    const result = await layoutDuplicate(entry.id);
    if (result.ok) {
      setStatus(`Copied "${entry.name}".`);
    } else {
      setError(result.error);
    }
    await load();
    setBusy(false);
  };

  const remove = async (entry: LayoutEntry) => {
    if (
      !window.confirm(
        `Remove the layout "${entry.name}"? Your current desktop is not changed.`,
      )
    ) {
      return;
    }
    setBusy(true);
    setError(null);
    setStatus(null);
    const result = await layoutRemove(entry.id);
    if (!result.ok) setError(result.error);
    await load();
    setBusy(false);
  };

  return (
    <div className="page">
      <div>
        <h1 className="page-title">Layout Editor</h1>
        <p className="page-subtitle">
          Save and switch complete desktop setups — wallpaper, widgets and
          appearance — in one click.
        </p>
      </div>

      <div className="card stack">
        <strong>
          <LayoutTemplate size={15} aria-hidden="true" /> Save the current
          desktop
        </strong>
        <div className="muted">
          A layout captures the applied wallpaper (and its fit), every widget
          with its position and settings, and your appearance theme.
        </div>
        <div className="row">
          <input
            className="button"
            style={{ width: "260px" }}
            type="text"
            placeholder="Layout name (e.g. Night gaming)"
            value={name}
            maxLength={60}
            onChange={(event) => setName(event.target.value)}
            disabled={!IN_TAURI || busy}
            onKeyDown={(event) => {
              if (event.key === "Enter" && name.trim()) void saveNew();
            }}
          />
          <button
            type="button"
            className="button button-primary"
            onClick={() => void saveNew()}
            disabled={!IN_TAURI || busy || name.trim() === ""}
          >
            <Check size={15} aria-hidden="true" /> Save as layout
          </button>
        </div>
        {status && <div className="muted">{status}</div>}
        {error && <div className="muted">{error}</div>}
      </div>

      {entries.length === 0 && (
        <EmptyState
          icon={LayoutTemplate}
          title="No layouts yet"
          description="Save your current desktop above — wallpaper, widgets and appearance — then switch between complete setups in one click."
        />
      )}

      {entries.map((entry) => (
        <div className="card stack" key={entry.id}>
          <div className="row" style={{ alignItems: "flex-start" }}>
            {entry.wallpaperThumb ? (
              <img
                className="layout-thumb"
                src={assetUrl(entry.wallpaperThumb)}
                alt=""
                aria-hidden="true"
              />
            ) : (
              <div
                className="layout-thumb layout-thumb-placeholder"
                aria-hidden="true"
              >
                <LayoutTemplate size={22} />
              </div>
            )}
            <div className="stack" style={{ alignItems: "flex-start" }}>
              <strong>{entry.name}</strong>
              <span className="muted">{summarise(entry)}</span>
              <span className="muted">
                Updated {updatedLabel(entry.updatedAt)}
              </span>
              {entry.wallpaperAvailable === false && (
                <span className="muted">
                  The saved wallpaper is no longer in your library — re-link
                  it, then update this layout.
                </span>
              )}
            </div>
          </div>

          <div className="row">
            <button
              type="button"
              className="button button-primary"
              onClick={() => void apply(entry)}
              disabled={
                !IN_TAURI || busy || entry.wallpaperAvailable === false
              }
            >
              <Rocket size={14} aria-hidden="true" /> Apply layout
            </button>
            <button
              type="button"
              className="button"
              onClick={() => void update(entry)}
              disabled={!IN_TAURI || busy}
              title="Refresh this layout from the current desktop"
            >
              <RefreshCw size={14} aria-hidden="true" /> Update
            </button>
            <button
              type="button"
              className="button"
              onClick={() => void duplicate(entry)}
              disabled={!IN_TAURI || busy}
              title="Copy this layout under a new name"
            >
              <Copy size={14} aria-hidden="true" /> Duplicate
            </button>
            <button
              type="button"
              className="button"
              onClick={() => {
                setRenamingId(entry.id);
                setRenameText(entry.name);
              }}
              disabled={!IN_TAURI || busy}
            >
              <Pencil size={14} aria-hidden="true" /> Rename
            </button>
            <button
              type="button"
              className="button"
              onClick={() => void remove(entry)}
              disabled={!IN_TAURI || busy}
            >
              <Trash2 size={14} aria-hidden="true" /> Remove
            </button>
          </div>

          {renamingId === entry.id && (
            <div className="row">
              <input
                className="button"
                style={{ width: "260px" }}
                type="text"
                value={renameText}
                maxLength={60}
                onChange={(event) => setRenameText(event.target.value)}
                disabled={busy}
                onKeyDown={(event) => {
                  if (event.key === "Enter" && renameText.trim())
                    void rename(entry);
                }}
              />
              <button
                type="button"
                className="button button-primary"
                onClick={() => void rename(entry)}
                disabled={busy || renameText.trim() === ""}
              >
                <Check size={14} aria-hidden="true" /> Save name
              </button>
              <button
                type="button"
                className="button"
                onClick={() => setRenamingId(null)}
                disabled={busy}
              >
                <X size={14} aria-hidden="true" /> Cancel
              </button>
            </div>
          )}
        </div>
      ))}
    </div>
  );
}
