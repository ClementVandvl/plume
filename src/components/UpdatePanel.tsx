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
 *
 * One row reads like a sentence: the app and its version on the left, what is
 * known about it underneath, the single relevant action on the right.
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

type Props = {
  /** Run a check when the panel mounts. */
  auto: boolean;
  /** Current value of the start-up check toggle. */
  enabled: boolean;
  /** Toggling saves immediately — a preference, not a form. */
  onToggleAuto: (value: boolean) => void;
};

export function UpdatePanel({ auto, enabled, onToggleAuto }: Props) {
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
        setState({ kind: "current" });
      }
    } catch (cause) {
      logError("app", "Vérification des mises à jour impossible", cause);
      // A silent check that fails should not shout: the teacher did not ask.
      setState(quiet ? { kind: "idle" } : { kind: "failed", message: String(cause) });
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

  const status = (() => {
    switch (state.kind) {
      case "unconfigured":
        return "Les mises à jour ne sont pas configurées pour cette version.";
      case "checking":
        return "Recherche en cours…";
      case "current":
        return "Plume est à jour.";
      case "available":
        return `Version ${state.update.version} disponible${
          state.update.date ? ` — publiée le ${state.update.date.slice(0, 10)}` : ""
        }.`;
      case "installing":
        return state.progress;
      case "installed":
        return "Mise à jour installée — elle s'appliquera au redémarrage.";
      default:
        return "Dernière version connue de cette machine.";
    }
  })();

  return (
    <section className="stack stack--tight">
      <h3 className="section-title">Mises à jour</h3>

      <div className="update">
        <div className="update__row">
          <div className="update__identity">
            <span className="update__name">Plume {version || "…"}</span>
            <span
              className={`update__status ${
                state.kind === "available" || state.kind === "installed"
                  ? "update__status--highlight"
                  : ""
              }`}
            >
              {status}
            </span>
          </div>

          {state.kind === "available" ? (
            <button
              type="button"
              className="btn btn--primary"
              onClick={() => install(state.update)}
            >
              Installer {state.update.version}
            </button>
          ) : state.kind === "installed" ? (
            <button
              type="button"
              className="btn btn--primary"
              onClick={() =>
                relaunch().catch((cause) =>
                  logError("app", "Redémarrage impossible", cause),
                )
              }
            >
              Redémarrer maintenant
            </button>
          ) : state.kind === "installing" ? (
            <button type="button" className="btn btn--primary" disabled>
              Installation…
            </button>
          ) : state.kind === "unconfigured" ? null : (
            <button
              type="button"
              className="btn btn--ghost"
              onClick={() => look()}
              disabled={state.kind === "checking"}
            >
              {state.kind === "checking" ? "Recherche…" : "Rechercher"}
            </button>
          )}
        </div>

        {state.kind === "available" && state.update.body && (
          <pre className="release-notes">{state.update.body}</pre>
        )}

        {state.kind === "failed" && (
          <p className="notice notice--error" role="alert">
            {state.message}
          </p>
        )}

        {state.kind !== "unconfigured" && (
          <label className="check update__auto">
            <input
              type="checkbox"
              checked={enabled}
              onChange={(event) => onToggleAuto(event.target.checked)}
            />
            Rechercher automatiquement au démarrage
          </label>
        )}
      </div>
    </section>
  );
}
