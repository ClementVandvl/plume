import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { clearLogs, logs as fetchLogs } from "../api";
import { logError } from "../log";
import type { LogEntry } from "../types";

const time = new Intl.DateTimeFormat("fr-FR", {
  hour: "2-digit",
  minute: "2-digit",
  second: "2-digit",
});

const SCOPE_LABEL: Record<string, string> = {
  app: "app",
  claude: "claude",
  latex: "latex",
  workspace: "fichiers",
  render: "rendu",
  template: "modèle",
};

type Props = { open: boolean; onClose: () => void };

export function Console({ open, onClose }: Props) {
  const [entries, setEntries] = useState<LogEntry[]>([]);
  const [showDebug, setShowDebug] = useState(false);
  const bottom = useRef<HTMLDivElement>(null);

  // History is loaded on open, not only streamed: a log you can read only while
  // it happens is useless once something has already gone wrong.
  useEffect(() => {
    if (!open) return;
    fetchLogs()
      .then(setEntries)
      .catch((cause) => logError("interface", "Historique du journal illisible", cause));
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

  const visible = entries.filter((e) => showDebug || e.level !== "debug");

  return (
    <aside className="console">
      <div className="console__bar">
        <span className="console__title">Console</span>
        <span className="console__count">{visible.length} lignes</span>
        <label className="console__toggle">
          <input
            type="checkbox"
            checked={showDebug}
            onChange={(e) => setShowDebug(e.target.checked)}
          />
          Détails techniques
        </label>
        <button
          type="button"
          className="btn btn--ghost"
          onClick={() => {
            clearLogs().catch((cause) => logError("interface", "Vidage du journal impossible", cause));
            setEntries([]);
          }}
        >
          Vider
        </button>
        <button type="button" className="btn btn--ghost" onClick={onClose}>
          Fermer
        </button>
      </div>

      <div className="console__body">
        {visible.length === 0 ? (
          <p className="muted console__empty">Rien à signaler pour l'instant.</p>
        ) : (
          visible.map((entry, index) => (
            <div key={`${entry.at}-${index}`} className={`log log--${entry.level}`}>
              <span className="log__time">{time.format(new Date(entry.at))}</span>
              <span className="log__scope">{SCOPE_LABEL[entry.scope] ?? entry.scope}</span>
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
