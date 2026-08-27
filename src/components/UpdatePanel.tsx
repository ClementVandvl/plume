import { useEffect, useState } from "react";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { getVersion } from "@tauri-apps/api/app";
import { updatesConfigured } from "../api";
import { logError, logInfo } from "../log";

/**
 * Update checking and installing.
 *
 * Checking may happen on its own; installing never does. Replacing the
 * application someone is working in is not a decision to take on their behalf,
 * and a teacher mid-transcription least of all.
 */

type State =
  | { kind: "unconfigured" }
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "current" }
  | { kind: "available"; update: Update }
  | { kind: "installing"; progress: string }
  | { kind: "installed" }
  | { kind: "failed"; message: string };

export function UpdatePanel({ auto }: { auto: boolean }) {
  const [state, setState] = useState<State>({ kind: "idle" });
  const [version, setVersion] = useState("");

  useEffect(() => {
    getVersion().then(setVersion).catch(() => {});
  }, []);

  useEffect(() => {
    let cancelled = false;

    updatesConfigured()
      .then((configured) => {
        if (cancelled) return;
        if (!configured) {
          setState({ kind: "unconfigured" });
          return;
        }
        // Only the check is automatic, and only when enabled.
        if (auto) look(true);
      })
      .catch(() => {});

    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [auto]);

  async function look(quiet = false) {
    setState({ kind: "checking" });
    try {
      const update = await check();
      if (update) {
        logInfo("app", `Mise à jour disponible : ${update.version}`);
        setState({ kind: "available", update });
      } else {
        setState(quiet ? { kind: "idle" } : { kind: "current" });
      }
    } catch (cause) {
      // A silent check that fails should not shout: the teacher did not ask.
      if (quiet) {
        setState({ kind: "idle" });
        logError("app", "Vérification des mises à jour impossible", cause);
        return;
      }
      setState({ kind: "failed", message: String(cause) });
      logError("app", "Vérification des mises à jour impossible", cause);
    }
  }

  async function install(update: Update) {
    setState({ kind: "installing", progress: "Téléchargement…" });
    try {
      let downloaded = 0;
      let total = 0;

      await update.downloadAndInstall((event) => {
        if (event.event === "Started") {
          total = event.data.contentLength ?? 0;
        } else if (event.event === "Progress") {
          downloaded += event.data.chunkLength;
          const share = total ? Math.round((downloaded / total) * 100) : 0;
          setState({
            kind: "installing",
            progress: total ? `Téléchargement… ${share} %` : "Téléchargement…",
          });
        } else if (event.event === "Finished") {
          setState({ kind: "installing", progress: "Installation…" });
        }
      });

      logInfo("app", `Mise à jour ${update.version} installée`);
      setState({ kind: "installed" });
    } catch (cause) {
      setState({ kind: "failed", message: String(cause) });
      logError("app", "Installation de la mise à jour impossible", cause);
    }
  }

  return (
    <section className="stack stack--tight">
      <h3 className="section-title">Mises à jour</h3>

      <p className="field__hint">
        Version installée {version || "…"}.
        {state.kind === "unconfigured" &&
          " Les mises à jour ne sont pas encore configurées pour cette version."}
      </p>

      {state.kind === "available" && (
        <div className="banner banner--info">
          <span>
            <strong>Plume {state.update.version}</strong> est disponible.
            {state.update.date && ` Publiée le ${state.update.date.slice(0, 10)}.`}
          </span>
          <button
            type="button"
            className="btn btn--primary"
            onClick={() => install(state.update)}
          >
            Installer
          </button>
        </div>
      )}

      {state.kind === "available" && state.update.body && (
        <pre className="release-notes">{state.update.body}</pre>
      )}

      {state.kind === "installing" && <p className="notice">{state.progress}</p>}

      {state.kind === "installed" && (
        <div className="banner banner--info">
          <span>Mise à jour installée. Elle s'appliquera au redémarrage.</span>
          <button
            type="button"
            className="btn btn--primary"
            onClick={() => relaunch().catch((cause) => logError("app", "Redémarrage impossible", cause))}
          >
            Redémarrer maintenant
          </button>
        </div>
      )}

      {state.kind === "current" && <p className="notice">Plume est à jour.</p>}

      {state.kind === "failed" && (
        <p className="notice notice--error" role="alert">
          {state.message}
        </p>
      )}

      {state.kind !== "unconfigured" &&
        state.kind !== "installing" &&
        state.kind !== "installed" && (
          <div className="row-actions">
            <button
              type="button"
              className="btn btn--ghost"
              onClick={() => look()}
              disabled={state.kind === "checking"}
            >
              {state.kind === "checking" ? "Recherche…" : "Rechercher une mise à jour"}
            </button>
          </div>
        )}
    </section>
  );
}
