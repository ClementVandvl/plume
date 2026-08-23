import { invoke } from "@tauri-apps/api/core";

/**
 * Reports into the same console as the backend.
 *
 * An error the user can see but that leaves no trace in the console is the worst
 * of both worlds: they know something broke and have nothing to show for it.
 */
type Level = "debug" | "info" | "warn" | "error";

const send = (level: Level, scope: string, message: string, detail?: string) => {
  invoke("log_client", { level, scope, message, detail: detail ?? null }).catch(() => {
    // The console is a convenience; losing a line must never break a flow.
  });
};

export const logInfo = (scope: string, message: string, detail?: string) =>
  send("info", scope, message, detail);

export function logError(scope: string, message: string, cause?: unknown) {
  const detail = cause === undefined ? undefined : String(cause);
  send("error", scope, message, detail);
  console.error(message, cause);
}

/** Catches what no local handler saw. */
export function installGlobalErrorReporting() {
  window.addEventListener("error", (event) => {
    send("error", "interface", event.message, `${event.filename}:${event.lineno}`);
  });
  window.addEventListener("unhandledrejection", (event) => {
    send("error", "interface", "Promesse rejetée sans gestionnaire", String(event.reason));
  });
}
