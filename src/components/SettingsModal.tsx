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
import { t } from "../i18n";
import { logError } from "../log";
import { useAdvanced } from "../ui/mode";
import type { Environment, Settings } from "../types";
import { Icon } from "../ui/Icon";
import { AdvancedRow, Toggle } from "../ui/controls";
import { UpdatePanel } from "./UpdatePanel";
import { Modal } from "./Modal";

type Props = {
  environment: Environment | null;
  onEnvironmentChanged: () => void;
  workspace: string;
  settings: Settings;
  onSaved: (settings: Settings) => void;
  onClose: () => void;
};

const MODELS = [
  { id: "sonnet", labelKey: "model.sonnet" },
  { id: "opus", labelKey: "model.opus" },
  { id: "fable", labelKey: "model.fable" },
] as const;

const THEMES = ["light", "dark", "system"] as const;

export function SettingsModal({
  environment,
  onEnvironmentChanged,
  workspace,
  settings,
  onSaved,
  onClose,
}: Props) {
  const advanced = useAdvanced();
  const [installing, setInstalling] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [showAdvanced, setShowAdvanced] = useState(advanced);

  // Every control here saves on the spot: these are preferences, not a form,
  // and a toggle that waits for a distant "Enregistrer" reads as broken.
  async function persist(patch: Partial<Settings>) {
    try {
      const next = { ...settings, ...patch };
      await saveSettings(next);
      onSaved(next);
    } catch (cause) {
      setError(String(cause));
      logError("workspace", t("error.refresh"), cause);
    }
  }

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
    setInstalling(t("settings.tool.installing"));
    setError(null);
    try {
      if (tool === "claude") await installClaude();
      else await installEngine();
      onEnvironmentChanged();
    } catch (cause) {
      setError(String(cause));
      logError(tool === "claude" ? "claude" : "latex", t("error.refresh"), cause);
    } finally {
      setInstalling(null);
    }
  }

  return (
    <Modal
      title={t("settings.title")}
      subtitle={t("settings.subtitle")}
      onClose={onClose}
      footer={
        <>
          <span className="modal__note">
            <UpdateFootnote />
          </span>
          <button type="button" className="btn btn--primary" onClick={onClose}>
            {t("settings.done")}
          </button>
        </>
      }
    >
      <section className="stack stack--tight">
        <h3 className="section-title">{t("settings.needs.title")}</h3>
        <div className="listcard">
          {(environment?.tools ?? []).map((tool) => (
            <div
              key={tool.key}
              className={`toolrow ${tool.found ? "" : "toolrow--missing"}`}
            >
              {tool.found ? (
                <span className="toolrow__ok">
                  <Icon name="check" size={13} />
                </span>
              ) : (
                <span className="toolrow__warn">
                  <Icon name="warning" size={19} />
                </span>
              )}
              <div className="toolrow__body">
                <span className="toolrow__label">{tool.label}</span>
                <span className={`toolrow__hint ${tool.found ? "" : "toolrow__hint--warn"}`}>
                  {tool.found ? tool.role : (tool.hint ?? tool.role)}
                </span>
                {tool.found && tool.key === "claude" && (
                  <button
                    type="button"
                    className="btn btn--link"
                    onClick={() =>
                      openClaudeLogin().catch((cause) =>
                        logError("claude", t("error.refresh"), cause),
                      )
                    }
                  >
                    {t("settings.tool.login")}
                  </button>
                )}
              </div>
              {tool.found ? (
                <span className="toolrow__state">{t("settings.tool.ready")}</span>
              ) : tool.installable ? (
                <button
                  type="button"
                  className="btn btn--primary btn--sm"
                  onClick={() => provision(tool.key)}
                  disabled={installing !== null}
                >
                  {installing ?? t("settings.tool.install")}
                </button>
              ) : (
                <button
                  type="button"
                  className="btn btn--link"
                  onClick={() =>
                    openUrl(tool.installUrl).catch((cause) =>
                      logError("interface", t("error.refresh"), cause),
                    )
                  }
                >
                  {t("settings.tool.installExternal", { name: tool.label })}
                </button>
              )}
            </div>
          ))}
        </div>
      </section>

      <section className="stack stack--tight">
        <h3 className="section-title">{t("settings.appearance.title")}</h3>
        <div className="themes">
          {THEMES.map((theme) => (
            <button
              key={theme}
              type="button"
              className={`theme ${settings.theme === theme ? "theme--on" : ""}`}
              onClick={() => persist({ theme })}
            >
              <span className={`theme__swatch theme__swatch--${theme}`}>
                <span className="theme__rail" />
                <span className="theme__page" />
              </span>
              {t(`settings.theme.${theme}`)}
            </button>
          ))}
        </div>
      </section>

      <section className="stack stack--tight">
        <h3 className="section-title">{t("settings.general.title")}</h3>

        <div className="setting">
          <div className="setting__copy">
            <span className="setting__label">{t("settings.updates.title")}</span>
            <span className="setting__hint">{t("settings.updates.hint")}</span>
          </div>
          <Toggle
            checked={settings.checkUpdates}
            onChange={(value) => persist({ checkUpdates: value })}
            label={t("settings.updates.title")}
          />
        </div>

        <div className="setting">
          <div className="setting__copy">
            <span className="setting__label">{t("settings.folder.title")}</span>
            <span className="setting__hint setting__hint--path">{workspace || "…"}</span>
          </div>
          <button
            type="button"
            className="btn btn--outline btn--sm"
            onClick={() =>
              revealWorkspace().catch((cause) =>
                logError("interface", t("error.refresh"), cause),
              )
            }
          >
            {t("common.openFolder")}
          </button>
        </div>

        <AdvancedRow
          text={t("settings.advanced.row")}
          open={showAdvanced}
          onToggle={() => setShowAdvanced((open) => !open)}
        >
          <div className="adv__body">
            <label className="field">
              <span className="field__label">{t("settings.model.label")}</span>
              <select
                className="input"
                value={settings.defaultModel}
                onChange={(e) => persist({ defaultModel: e.target.value })}
              >
                {MODELS.map((m) => (
                  <option key={m.id} value={m.id}>
                    {t(m.labelKey)}
                  </option>
                ))}
              </select>
              <span className="field__hint">{t("settings.model.hint")}</span>
            </label>

            <label className="field">
              <span className="field__label">{t("settings.parallel.label")}</span>
              <select
                className="input"
                value={settings.concurrentPages}
                onChange={(e) => persist({ concurrentPages: Number(e.target.value) })}
              >
                <option value={0}>
                  {environment
                    ? environment.memoryGb
                      ? t("settings.parallel.autoAt", {
                          pages: environment.autoPages,
                          memory: Math.round(environment.memoryGb),
                        })
                      : t("settings.parallel.autoUnknown", { pages: environment.autoPages })
                    : t("settings.parallel.auto")}
                </option>
                <option value={1}>{t("settings.parallel.one")}</option>
                <option value={2}>2</option>
                <option value={3}>{t("settings.parallel.three")}</option>
              </select>
              <span className="field__hint">{t("settings.parallel.hint")}</span>
            </label>
          </div>
        </AdvancedRow>
      </section>

      <UpdatePanel
        auto={settings.checkUpdates}
        enabled={settings.checkUpdates}
        onToggleAuto={(value) => persist({ checkUpdates: value })}
      />

      {error && (
        <p className="notice notice--error" role="alert">
          {error}
        </p>
      )}
    </Modal>
  );
}

/** "Version 0.4 · à jour" in the footer — quiet, no button. */
function UpdateFootnote() {
  const [version, setVersion] = useState("");
  useEffect(() => {
    import("@tauri-apps/api/app")
      .then(({ getVersion }) => getVersion())
      .then(setVersion)
      .catch(() => {});
  }, []);
  return <>{version ? t("settings.version", { version }) : ""}</>;
}
