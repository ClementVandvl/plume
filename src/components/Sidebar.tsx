import { openUrl } from "../api";
import { t } from "../i18n";
import { logError } from "../log";
import type { Route } from "../types";
import { Icon } from "../ui/Icon";

type Props = {
  route: Route;
  onNavigate: (route: Route) => void;
  onSettings: () => void;
  courseCount: number;
  trashCount: number;
  environmentReady: boolean;
};

const HELP_URL = "https://github.com/ClementVandvl/plume#readme";

const SECTIONS = [
  { name: "home", labelKey: "nav.home", icon: "home" },
  { name: "courses", labelKey: "nav.courses", icon: "book" },
  { name: "houseStyle", labelKey: "nav.houseStyle", icon: "palette" },
  { name: "instructions", labelKey: "nav.instructions", icon: "marker" },
] as const;

export function Sidebar({
  route,
  onNavigate,
  onSettings,
  courseCount,
  trashCount,
  environmentReady,
}: Props) {
  return (
    <nav className="sidebar">
      <span className="sidebar__label">{t("nav.section")}</span>

      <ul className="sidebar__nav">
        {SECTIONS.map((section) => (
          <li key={section.name}>
            <button
              type="button"
              className={`nav-item ${route.name === section.name ? "nav-item--active" : ""}`}
              onClick={() => onNavigate({ name: section.name } as Route)}
            >
              <Icon name={section.icon} />
              {t(section.labelKey)}
              {section.name === "courses" && courseCount > 0 && (
                <span className="nav-item__count">{courseCount}</span>
              )}
            </button>
          </li>
        ))}
      </ul>

      <div className="sidebar__foot">
        {trashCount > 0 && (
          <button
            type="button"
            className={`nav-item ${route.name === "trash" ? "nav-item--active" : ""}`}
            onClick={() => onNavigate({ name: "trash" })}
          >
            <Icon name="trash" />
            {t("nav.trash")}
            <span className="nav-item__count">{trashCount}</span>
          </button>
        )}
        <button
          type="button"
          className="nav-item"
          onClick={() =>
            openUrl(HELP_URL).catch((cause) =>
              logError("interface", t("error.refresh"), cause),
            )
          }
        >
          <Icon name="help" />
          {t("nav.help")}
        </button>
        <button type="button" className="nav-item" onClick={onSettings}>
          <Icon name="settings" />
          {t("nav.settings")}
          {!environmentReady && (
            <span className="nav-item__dot" aria-label={t("nav.settings.missing")} />
          )}
        </button>
      </div>
    </nav>
  );
}
