import { useEffect, useState } from "react";
import { t } from "../i18n";
import { detectPlatform, isTauri, type Platform } from "../platform";
import { logError } from "../log";
import { Icon } from "./Icon";
import { OverflowMenu } from "./controls";

export type UiMode = "simple" | "advanced";

type Props = {
  /** What the window is about right now — a course title, a view name. */
  context?: string;
  mode: UiMode;
  onMode: (mode: UiMode) => void;
  /**
   * Hides the Simple/Advancé switch.
   *
   * Set while the settings are still being read: the switch writes the whole
   * settings object back, so touching it before the stored one has arrived
   * would save the defaults over it.
   */
  bare?: boolean;
  /**
   * Opens the console.
   *
   * A direct call rather than the event the native menu emits: on Windows this
   * is the only way in, and a round trip through the backend would be one more
   * thing to be wrong about.
   */
  onConsole?: () => void;
};

/**
 * The window's own title bar — the same 44px on macOS, Windows and Linux.
 *
 * macOS keeps its native traffic lights (the bar is an overlay, so they float
 * over our left edge and we pad around them); Windows and Linux get drawn
 * buttons on the right. The Simple/Advancé switch lives here because it is a
 * window-level mode, not a page control: it follows you everywhere.
 */
export function Titlebar({ context, mode, onMode, bare = false, onConsole }: Props) {
  const [platform, setPlatform] = useState<Platform>("browser");

  useEffect(() => {
    detectPlatform().then(setPlatform).catch(() => {});
  }, []);

  async function windowAction(action: "minimize" | "maximize" | "close") {
    if (!isTauri()) return;
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
        {/* Windows and Linux draw their own buttons here, which means the
            native menu bar is gone with the decorations — and with it the only
            way to open the console. It is drawn back in. */}
        {!bare && (platform === "windows" || platform === "linux") && (
          <OverflowMenu
            named
            label={t("titlebar.tools")}
            entries={[
              {
                label: t("titlebar.tools.console"),
                icon: "dots",
                onPick: () => onConsole?.(),
              },
            ]}
          />
        )}

        {!bare && (
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
        )}

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
