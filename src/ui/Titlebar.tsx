import { useEffect, useState } from "react";
import { t } from "../i18n";
import { detectPlatform, isTauri, type Platform } from "../platform";
import { logError } from "../log";
import { Icon } from "./Icon";

export type UiMode = "simple" | "advanced";

type Props = {
  /** What the window is about right now — a course title, a view name. */
  context?: string;
  mode: UiMode;
  onMode: (mode: UiMode) => void;
};

/**
 * The window's own title bar — the same 44px on macOS, Windows and Linux.
 *
 * macOS keeps its native traffic lights (the bar is an overlay, so they float
 * over our left edge and we pad around them); Windows and Linux get drawn
 * buttons on the right. The Simple/Advancé switch lives here because it is a
 * window-level mode, not a page control: it follows you everywhere.
 */
export function Titlebar({ context, mode, onMode }: Props) {
  const [platform, setPlatform] = useState<Platform>("browser");

  useEffect(() => {
    detectPlatform().then(setPlatform).catch(() => {});
  }, []);

  async function windowAction(action: "minimize" | "maximize" | "close") {
    if (!isTauri) return;
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      const current = getCurrentWindow();
      if (action === "minimize") await current.minimize();
      else if (action === "maximize") await current.toggleMaximize();
      else await current.close();
    } catch (cause) {
      logError("interface", "Action sur la fenêtre impossible", cause);
    }
  }

  return (
    <header
      className={`titlebar ${platform === "macos" ? "titlebar--mac" : ""}`}
      data-tauri-drag-region
    >
      <div className="titlebar__brand" data-tauri-drag-region>
        <Icon name="feather" />
        <span className="titlebar__name">{t("app.name")}</span>
        {context && <span className="titlebar__context">— {context}</span>}
      </div>

      <div className="titlebar__side">
        <div className="seg" role="group" aria-label={t("common.advanced")}>
          <button
            type="button"
            className={`seg__opt ${mode === "simple" ? "seg__opt--on" : ""}`}
            onClick={() => onMode("simple")}
          >
            {t("titlebar.mode.simple")}
          </button>
          <button
            type="button"
            className={`seg__opt ${mode === "advanced" ? "seg__opt--on seg__opt--dark" : ""}`}
            onClick={() => onMode("advanced")}
          >
            {t("titlebar.mode.advanced")}
          </button>
        </div>

        {(platform === "windows" || platform === "linux") && (
          <div className="titlebar__controls">
            <button
              type="button"
              className="titlebar__btn"
              onClick={() => windowAction("minimize")}
              aria-label={t("titlebar.minimize")}
            >
              <Icon name="minus" size={13} />
            </button>
            <button
              type="button"
              className="titlebar__btn"
              onClick={() => windowAction("maximize")}
              aria-label={t("titlebar.maximize")}
            >
              <Icon name="square" size={11} />
            </button>
            <button
              type="button"
              className="titlebar__btn titlebar__btn--close"
              onClick={() => windowAction("close")}
              aria-label={t("titlebar.close")}
            >
              <Icon name="close" size={13} />
            </button>
          </div>
        )}
      </div>
    </header>
  );
}
