/**
 * Where are we running?
 *
 * The window chrome is ours on every OS, but its shape differs: macOS keeps
 * its native traffic lights (overlay title bar), Windows and Linux get our
 * drawn buttons on the right. In `npm run dev` there is no Tauri at all, so
 * everything here degrades to "browser": no drag region, no window buttons.
 */

export type Platform = "macos" | "windows" | "linux" | "browser";

/**
 * True inside the Tauri webview, false under plain Vite.
 *
 * A function, not a constant: as a constant it was read the moment this module
 * was imported, which happens before the development mock installs its fake
 * backend. The window chrome then always believed it was in a browser, and the
 * platform-specific half of it — Windows buttons, the tools menu — could never
 * be looked at outside a real build.
 */
export function isTauri(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

let cached: Platform | null = null;

export async function detectPlatform(): Promise<Platform> {
  if (cached) return cached;
  if (!isTauri()) {
    cached = "browser";
    return cached;
  }
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    const os = await invoke<string>("os_platform");
    cached = os === "macos" ? "macos" : os === "windows" ? "windows" : "linux";
  } catch {
    cached = "linux";
  }
  return cached;
}
