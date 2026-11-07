import { useCallback, useEffect, useState } from "react";
import { Clock, Gauge, Image, LayoutGrid, Plus, Trash2 } from "lucide-react";
import {
  hostStatus,
  IN_TAURI,
  widgetCreate,
  widgetRemove,
  widgetUpdate,
  widgetsList,
  type WidgetEntry,
} from "../lib/ipc";

const KIND_LABELS: Record<string, string> = {
  clock: "Clock",
  system: "System monitor",
  wallpaper: "Wallpaper status",
};

interface WidgetDraft {
  enabled: boolean;
  monitor: number;
  x: number;
  y: number;
  width: number;
  height: number;
}

function kindIcon(kind: string) {
  if (kind === "system") return Gauge;
  if (kind === "wallpaper") return Image;
  return Clock;
}

export function WidgetsPage() {
  const [widgets, setWidgets] = useState<WidgetEntry[]>([]);
  const [drafts, setDrafts] = useState<Record<number, WidgetDraft>>({});
  const [monitorCount, setMonitorCount] = useState(1);
  const [newKind, setNewKind] = useState("clock");
  const [newMonitor, setNewMonitor] = useState(0);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    const result = await widgetsList();
    if (result.ok) {
      setWidgets(result.value);
      setDrafts(
        Object.fromEntries(
          result.value.map((widget) => [
            widget.id,
            {
              enabled: widget.enabled,
              monitor: widget.monitor,
              x: widget.x,
              y: widget.y,
              width: widget.width,
              height: widget.height,
            },
          ]),
        ),
      );
    } else {
      setError(result.error);
    }
    const report = await hostStatus();
    setMonitorCount(Math.max(1, report.monitors.length));
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const addWidget = async () => {
    setBusy(true);
    setError(null);
    const result = await widgetCreate(newKind, newMonitor);
    if (!result.ok) setError(result.error);
    await load();
    setBusy(false);
  };

  const saveDraft = async (id: number) => {
    const draft = drafts[id];
    if (!draft) return;
    setBusy(true);
    setError(null);
    const result = await widgetUpdate(id, draft);
    if (!result.ok) setError(result.error);
    await load();
    setBusy(false);
  };

  const removeWidget = async (id: number) => {
    if (!window.confirm("Remove this widget from the desktop?")) return;
    setBusy(true);
    setError(null);
    const result = await widgetRemove(id);
    if (!result.ok) setError(result.error);
    await load();
    setBusy(false);
  };

  const setDraft = (id: number, patch: Partial<WidgetDraft>) =>
    setDrafts((previous) => ({
      ...previous,
      [id]: { ...previous[id], ...patch },
    }));

  return (
    <div className="page">
      <div className="card stack">
        <strong>
          <LayoutGrid size={15} aria-hidden="true" /> Add a widget
        </strong>
        <div className="muted">
          Widgets live on your desktop — above the wallpaper, behind the
          icons. They keep working while GameScape is in the tray.
        </div>
        <div className="row">
          <select
            className="button"
            value={newKind}
            onChange={(event) => setNewKind(event.target.value)}
            disabled={!IN_TAURI || busy}
          >
            <option value="clock">Clock</option>
            <option value="system">System monitor</option>
            <option value="wallpaper">Wallpaper status</option>
          </select>
          {monitorCount > 1 && (
            <select
              className="button"
              value={newMonitor}
              onChange={(event) => setNewMonitor(Number(event.target.value))}
              disabled={!IN_TAURI || busy}
            >
              {Array.from({ length: monitorCount }, (_, index) => (
                <option key={index} value={index}>
                  Monitor {index + 1}
                </option>
              ))}
            </select>
          )}
          <button
            type="button"
            className="button button-primary"
            onClick={() => void addWidget()}
            disabled={!IN_TAURI || busy}
          >
            <Plus size={15} aria-hidden="true" /> Add widget
          </button>
        </div>
        {error && <div className="muted">{error}</div>}
      </div>

      {widgets.length === 0 && (
        <div className="card stack">
          <strong>No widgets yet</strong>
          <div className="muted">
            Add a clock or a system monitor above — it appears on your desktop
            instantly. Positions and sizes are pixels from the top-left of the
            chosen monitor.
          </div>
        </div>
      )}

      {widgets.map((widget) => {
        const Icon = kindIcon(widget.kind);
        const draft = drafts[widget.id];
        return (
          <div className="card stack" key={widget.id}>
            <div className="row" style={{ justifyContent: "space-between" }}>
              <strong>
                <Icon size={15} aria-hidden="true" />{" "}
                {KIND_LABELS[widget.kind] ?? widget.kind} #{widget.id}
              </strong>
              <button
                type="button"
                className="button"
                onClick={() => void removeWidget(widget.id)}
                disabled={!IN_TAURI || busy}
              >
                <Trash2 size={14} aria-hidden="true" /> Remove
              </button>
            </div>
            {draft && (
              <div className="row">
                <label className="row">
                  <input
                    type="checkbox"
                    checked={draft.enabled}
                    onChange={(event) =>
                      setDraft(widget.id, { enabled: event.target.checked })
                    }
                    disabled={!IN_TAURI || busy}
                  />
                  <span className="muted">On desktop</span>
                </label>
                <select
                  className="button"
                  value={draft.monitor}
                  onChange={(event) =>
                    setDraft(widget.id, { monitor: Number(event.target.value) })
                  }
                  disabled={!IN_TAURI || busy}
                >
                  {Array.from({ length: monitorCount }, (_, index) => (
                    <option key={index} value={index}>
                      Monitor {index + 1}
                    </option>
                  ))}
                </select>
                {(
                  [
                    ["X", "x"],
                    ["Y", "y"],
                    ["W", "width"],
                    ["H", "height"],
                  ] as const
                ).map(([label, field]) => (
                  <label className="row" key={field}>
                    <span className="muted">{label}</span>
                    <input
                      className="button"
                      style={{ width: "82px" }}
                      type="number"
                      value={draft[field]}
                      onChange={(event) =>
                        setDraft(widget.id, {
                          [field]: Number(event.target.value),
                        } as Partial<WidgetDraft>)
                      }
                      disabled={!IN_TAURI || busy}
                    />
                  </label>
                ))}
                <button
                  type="button"
                  className="button"
                  onClick={() => void saveDraft(widget.id)}
                  disabled={!IN_TAURI || busy}
                >
                  Save
                </button>
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}
