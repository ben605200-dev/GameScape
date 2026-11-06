import type { CatalogueEntry, MediaType } from "./ipc";

/**
 * Pure catalogue helpers: labels, derived facets, filtering and sorting.
 * Client-side filtering is the documented strategy for catalogues up to
 * ~2,000 entries (the split threshold lives in the website/data plan).
 */

export const MEDIA_LABELS: Record<string, string> = {
  static: "Static",
  gif: "GIF",
  mp4: "MP4",
};

export const EFFECT_STYLES = ["orbit", "ribbon", "grid"] as const;

/** Deterministic fallback so every wallpaper gets a fitting effect. */
export function defaultEffectStyle(id: string): string {
  let hash = 0;
  for (const char of id) {
    hash = (hash * 31 + char.charCodeAt(0)) % 997;
  }
  return EFFECT_STYLES[hash % EFFECT_STYLES.length];
}

export function effectStyleOf(entry: CatalogueEntry): string {
  return entry.effect?.style ?? defaultEffectStyle(entry.id);
}

export type Orientation = "landscape" | "portrait" | "ultrawide";
export type ResolutionBucket = "1080p" | "1440p" | "4k";
export type SortOrder = "newest" | "alpha";

export function orientationOf(entry: CatalogueEntry): Orientation {
  const { width, height } = entry.resolution;
  if (height > width) return "portrait";
  if (width / height >= 2.3) return "ultrawide";
  return "landscape";
}

export function resolutionBucketOf(entry: CatalogueEntry): ResolutionBucket {
  const shortSide = Math.min(entry.resolution.width, entry.resolution.height);
  if (shortSide >= 2000) return "4k";
  if (shortSide >= 1300) return "1440p";
  return "1080p";
}

export interface CatalogueFilters {
  query: string;
  game: string;
  mediaType: MediaType | "all";
  orientation: Orientation | "all";
  resolution: ResolutionBucket | "all";
  sort: SortOrder;
}

export const DEFAULT_CATALOGUE_FILTERS: CatalogueFilters = {
  query: "",
  game: "all",
  mediaType: "all",
  orientation: "all",
  resolution: "all",
  sort: "newest",
};

export function hasActiveFilters(filters: CatalogueFilters): boolean {
  return (
    filters.query.trim() !== "" ||
    filters.game !== "all" ||
    filters.mediaType !== "all" ||
    filters.orientation !== "all" ||
    filters.resolution !== "all"
  );
}

export function filterCatalogue(
  entries: CatalogueEntry[],
  filters: CatalogueFilters,
): CatalogueEntry[] {
  const query = filters.query.trim().toLowerCase();

  const filtered = entries.filter((entry) => {
    if (filters.game !== "all" && (entry.game ?? "") !== filters.game) {
      return false;
    }
    if (filters.mediaType !== "all" && entry.mediaType !== filters.mediaType) {
      return false;
    }
    if (
      filters.orientation !== "all" &&
      orientationOf(entry) !== filters.orientation
    ) {
      return false;
    }
    if (
      filters.resolution !== "all" &&
      resolutionBucketOf(entry) !== filters.resolution
    ) {
      return false;
    }
    if (query) {
      const haystack = [entry.title, entry.game ?? "", ...entry.tags]
        .join(" ")
        .toLowerCase();
      if (!haystack.includes(query)) return false;
    }
    return true;
  });

  const sorted = [...filtered];
  if (filters.sort === "alpha") {
    sorted.sort((a, b) => a.title.localeCompare(b.title));
  } else {
    sorted.sort((a, b) => b.addedAt.localeCompare(a.addedAt));
  }
  return sorted;
}

export function gamesOf(entries: CatalogueEntry[]): string[] {
  const games = new Set<string>();
  for (const entry of entries) {
    if (entry.game) games.add(entry.game);
  }
  return [...games].sort((a, b) => a.localeCompare(b));
}
