/**
 * Theme plumbing.
 *
 * The tokens do the styling; this file only stamps `data-theme` on the root
 * element. "system" removes the attribute, letting the `prefers-color-scheme`
 * media query in tokens.css decide — so the app follows the OS live, without
 * JavaScript watching anything.
 */

export type Theme = "light" | "dark" | "system";

export function applyTheme(theme: Theme) {
  const root = document.documentElement;
  if (theme === "system") delete root.dataset.theme;
  else root.dataset.theme = theme;
}

export function asTheme(raw: string | undefined): Theme {
  return raw === "light" || raw === "dark" ? raw : "system";
}
