/**
 * Typed companions for the design tokens.
 * The CSS file is the source of truth for values; this module exports the
 * names and small helpers that TypeScript code needs.
 */

export const THEMES = ["dark", "light"] as const;

export type ThemeName = (typeof THEMES)[number];

export const DEFAULT_THEME: ThemeName = "dark";

export function isThemeName(value: unknown): value is ThemeName {
  return value === "dark" || value === "light";
}
