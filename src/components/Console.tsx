import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { clearLogs, logs as fetchLogs } from "../api";
import { formatTime, t } from "../i18n";
import { logError } from "../log";
import type { LogEntry } from "../types";
import { Icon } from "../ui/Icon";

const SCOPE_KEYS = [
  "app",
  "claude",
  "latex",
  "workspace",
  "render",
  "template",
  "interface",
  "ir",
] as const;

function scopeLabel(scope: string): string {
  return (SCOPE_KEYS as readonly string[]).includes(scope)
    ? t(`console.scope.${scope as (typeof SCOPE_KEYS)[number]}`)
    : scope;
}

type Filter = "all" | "warn" | "error";

type Props = { open: boolean; onClose: () => void };

/**
 * The technical journal — a dark drawer at the bottom, never open by default.
 * Everything the simple mode hides ends up traceable here.
 */
export function Console({ open, onClose }: Props) {
  const [entries, setEntries] = useState<LogEntry[]>([]);
  const [filter, setFilter] = useState<Filter>("all");
  const [showDebug, setShowDebug] = useState(false);
  const bottom = useRef<HTMLDivElement>(null);

  // History is loaded on open, not only streamed: a log you can read only while
  // it happens is useless once something has already gone wrong.
  useEffect(() => {
    if (!open) return;
    fetchLogs()
      .then(setEntries)
      .catch((cause) => logError("interface", t("error.refresh"), cause));
  }, [open]);

  useEffect(() => {
    const stop = listen<LogEntry>("log", (event) => {
      setEntries((current) => [...current.slice(-799), event.payload]);
    });
    return () => {
      stop.then((off) => off()).catch(() => {});
    };
  }, []);

  useEffect(() => {
    if (open) bottom.current?.scrollIntoView({ block: "end" });
  }, [entries, open]);

  if (!open) return null;

  const warnings = entries.filter((e) => e.level === "warn").length;
  const errors = entries.filter((e) => e.level === "error").length;
  const visible = entries
    .filter((e) => showDebug || e.level !== "debug")
    .filter((e) =>
      filter === "all" ? true : filter === "warn" ? e.level === "warn" : e.level === "error",
    );

  return (
    <aside className="console">
      <div className="console__bar">
        <span className="console__title">{t("console.title")}</span>
        <span className="console__shortcut">{t("console.shortcut")}</span>
        <span className="console__spacer" />
        <button
          type="button"
          className={`console__chip ${filter === "all" ? "console__chip--on" : ""}`}
          onClick={() => setFilter("all")}
        >
          {t("console.filter.all")}
        </button>
        <button
          type="button"
          className={`console__chip console__chip--warn ${filter === "warn" ? "console__chip--on" : ""}`}
          onClick={() => setFilter(filter === "warn" ? "all" : "warn")}
          disabled={warnings === 0}
        >
          {t("console.filter.warnings")} {warnings > 0 ? warnings : ""}
        </button>
        <button
          type="button"
          className={`console__chip console__chip--error ${filter === "error" ? "console__chip--on" : ""}`}
          onClick={() => setFilter(filter === "error" ? "all" : "error")}
          disabled={errors === 0}
        >
          {t("console.filter.errors")} {errors > 0 ? errors : ""}
        </button>
        <label className="console__toggle">
          <input
            type="checkbox"
            checked={showDebug}
            onChange={(e) => setShowDebug(e.target.checked)}
          />
          {t("console.debug")}
        </label>
        <button
          type="button"
          className="console__chip"
          onClick={() => {
            clearLogs().catch((cause) => logError("interface", t("error.refresh"), cause));
            setEntries([]);
          }}
        >
          {t("console.clear")}
        </button>
        <button
          type="button"
          className="console__close"
          onClick={onClose}
          aria-label={t("common.close")}
        >
          <Icon name="close" size={13} />
        </button>
      </div>

      <div className="console__body">
        {visible.length === 0 ? (
          <p className="console__empty">{t("console.empty")}</p>
        ) : (
          visible.map((entry, index) => (
            <div key={`${entry.at}-${index}`} className={`log log--${entry.level}`}>
              <span className="log__time">{formatTime(entry.at)}</span>
              <span className="log__scope">{scopeLabel(entry.scope)}</span>
              <span className="log__message">
                {entry.message}
                {entry.detail && <span className="log__detail">{entry.detail}</span>}
              </span>
            </div>
          ))
        )}
        <div ref={bottom} />
      </div>
    </aside>
  );
}
