import { useCallback, useEffect, useState } from "react";
import {
  Calendar,
  Clock,
  Gauge,
  Hourglass,
  Image,
  LayoutGrid,
  Plus,
  Rocket,
  Trash2,
  Type,
  X,
} from "lucide-react";
import {
  hostStatus,
  IN_TAURI,
  widgetCreate,
  widgetRemove,
  widgetUpdate,
  widgetsList,
  type HostMonitor,
  type WidgetEntry,
} from "../lib/ipc";

const KIND_LABELS: Record<string, string> = {
  clock: "Clock",
  system: "System monitor",
  wallpaper: "Wallpaper status",
  calendar: "Calendar",
  countdown: "Countdown",
  text: "Text",
  launcher: "Launcher",
};

interface LauncherItem {
  label: string;
  target: string;
}

const TEXT_FONTS = [
  "Segoe UI",
  "Segoe UI Variable",
  "Georgia",
  "Trebuchet MS",
  "Verdana",
  "Tahoma",
  "Consolas",
  "Cascadia Code",
  "Courier New",
  "Impact",
  "Arial Black",
  "Segoe Script",
  "Ink Free",
  "Comic Sans MS",
];

interface WidgetDraft {
  enabled: boolean;
  monitor: number;
  x: number;
  y: number;
  width: number;
  height: number;
  label?: string;
  target?: string;
  text?: string;
  font?: string;
  size?: number;
  items?: LauncherItem[];
}

function kindIcon(kind: string) {
  if (kind === "system") return Gauge;
  if (kind === "wallpaper") return Image;
  if (kind === "calendar") return Calendar;
  if (kind === "countdown") return Hourglass;
  if (kind === "text") return Type;
  if (kind === "launcher") return Rocket;
  return Clock;
}

export function WidgetsPage() {
  const [widgets, setWidgets] = useState<WidgetEntry[]>([]);
  const [drafts, setDrafts] = useState<Record<number, WidgetDraft>>({});
  const [monitors, setMonitors] = useState<HostMonitor[]>([
    { index: 0, width: 1920, height: 1080 },
  ]);
  const monitorCount = Math.max(1, monitors.length);
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
          result.value.map((widget) => {
            let label: string | undefined;
            let target: string | undefined;
            let text: string | undefined;
            let font: string | undefined;
            let size: number | undefined;
            let items: LauncherItem[] | undefined;
            if (widget.settings) {
              try {
                const parsed = JSON.parse(widget.settings) as {
                  label?: string;
                  target?: string;
                  text?: string;
                  font?: string;
                  size?: number;
                  items?: LauncherItem[];
                };
                label = parsed.label;
                target = parsed.target;
                text = parsed.text;
                font = parsed.font;
                size = parsed.size;
                if (widget.kind === "launcher" && Array.isArray(parsed.items)) {
                  items = parsed.items.filter(
                    (item) =>
                      item &&
                      typeof item.label === "string" &&
                      typeof item.target === "string",
                  );
                }
              } catch {
                /* settings unreadable; keep the defaults */
              }
            }
            return [
              widget.id,
              {
                enabled: widget.enabled,
                monitor: widget.monitor,
                x: widget.x,
                y: widget.y,
                width: widget.width,
                height: widget.height,
                label,
                target,
                text,
                font,
                size,
                items,
              },
            ];
          }),
        ),
      );
    } else {
      setError(result.error);
    }
    const report = await hostStatus();
    if (report.monitors.length > 0) setMonitors(report.monitors);
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
    const kind = widgets.find((widget) => widget.id === id)?.kind;
    let settings: string | null = null;
    if (kind === "countdown") {
      settings = JSON.stringify({
        label: draft.label ?? "",
        target: draft.target ?? "",
      });
    } else if (kind === "text") {
      settings = JSON.stringify({
        text: draft.text ?? "",
        font: draft.font ?? "Segoe UI",
        size: draft.size ?? 28,
      });
    } else if (kind === "clock") {
      settings = JSON.stringify({ font: draft.font ?? "Segoe UI" });
    } else if (kind === "launcher") {
      settings = JSON.stringify({
        items: (draft.items ?? [])
          .map((item) => ({
            label: item.label.trim() || "Shortcut",
            target: item.target.trim(),
          }))
          .filter((item) => item.target !== "")
          .slice(0, 6),
      });
    }
    const result = await widgetUpdate(id, {
      enabled: draft.enabled,
      monitor: draft.monitor,
      x: draft.x,
      y: draft.y,
      width: draft.width,
      height: draft.height,
      settings,
    });
    if (!result.ok) setError(result.error);
    await load();
    setBusy(false);
  };

  const applyCorner = (id: number, corner: "tl" | "tr" | "bl" | "br") => {
    const draft = drafts[id];
    if (!draft) return;
    const monitor = monitors[draft.monitor] ?? monitors[0];
    const margin = 40;
    const right = corner === "tr" || corner === "br";
    const bottom = corner === "bl" || corner === "br";
    setDraft(id, {
      x: right ? Math.max(margin, monitor.width - draft.width - margin) : margin,
      y: bottom
        ? Math.max(margin, monitor.height - draft.height - margin)
        : margin,
    });
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
            <option value="calendar">Calendar</option>
            <option value="countdown">Countdown</option>
            <option value="text">Text</option>
            <option value="launcher">Launcher</option>
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
                <span className="muted">Corner</span>
                {(["tl", "tr", "bl", "br"] as const).map((corner) => (
                  <button
                    key={corner}
                    type="button"
                    className="button"
                    onClick={() => applyCorner(widget.id, corner)}
                    disabled={!IN_TAURI || busy}
                    title={`Snap to ${corner}`}
                  >
                    {corner === "tl"
                      ? "↖"
                      : corner === "tr"
                        ? "↗"
                        : corner === "bl"
                          ? "↙"
                          : "↘"}
                  </button>
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
            {draft && widget.kind === "countdown" && (
              <div className="row">
                <label className="row">
                  <span className="muted">Label</span>
                  <input
                    className="button"
                    style={{ width: "160px" }}
                    type="text"
                    value={draft.label ?? ""}
                    onChange={(event) =>
                      setDraft(widget.id, { label: event.target.value })
                    }
                    disabled={!IN_TAURI || busy}
                  />
                </label>
                <label className="row">
                  <span className="muted">Target</span>
                  <input
                    className="button"
                    type="datetime-local"
                    value={draft.target ?? ""}
                    onChange={(event) =>
                      setDraft(widget.id, { target: event.target.value })
                    }
                    disabled={!IN_TAURI || busy}
                  />
                </label>
              </div>
            )}
            {draft && widget.kind === "clock" && (
              <div className="row">
                <label className="row">
                  <span className="muted">Font</span>
                  <select
                    className="button"
                    value={draft.font ?? "Segoe UI"}
                    onChange={(event) =>
                      setDraft(widget.id, { font: event.target.value })
                    }
                    disabled={!IN_TAURI || busy}
                  >
                    {TEXT_FONTS.map((font) => (
                      <option key={font} value={font}>
                        {font}
                      </option>
                    ))}
                  </select>
                </label>
                <span className="muted" style={{ fontSize: 12.5 }}>
                  The clock floats without a background; its size follows the
                  widget.
                </span>
              </div>
            )}
            {draft && widget.kind === "text" && (
              <div className="row">
                <textarea
                  className="button"
                  style={{ width: "280px", minHeight: "64px" }}
                  value={draft.text ?? ""}
                  onChange={(event) =>
                    setDraft(widget.id, { text: event.target.value })
                  }
                  placeholder="Text to show on the desktop"
                  disabled={!IN_TAURI || busy}
                />
                <select
                  className="button"
                  value={draft.font ?? "Segoe UI"}
                  onChange={(event) =>
                    setDraft(widget.id, { font: event.target.value })
                  }
                  disabled={!IN_TAURI || busy}
                >
                  {TEXT_FONTS.map((font) => (
                    <option key={font} value={font}>
                      {font}
                    </option>
                  ))}
                </select>
                <label className="row">
                  <span className="muted">Size</span>
                  <input
                    className="button"
                    style={{ width: "72px" }}
                    type="number"
                    min={10}
                    max={200}
                    value={draft.size ?? 28}
                    onChange={(event) =>
                      setDraft(widget.id, { size: Number(event.target.value) })
                    }
                    disabled={!IN_TAURI || busy}
                  />
                </label>
              </div>
            )}
            {draft && widget.kind === "launcher" && (
              <div className="stack">
                {(draft.items ?? []).map((item, index) => (
                  <div className="row" key={index}>
                    <input
                      className="button"
                      style={{ width: "150px" }}
                      type="text"
                      placeholder="Label"
                      value={item.label}
                      onChange={(event) => {
                        const items = [...(draft.items ?? [])];
                        items[index] = {
                          ...items[index],
                          label: event.target.value,
                        };
                        setDraft(widget.id, { items });
                      }}
                      disabled={!IN_TAURI || busy}
                    />
                    <input
                      className="button"
                      style={{ flex: 1 }}
                      type="text"
                      placeholder="Link or path (https://… or C:\\Games\\…)"
                      value={item.target}
                      onChange={(event) => {
                        const items = [...(draft.items ?? [])];
                        items[index] = {
                          ...items[index],
                          target: event.target.value,
                        };
                        setDraft(widget.id, { items });
                      }}
                      disabled={!IN_TAURI || busy}
                    />
                    <button
                      type="button"
                      className="button"
                      onClick={() =>
                        setDraft(widget.id, {
                          items: (draft.items ?? []).filter(
                            (_, itemIndex) => itemIndex !== index,
                          ),
                        })
                      }
                      disabled={!IN_TAURI || busy}
                    >
                      <X size={14} aria-hidden="true" />
                    </button>
                  </div>
                ))}
                <div className="row">
                  <button
                    type="button"
                    className="button"
                    onClick={() =>
                      setDraft(widget.id, {
                        items: [
                          ...(draft.items ?? []),
                          { label: "", target: "" },
                        ].slice(0, 6),
                      })
                    }
                    disabled={
                      !IN_TAURI || busy || (draft.items ?? []).length >= 6
                    }
                  >
                    <Plus size={14} aria-hidden="true" /> Add shortcut
                  </button>
                  <span className="muted" style={{ fontSize: 12.5 }}>
                    Up to 6 shortcuts — click them right on the desktop.
                  </span>
                </div>
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}
