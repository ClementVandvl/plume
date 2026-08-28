/**
 * The icon plate.
 *
 * One stroke style — 1.7, round caps — drawn from the same 24×24 grid, so any
 * icon can sit in any control without re-tuning. Icons are inline SVG paths:
 * no font, no sprite sheet, and `currentColor` follows the text around.
 */

const PATHS: Record<string, string> = {
  feather: "M20 4c0 8-5.4 12.5-12 12.5H5C5 9 10 4 17 4h3M4 21 15 10",
  home: "M4 10 12 3l8 7v10a1 1 0 0 1-1 1h-4v-6H9v6H5a1 1 0 0 1-1-1z",
  book: "M5 4h11a3 3 0 0 1 3 3v13H8a3 3 0 0 0-3 3zM8 20h11",
  palette:
    "M12 3a9 9 0 1 0 0 18c1.1 0 2-.9 2-2 0-1.4-1-1.7-1-3 0-1.1.9-2 2-2h2a4 4 0 0 0 4-4 9 9 0 0 0-9-7M7.5 11.5h.01M11 8h.01M15.5 9h.01",
  marker: "M4 20h4l10.5-10.5a2.1 2.1 0 0 0-3-3L5 17zM14 6l4 4M4 20l1-3",
  trash: "M4 7h16M9 7V5h6v2m-9 0 1 13h10l1-13M10 11v5m4-5v5",
  restore: "M4 11a8 8 0 1 0 2.3-5.7M4 4v6h6",
  help: "M12 17v.01M12 14c0-2 2.2-2.3 2.2-4A2.2 2.2 0 0 0 9.8 9.8M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18",
  terminal: "M4 5h16v14H4zM8 10l2.5 2.5L8 15M13 15h4",
  settings:
    "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6M19.4 15a1.6 1.6 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.6 1.6 0 0 0-2.7 1.1v.3a2 2 0 1 1-4 0v-.2a1.6 1.6 0 0 0-2.8-1.1l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.6 1.6 0 0 0-1.1-2.7H3a2 2 0 1 1 0-4h.2A1.6 1.6 0 0 0 4.3 6l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.6 1.6 0 0 0 2.7-1.1V2a2 2 0 1 1 4 0v.2a1.6 1.6 0 0 0 2.7 1.1l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.6 1.6 0 0 0 1.1 2.7h.3a2 2 0 1 1 0 4h-.2a1.6 1.6 0 0 0-1.1 1z",
  search: "m20 20-4.5-4.5M17 11a6 6 0 1 1-12 0 6 6 0 0 1 12 0",
  upload:
    "M4 16.5V18a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-1.5M12 3v11m0-11 4 4m-4-4-4 4",
  back: "m14 6-8 6 8 6",
  "chevron-down": "m6 9 6 6 6-6",
  warning:
    "M12 9v5m0 3v.01M10.3 3.9 2.6 17.2A1.6 1.6 0 0 0 4 19.6h16a1.6 1.6 0 0 0 1.4-2.4L13.7 3.9a1.6 1.6 0 0 0-2.8 0",
  info: "M12 8v5m0 3v.01M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18",
  check: "m5 12 5 5 9-10",
  close: "M6 6l12 12M18 6 6 18",
  plus: "M12 5v14M5 12h14",
  minus: "M5 12h14",
  square: "M6 6h12v12H6z",
  dots: "M5 12h.01M12 12h.01M19 12h.01",
  "arrow-up": "M12 19V5m0 0-6 6m6-6 6 6",
  "arrow-down": "M12 5v14m0 0-6-6m6 6 6-6",
  folder: "M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z",
  grip: "M9 6h.01M9 12h.01M9 18h.01M15 6h.01M15 12h.01M15 18h.01",
  pdf: "M6 3h9l4 4v14H6zM14 3v5h5M9 13h6M9 17h6",
};

export type IconName = keyof typeof PATHS & string;

export function Icon({ name, size }: { name: IconName; size?: number }) {
  return (
    <svg
      className="icon"
      viewBox="0 0 24 24"
      fill="none"
      aria-hidden="true"
      style={size ? { width: size, height: size } : undefined}
    >
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
