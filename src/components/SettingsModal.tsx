import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  installClaude,
  installEngine,
  openClaudeLogin,
  openUrl,
  revealWorkspace,
  saveSettings,
} from "../api";
import { logError } from "../log";
import type { Environment, Settings, Template } from "../types";
import { Modal } from "./Modal";

type Props = {
  environment: Environment | null;
  onEnvironmentChanged: () => void;
  templates: Template[];
  workspace: string;
  settings: Settings;
  onSaved: (settings: Settings) => void;
  onClose: () => void;
};

const MODELS = [
  { id: "sonnet", label: "Sonnet — rapide, recommandé" },
  { id: "opus", label: "Opus — plus lent, plus fin" },
  { id: "fable", label: "Fable" },
];

export function SettingsModal({
  environment,
  onEnvironmentChanged,
  templates,
  workspace,
  settings,
  onSaved,
  onClose,
}: Props) {
  const [draft, setDraft] = useState(settings);
  const [installing, setInstalling] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Reading rules live on their own page now; this modal only owns the model.
  const dirty = draft.defaultModel !== settings.defaultModel;

  // The engine download reports coarse steps: it runs once, and what matters is
  // that it is progressing.
  useEffect(() => {
    const stop = listen<string>("provision", (event) => setInstalling(event.payload));
    return () => {
      stop.then((off) => off()).catch(() => {});
    };
  }, []);

  // One handler for both prerequisites: they differ only in what they run.
  async function provision(tool: string) {
    setInstalling("Préparation…");
    setError(null);
    try {
      if (tool === "claude") await installClaude();
      else await installEngine();
      onEnvironmentChanged();
    } catch (cause) {
      setError(String(cause));
      logError(tool === "claude" ? "claude" : "latex", "Installation impossible", cause);
    } finally {
      setInstalling(null);
    }
  }

  async function persist() {
    setSaving(true);
    setError(null);
    try {
      await saveSettings(draft);
      onSaved(draft);
    } catch (cause) {
      setError(String(cause));
      logError("workspace", "Enregistrement des réglages impossible", cause);
    } finally {
      setSaving(false);
    }
  }

  return (
    <Modal title="Réglages" onClose={onClose}>
      <section className="stack stack--tight">
        <h3 className="section-title">État du système</h3>
        <ul className="tools">
          {(environment?.tools ?? []).map((tool) => (
            <li key={tool.key} className="tool">
              <span
                className={`dot ${tool.found ? "dot--ok" : "dot--missing"}`}
                aria-hidden="true"
              />
              <div className="tool__body">
                <div className="tool__head">
                  <span className="tool__label">{tool.label}</span>
                  {tool.version && <code className="tool__version">{tool.version}</code>}
                </div>
                <p className="tool__role">{tool.role}</p>
                {tool.found ? (
                  <>
                    <p className="tool__path" title={tool.path ?? ""}>
                      {tool.path}
                    </p>
                    {tool.key === "claude" && (
                      <button
                        type="button"
                        className="btn btn--link"
                        onClick={() =>
                          openClaudeLogin().catch((cause) =>
                            logError("claude", "Ouverture du terminal impossible", cause),
                          )
                        }
                      >
                        Se connecter dans un terminal ↗
                      </button>
                    )}
                  </>
                ) : (
                  <>
                    <p className="tool__hint">{tool.hint}</p>
                    {tool.installable ? (
                      <button
                        type="button"
                        className="btn btn--primary tool__install"
                        onClick={() => provision(tool.key)}
                        disabled={installing !== null}
                      >
                        {installing ?? `Installer ${tool.label}`}
                      </button>
                    ) : (
                      <button
                        type="button"
                        className="btn btn--link"
                        onClick={() =>
                          openUrl(tool.installUrl).catch((cause) =>
                            logError("interface", "Action impossible", cause),
                          )
                        }
                      >
                        Installer {tool.label} ↗
                      </button>
                    )}
                  </>
                )}
              </div>
            </li>
          ))}
        </ul>
      </section>

      <section className="stack stack--tight">
        <h3 className="section-title">Lecture</h3>
        <label className="field">
          <span className="field__label">Modèle par défaut</span>
          <select
            className="input"
            value={draft.defaultModel}
            onChange={(e) => setDraft({ ...draft, defaultModel: e.target.value })}
          >
            {MODELS.map((m) => (
              <option key={m.id} value={m.id}>
                {m.label}
              </option>
            ))}
          </select>
          <span className="field__hint">
            Modifiable cours par cours au moment de la lecture.
          </span>
        </label>

        {error && (
          <p className="notice notice--error" role="alert">
            {error}
          </p>
        )}

        <div className="row-actions">
          <button
            type="button"
            className="btn btn--primary"
            onClick={persist}
            disabled={!dirty || saving}
          >
            {saving ? "Enregistrement…" : "Enregistrer"}
          </button>
        </div>
      </section>

      <section className="stack stack--tight">
        <h3 className="section-title">Classeur</h3>
        <div className="folder">
          <p className="folder__path">{workspace || "…"}</p>
          <button
            type="button"
            className="btn btn--ghost"
            onClick={() => revealWorkspace().catch((cause) => logError("interface", "Action impossible", cause))}
          >
            Ouvrir
          </button>
        </div>
        <p className="field__hint">
          {templates.length} modèle{templates.length > 1 ? "s" : ""} installé
          {templates.length > 1 ? "s" : ""}.
        </p>
      </section>
    </Modal>
  );
}
