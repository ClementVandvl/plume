import type { Route } from "../types";

type Props = {
  route: Route;
  onNavigate: (route: Route) => void;
  onSettings: () => void;
  onConsole: () => void;
  courseCount: number;
  environmentReady: boolean;
};

const SECTIONS: { name: Route["name"]; label: string; icon: string; soon?: boolean }[] = [
  { name: "home", label: "Accueil", icon: "home" },
  { name: "courses", label: "Mes cours", icon: "book" },
  { name: "templates", label: "Modèles", icon: "palette" },
  { name: "rules", label: "Règles de lecture", icon: "marker" },
];

export function Sidebar({
  route,
  onNavigate,
  onSettings,
  onConsole,
  courseCount,
  environmentReady,
}: Props) {
  return (
    <nav className="sidebar">
      <div className="sidebar__brand">
        <Icon name="feather" />
        <span className="sidebar__wordmark">Plume</span>
      </div>

      <ul className="sidebar__nav">
        {SECTIONS.map((section) => (
          <li key={section.name}>
            <button
              type="button"
              className={`nav-item ${route.name === section.name ? "nav-item--active" : ""}`}
              onClick={() => onNavigate({ name: section.name } as Route)}
            >
              <Icon name={section.icon} />
              {section.label}
              {section.name === "courses" && courseCount > 0 && (
                <span className="nav-item__count">{courseCount}</span>
              )}
            </button>
          </li>
        ))}
      </ul>

      <div className="sidebar__foot">
        <button type="button" className="nav-item" onClick={onConsole}>
          <Icon name="terminal" />
          Console
        </button>
        <button type="button" className="nav-item" onClick={onSettings}>
          <Icon name="settings" />
          Réglages
          {!environmentReady && <span className="nav-item__dot" aria-label="Prérequis manquant" />}
        </button>
      </div>
    </nav>
  );
}

const PATHS: Record<string, string> = {
  feather: "M20 4c0 8-5.4 12.5-12 12.5H5C5 9 10 4 17 4h3M4 21 15 10",
  home: "M4 10 12 3l8 7v10a1 1 0 0 1-1 1h-4v-6H9v6H5a1 1 0 0 1-1-1z",
  book: "M5 4h11a3 3 0 0 1 3 3v13H8a3 3 0 0 0-3 3zM8 20h11",
  palette: "M12 3a9 9 0 1 0 0 18c1.1 0 2-.9 2-2 0-1.4-1-1.7-1-3 0-1.1.9-2 2-2h2a4 4 0 0 0 4-4 9 9 0 0 0-9-7M7.5 11.5h.01M11 8h.01M15.5 9h.01",
  marker:
    "M4 20h4l10.5-10.5a2.1 2.1 0 0 0-3-3L5 17zM14 6l4 4M4 20l1-3",
  terminal: "M4 5h16v14H4zM8 10l2.5 2.5L8 15M13 15h4",
  settings:
    "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6M19.4 15a1.6 1.6 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.6 1.6 0 0 0-2.7 1.1v.3a2 2 0 1 1-4 0v-.2a1.6 1.6 0 0 0-2.8-1.1l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.6 1.6 0 0 0-1.1-2.7H3a2 2 0 1 1 0-4h.2A1.6 1.6 0 0 0 4.3 6l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.6 1.6 0 0 0 2.7-1.1V2a2 2 0 1 1 4 0v.2a1.6 1.6 0 0 0 2.7 1.1l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.6 1.6 0 0 0 1.1 2.7h.3a2 2 0 1 1 0 4h-.2a1.6 1.6 0 0 0-1.1 1z",
};

export function Icon({ name }: { name: string }) {
  return (
    <svg className="icon" viewBox="0 0 24 24" fill="none" aria-hidden="true">
      <path
        d={PATHS[name] ?? ""}
        stroke="currentColor"
        strokeWidth="1.7"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}
