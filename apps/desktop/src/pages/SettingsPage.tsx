import { useEffect, useState } from "react";
import { Database, Moon, Sun } from "lucide-react";
import { useTheme } from "../app/ThemeProvider";
import { getDbHealth, type DbHealth } from "../lib/ipc";

export function SettingsPage() {
  const { theme, setTheme } = useTheme();
  const [health, setHealth] = useState<DbHealth | null>(null);

  useEffect(() => {
    let cancelled = false;
    void getDbHealth().then((h) => {
      if (!cancelled) setHealth(h);
    });
    return () => {
      cancelled = true;
    };
  }, []);

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
          <Database size={15} aria-hidden="true" /> Storage
        </strong>
        <div className="muted">
          Everything stays on this PC — GameScape has no accounts and sends
          nothing anywhere. Your data lives in the Windows app data folder and
          your chosen library location.
        </div>
        <div className="mono">
          {health
            ? `database ok · schema v${health.schemaVersion} · ${health.mediaCount} library items · ${health.catalogueCount} catalogue records`
            : "Database status available inside the app (not in browser preview)."}
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
          Phase 2 — foundation build. Built with Tauri, Rust and React.
        </div>
      </div>
    </div>
  );
}
