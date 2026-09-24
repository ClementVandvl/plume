import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { checkClaudeUpdate, updateClaude } from "../api";
import { t } from "../i18n";
import { logError } from "../log";
import type { ClaudeUpdate } from "../types";

/**
 * Keeping Claude Code current, from the home screen and the settings alike.
 *
 * Plume installs Claude Code once and never opens it the way a person would,
 * so nothing else will ever tell the teacher it has fallen behind — and a CLI
 * that has fallen behind quietly runs an older model than the one chosen.
 *
 * Checking may happen on its own; updating never does, as for Plume itself.
 */
export type ClaudeUpdateState =
  | { kind: "idle" }
  | { kind: "checking" }
  /** Claude Code is not installed: the install row covers it. */
  | { kind: "absent" }
  | { kind: "current"; updatedTo: string | null }
  | { kind: "available"; update: ClaudeUpdate }
  | { kind: "updating"; update: ClaudeUpdate; progress: string }
  | { kind: "failed"; message: string; update: ClaudeUpdate | null };

export function useClaudeUpdate(onUpdated: () => void) {
  const [state, setState] = useState<ClaudeUpdateState>({ kind: "idle" });
  /** "Later" on the home banner holds until the next launch, not forever. */
  const [dismissed, setDismissed] = useState(false);

  useEffect(() => {
    const stop = listen<string>("claude-update", (event) =>
      setState((current) =>
        current.kind === "updating" ? { ...current, progress: event.payload } : current,
      ),
    );
    return () => {
      stop.then((off) => off()).catch(() => {});
    };
  }, []);

  const check = useCallback(async (quiet = false, updatedTo: string | null = null) => {
    setState({ kind: "checking" });
    try {
      const update = await checkClaudeUpdate();
      if (!update) setState({ kind: "absent" });
      else if (update.available) setState({ kind: "available", update });
      else setState({ kind: "current", updatedTo });
    } catch (cause) {
      logError("claude", "Vérification des mises à jour de Claude Code impossible", cause);
      // A silent check that fails should not shout: the teacher did not ask.
      setState(quiet ? { kind: "idle" } : { kind: "failed", message: String(cause), update: null });
    }
  }, []);

  async function install(update: ClaudeUpdate) {
    setState({ kind: "updating", update, progress: t("claudeUpdate.updating") });
    try {
      const version = await updateClaude();
      onUpdated();
      // Asked again rather than assumed: Homebrew may trail the release by a
      // day, and the screen should say what is really installed.
      await check(true, version.split(" ")[0]);
    } catch (cause) {
      logError("claude", "Mise à jour de Claude Code impossible", cause);
      setState({ kind: "failed", message: String(cause), update });
    }
  }

  return { state, check, install, dismissed, dismiss: () => setDismissed(true) };
}

export type ClaudeUpdater = ReturnType<typeof useClaudeUpdate>;

/** The update the screen is about, whatever is happening to it. */
export function pendingUpdate(state: ClaudeUpdateState): ClaudeUpdate | null {
  if (state.kind === "available" || state.kind === "updating") return state.update;
  if (state.kind === "failed") return state.update;
  return null;
}
