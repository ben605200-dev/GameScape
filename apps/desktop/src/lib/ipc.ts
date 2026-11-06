/**
 * Typed bridge to the Rust core.
 *
 * Every call degrades gracefully when the UI runs outside the Tauri shell
 * (plain `vite` in a browser): settings fall back to localStorage and
 * desktop-only data returns null, so UI work never blocks on the backend.
 */

import { convertFileSrc, invoke } from "@tauri-apps/api/core";

export const IN_TAURI =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/* ------------------------------------------------------------------ */
/* Settings                                                            */
/* ------------------------------------------------------------------ */

export interface AppSettings {
  theme?: string | null;
  /** Custom library location; null/undefined uses the default under Pictures. */
  libraryDir?: string | null;
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

/**
 * Saves a settings patch — merges onto the current values so unrelated
 * fields are never clobbered (e.g. changing the theme must not wipe the
 * custom library directory).
 */
export async function saveSettings(patch: AppSettings): Promise<void> {
  if (!IN_TAURI) {
    fallbackSaveSettings({ ...fallbackGetSettings(), ...patch });
    return;
  }
  try {
    const current = await invoke<AppSettings>("settings_get");
    await invoke("settings_set", { settings: { ...current, ...patch } });
  } catch {
    fallbackSaveSettings({ ...fallbackGetSettings(), ...patch });
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

export interface EffectSpec {
  /** orbit | ribbon | grid (the set grows over time). */
  style: string;
}

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
  /** Cursor-reactive effect descriptor for interactive entries. */
  effect?: EffectSpec | null;
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

/* ------------------------------------------------------------------ */
/* Library                                                             */
/* ------------------------------------------------------------------ */

export interface LibraryItem {
  id: number;
  title: string;
  description?: string | null;
  /** JSON array string of tags (parse before use). */
  tags?: string | null;
  format?: string | null;
  width?: number | null;
  height?: number | null;
  bytes?: number | null;
  sha256?: string | null;
  thumbPath?: string | null;
  filePath?: string | null;
  addedAt: string;
}

export interface ImportItemResult {
  fileName: string;
  /** imported | duplicate | unsupported | invalid | corrupted | failed */
  status: string;
  detail?: string | null;
  id?: number | null;
}

export interface ImportSummary {
  imported: number;
  duplicates: number;
  failed: number;
  items: ImportItemResult[];
}

export interface ImportProgressEvent {
  index: number;
  total: number;
  fileName: string;
  status: string;
  detail?: string | null;
}

export async function importFiles(paths: string[]): Promise<ImportSummary | null> {
  if (!IN_TAURI) return null;
  try {
    return await invoke<ImportSummary>("import_paths", { paths });
  } catch {
    return null;
  }
}

export async function listLibrary(): Promise<LibraryItem[] | null> {
  if (!IN_TAURI) return null;
  try {
    return await invoke<LibraryItem[]>("library_list");
  } catch {
    return null;
  }
}

export async function removeLibraryItem(
  id: number,
  deleteFiles: boolean,
): Promise<boolean> {
  if (!IN_TAURI) return false;
  try {
    await invoke("library_remove", { id, deleteFiles });
    return true;
  } catch {
    return false;
  }
}

/** Turn a local file path into a webview-loadable asset URL. */
export function assetUrl(path: string | null | undefined): string {
  if (!path || !IN_TAURI) return "";
  return convertFileSrc(path);
}

export interface CollectionSummary {
  id: number;
  name: string;
  count: number;
}

export interface CollectionMembership {
  collectionId: number;
  mediaId: number;
}

export interface IntegrityIssue {
  id: number;
  title: string;
  /** missing | changed */
  status: string;
}

export interface LibraryInfo {
  libraryDir: string;
}

export type ActionResult<T> =
  | { ok: true; value: T }
  | { ok: false; error: string };

async function runAction<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<ActionResult<T>> {
  if (!IN_TAURI) {
    return {
      ok: false,
      error: "This works inside the desktop app (browser preview is limited).",
    };
  }
  try {
    const value = await invoke<T>(command, args);
    return { ok: true, value };
  } catch (error) {
    return { ok: false, error: typeof error === "string" ? error : String(error) };
  }
}

export const libraryUpdate = (id: number, title: string, description: string, tags: string[]) =>
  runAction<void>("library_update", { id, title, description, tags });

export const libraryCheck = () => runAction<IntegrityIssue[]>("library_check");

export const libraryRelink = (id: number, newPath: string) =>
  runAction<void>("library_relink", { id, newPath });

export const libraryReveal = (id: number) => runAction<void>("library_reveal", { id });

export const libraryOpenDir = () => runAction<void>("library_open_dir");

export const libraryInfo = () => runAction<LibraryInfo>("library_info");

export const collectionsList = () => runAction<CollectionSummary[]>("collections_list");

export const collectionCreate = (name: string) =>
  runAction<number>("collection_create", { name });

export const collectionDelete = (id: number) => runAction<void>("collection_delete", { id });

export const collectionSetMembership = (
  collectionId: number,
  mediaId: number,
  member: boolean,
) => runAction<void>("collection_set_membership", { collectionId, mediaId, member });

export const collectionMemberships = () =>
  runAction<CollectionMembership[]>("collection_memberships");

/* ------------------------------------------------------------------ */
/* Favourites                                                          */
/* ------------------------------------------------------------------ */

export async function listFavourites(): Promise<string[] | null> {
  if (!IN_TAURI) return null;
  try {
    return await invoke<string[]>("favourites_list");
  } catch {
    return null;
  }
}

export async function setFavourite(
  catalogueId: string,
  favourite: boolean,
): Promise<boolean> {
  if (!IN_TAURI) return false;
  try {
    await invoke("favourite_set", { catalogueId, favourite });
    return true;
  } catch {
    return false;
  }
}
