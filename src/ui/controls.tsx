import { useEffect, useRef, useState, type ReactNode } from "react";
import { t } from "../i18n";
import type { DocumentStatus } from "../types";
import { Icon, type IconName } from "./Icon";

/**
 * The small reusable controls of the design system. Anything with more than a
 * screen's worth of behaviour gets its own file; these are the atoms.
 */

// ---------------------------------------------------------------- switch
export function Toggle({
  checked,
  onChange,
  label,
}: {
  checked: boolean;
  onChange: (value: boolean) => void;
  label: string;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      className={`toggle ${checked ? "toggle--on" : ""}`}
      onClick={() => onChange(!checked)}
    >
      <span className="toggle__knob" />
    </button>
  );
}

// ----------------------------------------------------------------- meter
type MeterTone = "ok" | "warn" | "muted" | "accent";

/** A thin progress pill; `share` is 0 to 1. */
export function Meter({ share, tone = "ok" }: { share: number; tone?: MeterTone }) {
  const width = `${Math.round(Math.min(1, Math.max(0, share)) * 100)}%`;
  return (
    <span className="meter" role="presentation">
      <span className={`meter__fill meter__fill--${tone}`} style={{ width }} />
    </span>
  );
}

// ----------------------------------------------------------- status pill
export function StatusPill({ status }: { status: DocumentStatus }) {
  return (
    <span className={`status-pill status-pill--${status}`}>
      <span className="status-pill__dot" />
      {t(`status.${status}`)}
    </span>
  );
}

// ------------------------------------------------------------- overflow menu
export type MenuEntry = {
  label: string;
  icon?: IconName;
  danger?: boolean;
  onPick: () => void;
};

/** The "⋯" menu: a plain popover, closed by a click anywhere or Escape. */
export function OverflowMenu({ label, entries }: { label: string; entries: MenuEntry[] }) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onDown = (event: MouseEvent) => {
      if (!root.current?.contains(event.target as Node)) setOpen(false);
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    window.addEventListener("mousedown", onDown);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("mousedown", onDown);
      window.removeEventListener("keydown", onKey);
    };
  }, [open]);

  return (
    <div className="menu" ref={root}>
      <button
        type="button"
        className="menu__trigger"
        aria-label={label}
        aria-expanded={open}
        onClick={(event) => {
          event.stopPropagation();
          setOpen((current) => !current);
        }}
      >
        <Icon name="dots" />
      </button>

      {open && (
        <div className="menu__list" role="menu">
          {entries.map((entry) => (
            <button
              key={entry.label}
              type="button"
              role="menuitem"
              className={`menu__item ${entry.danger ? "menu__item--danger" : ""}`}
              onClick={(event) => {
                event.stopPropagation();
                setOpen(false);
                entry.onPick();
              }}
            >
              {entry.icon && <Icon name={entry.icon} />}
              {entry.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

// ------------------------------------------------------------ page skeleton
/**
 * The little fake document standing in for a page thumbnail — used wherever a
 * real photo would be noise (lists, trash, resume card).
 */
export function PageSkeleton({ size = "md" }: { size?: "sm" | "md" | "lg" }) {
  return (
    <span className={`skeleton skeleton--${size}`} aria-hidden="true">
      <span className="skeleton__line skeleton__line--head" />
      <span className="skeleton__line" />
      <span className="skeleton__block" />
      <span className="skeleton__line skeleton__line--short" />
    </span>
  );
}

// ------------------------------------------------------------- section rows
/** The "Avancé ⌄" inset row that reveals more when clicked. */
export function AdvancedRow({
  text,
  open,
  onToggle,
  children,
}: {
  text: string;
  open: boolean;
  onToggle: () => void;
  children?: ReactNode;
}) {
  return (
    <div className="adv">
      <button type="button" className="adv__row" onClick={onToggle} aria-expanded={open}>
        <span className="adv__text">{text}</span>
        <span className={`adv__mark ${open ? "adv__mark--open" : ""}`}>
          {t("common.advanced")} <Icon name="chevron-down" size={12} />
        </span>
      </button>
      {open && children}
    </div>
  );
}
