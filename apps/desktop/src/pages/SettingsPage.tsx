import { useCallback, useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { Database, FolderOpen, Moon, Sun } from "lucide-react";
import { useTheme } from "../app/ThemeProvider";
import {
  getDbHealth,
  IN_TAURI,
  libraryInfo,
  saveSettings,
  type DbHealth,
} from "../lib/ipc";

export function SettingsPage() {
  const { theme, setTheme } = useTheme();
  const [health, setHealth] = useState<DbHealth | null>(null);
  const [libraryDir, setLibraryDir] = useState<string | null>(null);

  const loadLibraryDir = useCallback(async () => {
    const result = await libraryInfo();
    setLibraryDir(result.ok ? result.value.libraryDir : null);
  }, []);

  useEffect(() => {
    let cancelled = false;
    void getDbHealth().then((value) => {
      if (!cancelled) setHealth(value);
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
          <span className="chip">v0.0.0 · pre-alpha</span>
          <span className="chip">Apache-2.0</span>
        </div>
        <div className="muted">
          Phase 3 — personal library, collections and integrity tools. Built
          with Tauri, Rust and React.
        </div>
      </div>
    </div>
  );
}
