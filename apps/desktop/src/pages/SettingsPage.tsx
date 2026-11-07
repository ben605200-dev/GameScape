import { useCallback, useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { Database, FolderOpen, Monitor, Moon, RefreshCw, Sun } from "lucide-react";
import { useTheme } from "../app/ThemeProvider";
import {
  getDbHealth,
  getSettings,
  IN_TAURI,
  libraryInfo,
  saveSettings,
  type DbHealth,
} from "../lib/ipc";

export function SettingsPage() {
  const { theme, setTheme } = useTheme();
  const [health, setHealth] = useState<DbHealth | null>(null);
  const [libraryDir, setLibraryDir] = useState<string | null>(null);
  const [fit, setFit] = useState("fill");
  const [rotationEnabled, setRotationEnabled] = useState(false);
  const [rotationMinutes, setRotationMinutes] = useState(15);

  const loadLibraryDir = useCallback(async () => {
    const result = await libraryInfo();
    setLibraryDir(result.ok ? result.value.libraryDir : null);
  }, []);

  useEffect(() => {
    let cancelled = false;
    void getDbHealth().then((value) => {
      if (!cancelled) setHealth(value);
    });
    void getSettings().then((settings) => {
      if (cancelled) return;
      setFit(settings.wallpaperFit ?? "fill");
      setRotationEnabled(settings.rotationEnabled ?? false);
      setRotationMinutes(settings.rotationMinutes ?? 15);
    });
    void loadLibraryDir();
    return () => {
      cancelled = true;
    };
  }, [loadLibraryDir]);

  const chooseLibraryDir = async () => {
    if (!IN_TAURI) return;
    const selection = await open({ directory: true, multiple: false });
    if (typeof selection !== "string") return;
    await saveSettings({ libraryDir: selection });
    await loadLibraryDir();
  };

  const resetLibraryDir = async () => {
    await saveSettings({ libraryDir: null });
    await loadLibraryDir();
  };

  const changeFit = async (value: string) => {
    setFit(value);
    await saveSettings({ wallpaperFit: value });
  };

  const toggleRotation = async (enabled: boolean) => {
    setRotationEnabled(enabled);
    await saveSettings({ rotationEnabled: enabled });
  };

  const changeRotationMinutes = async (minutes: number) => {
    setRotationMinutes(minutes);
    await saveSettings({ rotationMinutes: minutes });
  };

  return (
    <div className="page">
      <div className="card stack">
        <strong>Appearance</strong>
        <div className="muted">
          Theme preference is stored locally and applies immediately.
        </div>
        <div className="row">
          <button
            type="button"
            className={`button${theme === "dark" ? " button-primary" : ""}`}
            onClick={() => setTheme("dark")}
          >
            <Moon size={16} aria-hidden="true" /> Dark
          </button>
          <button
            type="button"
            className={`button${theme === "light" ? " button-primary" : ""}`}
            onClick={() => setTheme("light")}
          >
            <Sun size={16} aria-hidden="true" /> Light
          </button>
        </div>
      </div>

      <div className="card stack">
        <strong>
          <Monitor size={15} aria-hidden="true" /> Desktop wallpaper fit
        </strong>
        <div className="muted">
          Wallpapers always fill the screen by default, whatever their
          original size. Pick a different fit if you prefer.
        </div>
        <select
          className="button"
          value={fit}
          onChange={(event) => void changeFit(event.target.value)}
          disabled={!IN_TAURI}
        >
          <option value="fill">Fill the screen (recommended)</option>
          <option value="fit">Fit — whole image, black bars</option>
          <option value="stretch">Stretch</option>
          <option value="center">Center — original size</option>
          <option value="span">Span across monitors</option>
        </select>
      </div>

      <div className="card stack">
        <strong>
          <RefreshCw size={15} aria-hidden="true" /> Wallpaper rotation
        </strong>
        <div className="muted">
          Change your wallpaper on a schedule, cycling through the wallpapers
          in your library. Rotation pauses while wallpapers are turned off.
        </div>
        <div className="row">
          <button
            type="button"
            className={`button${rotationEnabled ? " button-primary" : ""}`}
            onClick={() => void toggleRotation(true)}
            disabled={!IN_TAURI}
          >
            On
          </button>
          <button
            type="button"
            className={`button${!rotationEnabled ? " button-primary" : ""}`}
            onClick={() => void toggleRotation(false)}
            disabled={!IN_TAURI}
          >
            Off
          </button>
          <select
            className="button"
            value={rotationMinutes}
            onChange={(event) =>
              void changeRotationMinutes(Number(event.target.value))
            }
            disabled={!IN_TAURI || !rotationEnabled}
          >
            <option value={5}>Every 5 minutes</option>
            <option value={15}>Every 15 minutes</option>
            <option value={30}>Every 30 minutes</option>
            <option value={60}>Every 60 minutes</option>
          </select>
        </div>
      </div>

      <div className="card stack">
        <strong>
          <FolderOpen size={15} aria-hidden="true" /> Library location
        </strong>
        <div className="muted">
          Wallpapers you import are copied here. Changing this affects new
          imports — existing files stay where they are.
        </div>
        <div className="path-text">
          {libraryDir ?? "Available inside the desktop app."}
        </div>
        <div className="row">
          <button
            type="button"
            className="button"
            onClick={() => void chooseLibraryDir()}
            disabled={!IN_TAURI}
          >
            Change…
          </button>
          <button
            type="button"
            className="button"
            onClick={() => void resetLibraryDir()}
            disabled={!IN_TAURI}
          >
            Use default
          </button>
        </div>
      </div>

      <div className="card stack">
        <strong>
          <Database size={15} aria-hidden="true" /> Storage
        </strong>
        <div className="muted">
          Everything stays on this PC — GameScape has no accounts and sends
          nothing anywhere. Your data lives in the Windows app data folder.
        </div>
        <div className="mono">
          {health
            ? `database ok · schema v${health.schemaVersion} · ${health.mediaCount} library items · ${health.catalogueCount} catalogue records`
            : "Database status is available inside the app (not in browser preview)."}
        </div>
      </div>

      <div className="card stack">
        <strong>About</strong>
        <div className="row">
          <span className="chip">GameScape</span>
          <span className="chip">v0.5.0 · pre-alpha</span>
          <span className="chip">Apache-2.0</span>
        </div>
        <div className="muted">
          Phase 5 — desktop wallpapers with an auto-pause governor, plus an
          installer. Built with Tauri, Rust and React.
        </div>
      </div>
    </div>
  );
}
