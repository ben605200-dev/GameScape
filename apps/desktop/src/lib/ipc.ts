/**
 * Typed bridge to the Rust core.
 *
 * Every call degrades gracefully when the UI runs outside the Tauri shell
 * (plain `vite` in a browser): settings fall back to localStorage and
 * desktop-only data returns null, so UI work never blocks on the backend.
 */

import { invoke } from "@tauri-apps/api/core";

export const IN_TAURI =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/* ------------------------------------------------------------------ */
/* Settings                                                            */
/* ------------------------------------------------------------------ */

export interface AppSettings {
  theme?: string | null;
}

const LS_SETTINGS_KEY = "gamescape.settings.v0";

function fallbackGetSettings(): AppSettings {
  try {
    return JSON.parse(localStorage.getItem(LS_SETTINGS_KEY) ?? "{}") as AppSettings;
  } catch {
    return {};
  }
}

function fallbackSaveSettings(settings: AppSettings): void {
  try {
    localStorage.setItem(LS_SETTINGS_KEY, JSON.stringify(settings));
  } catch {
    /* storage unavailable (private mode) — preferences simply won't persist */
  }
}

export async function getSettings(): Promise<AppSettings> {
  if (!IN_TAURI) return fallbackGetSettings();
  try {
    return await invoke<AppSettings>("settings_get");
  } catch {
    return fallbackGetSettings();
  }
}

export async function saveSettings(settings: AppSettings): Promise<void> {
  if (!IN_TAURI) {
    fallbackSaveSettings(settings);
    return;
  }
  try {
    await invoke("settings_set", { settings });
  } catch {
    fallbackSaveSettings(settings);
  }
}

/* ------------------------------------------------------------------ */
/* Storage / diagnostics                                               */
/* ------------------------------------------------------------------ */

export interface DbHealth {
  ok: boolean;
  schemaVersion: number;
  mediaCount: number;
  catalogueCount: number;
}

export async function getDbHealth(): Promise<DbHealth | null> {
  if (!IN_TAURI) return null;
  try {
    return await invoke<DbHealth>("db_health");
  } catch {
    return null;
  }
}

/* ------------------------------------------------------------------ */
/* Catalogue                                                           */
/* ------------------------------------------------------------------ */

export interface Resolution {
  width: number;
  height: number;
}

export interface PlaceholderColors {
  dominantHex: string;
  accentHex: string;
}

export type MediaType = "static" | "gif" | "mp4";

export interface CatalogueEntry {
  id: string;
  title: string;
  description?: string | null;
  mediaType: MediaType;
  game?: string | null;
  tags: string[];
  resolution: Resolution;
  licenceId: string;
  addedAt: string;
  placeholder: PlaceholderColors;
  /** Cursor-reactive wallpaper flag (desktop runtime ships in Phase 5). */
  interactive?: boolean;
}

export interface CatalogueIndex {
  schemaVersion: string;
  generatedAt: string;
  entries: CatalogueEntry[];
}

export async function loadCatalogue(): Promise<CatalogueIndex | null> {
  if (!IN_TAURI) return null;
  try {
    return await invoke<CatalogueIndex>("catalogue_load");
  } catch {
    return null;
  }
}
