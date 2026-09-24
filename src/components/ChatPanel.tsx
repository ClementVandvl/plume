import { useEffect, useRef, useState } from "react";
import { formatMoney, formatRelative, t, tn } from "../i18n";
import { Icon } from "../ui/Icon";
import type { ChatMessage, ChatTally, VersionSummary } from "../types";

type Props = {
  messages: ChatMessage[];
  draft: string;
  onDraft: (text: string) => void;
  /** Claude is working on a request. */
  asking: boolean;
  /** What Claude is doing right now, while it works. */
  activity: string | null;
  /** A reading or a batch of corrections holds the document. */
  busy: boolean;
  error: string | null;
  /** The request was refused because Claude Code needs signing in again. */
  auth: { lapsed: boolean; pending: boolean; onLogin: () => void };
  versions: VersionSummary[];
  /** The most recent change was Claude's, and can be taken back in one click. */
  canUndo: boolean;
  /** Money appears in the advanced mode only, as everywhere else. */
  showCost: boolean;
  onSend: (text: string) => void;
  onStop: () => void;
  onUndo: () => void;
  onClear: () => void;
  onRestore: (version: VersionSummary) => void;
  onClose: () => void;
};

const EXAMPLES = [
  "chat.example.grid",
  "chat.example.numbers",
  "chat.example.teacher",
  "chat.example.center",
] as const;

const KIND_KEY = {
  chat: "versions.kind.chat",
  corrections: "versions.kind.corrections",
  reading: "versions.kind.reading",
  restore: "versions.kind.restore",
} as const;

/** What a version is called in the list: the moment it was taken before. */
export function versionName(version: VersionSummary): string {
  return t(KIND_KEY[version.kind as keyof typeof KIND_KEY] ?? "versions.kind.chat");
}

/** Whether a reply actually changed the document. */
function changed(message: ChatMessage): boolean {
  const tally = message.tally;
  return !message.failed && !!tally && tally.edited + tally.added + tally.removed > 0;
}

function tallyLine(tally: ChatTally): string {
  const parts = [
    tally.edited > 0 ? tn("chat.tally.edited", tally.edited) : null,
    tally.added > 0 ? tn("chat.tally.added", tally.added) : null,
    tally.removed > 0 ? tn("chat.tally.removed", tally.removed) : null,
  ].filter(Boolean);
  return parts.length > 0 ? parts.join(" · ") : t("chat.tally.none");
}

/**
 * A conversation with Claude about the whole document, beside the page.
 *
 * The note on a passage asks for that passage back. This asks for anything —
 * a grid of definitions, headings numbered another way — and Claude answers
 * with the passages it changed. Every change it makes is preceded by a version,
 * so the second tab is the way back, and the last reply carries its own undo.
 */
export function ChatPanel({
  messages,
  draft,
  onDraft,
  asking,
  activity,
  busy,
  error,
  auth,
  versions,
  canUndo,
  showCost,
  onSend,
  onStop,
  onUndo,
  onClear,
  onRestore,
  onClose,
}: Props) {
  const [tab, setTab] = useState<"talk" | "versions">("talk");
  const logRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);

  // The newest turn is the one being read: keep it in view as the
  // conversation grows, and as the heartbeat under it changes.
  useEffect(() => {
    const log = logRef.current;
    if (log) log.scrollTop = log.scrollHeight;
  }, [messages.length, asking, activity, tab]);

  useEffect(() => {
    if (tab === "talk") inputRef.current?.focus();
  }, [tab]);

  const lastReply = [...messages].reverse().find((m) => m.role === "claude");
  const canSend = !asking && !busy && draft.trim().length > 0;

  function send() {
    if (canSend) onSend(draft.trim());
  }

  return (
    <aside className="panel-side chat">
      <header className="panel-side__head">
        <div className="panel-side__lead">
          <span className="panel-side__kind">{t("chat.title")}</span>
          <span className="panel-side__meta">{t("chat.subtitle")}</span>
        </div>
        <div className="panel-side__nav">
          {messages.length > 0 && !asking && (
            <button
              type="button"
              className="icon-btn"
              onClick={onClear}
              aria-label={t("chat.clear")}
              title={t("chat.clear")}
            >
              <Icon name="trash" size={13} />
            </button>
          )}
          <button
            type="button"
            className="icon-btn"
            onClick={onClose}
            aria-label={t("common.close")}
          >
            <Icon name="close" size={13} />
          </button>
        </div>
      </header>

      <div className="chat__tabs">
        <div className="seg seg--field">
          <button
            type="button"
            className={`seg__opt ${tab === "talk" ? "seg__opt--on" : ""}`}
            onClick={() => setTab("talk")}
          >
            {t("chat.tab.talk")}
          </button>
          <button
            type="button"
            className={`seg__opt ${tab === "versions" ? "seg__opt--on" : ""}`}
            onClick={() => setTab("versions")}
          >
            {t("chat.tab.versions")}
            {versions.length > 0 && <span className="seg__count">{versions.length}</span>}
          </button>
        </div>
      </div>

      {tab === "talk" ? (
        <>
          <div className="panel-side__body chat__log" ref={logRef}>
            {messages.length === 0 && !asking && (
              <div className="chat__empty">
                <span className="panel-side__kind">{t("chat.empty.title")}</span>
                <p className="field__hint">{t("chat.empty.text")}</p>
                <div className="chat__examples">
                  {EXAMPLES.map((key) => (
                    <button
                      key={key}
                      type="button"
                      className="chip"
                      onClick={() => {
                        onDraft(t(key));
                        inputRef.current?.focus();
                      }}
                    >
                      {t(key)}
                    </button>
                  ))}
                </div>
              </div>
            )}

            {messages.map((message, index) => {
              const mine = message.role === "teacher";
              return (
                <div
                  key={`${message.at}-${index}`}
                  className={`chat__msg ${mine ? "chat__msg--teacher" : "chat__msg--claude"} ${message.failed ? "chat__msg--failed" : ""}`}
                >
                  <span className="chat__who">{mine ? t("chat.you") : t("chat.claude")}</span>
                  <p className="chat__text">{message.text}</p>
                  {!mine && message.tally && !message.failed && (
                    <span className="chat__tally">
                      {tallyLine(message.tally)}
                      {showCost && message.costUsd ? ` · ${formatMoney(message.costUsd)}` : ""}
                    </span>
                  )}
                  {!mine && message === lastReply && changed(message) && canUndo && !asking && (
                    <button type="button" className="chat__undo" onClick={onUndo}>
                      <Icon name="restore" size={12} />
                      {t("chat.undo")}
                    </button>
                  )}
                </div>
              );
            })}

            {asking && (
              <div className="chat__msg chat__msg--claude">
                <span className="chat__who">{t("chat.claude")}</span>
                <span className="chat__working">
                  <span className="chat__pulse" aria-hidden="true" />
                  {activity ?? t("chat.working")}
                </span>
              </div>
            )}
          </div>

          <footer className="panel-side__foot chat__compose">
            {auth.lapsed && (
              <div className="notice notice--error notice--action" role="alert">
                <span>{auth.pending ? t("auth.pending") : t("auth.failed.title")}</span>
                {!auth.pending && (
                  <button type="button" className="btn btn--primary btn--sm" onClick={auth.onLogin}>
                    {t("auth.login")}
                  </button>
                )}
              </div>
            )}
            {error && !auth.lapsed && (
              <p className="notice notice--error" role="alert">
                {error}
              </p>
            )}
            {busy && !asking && <p className="field__hint">{t("chat.busy")}</p>}
            <textarea
              ref={inputRef}
              className="input"
              rows={3}
              value={draft}
              placeholder={t("chat.placeholder")}
              disabled={asking}
              onChange={(e) => onDraft(e.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) {
                  event.preventDefault();
                  send();
                }
              }}
            />
            <div className="chat__actions">
              <span className="field__hint">{t("chat.hint")}</span>
              {asking ? (
                <button type="button" className="btn btn--outline btn--sm" onClick={onStop}>
                  {t("chat.stop")}
                </button>
              ) : (
                <button
                  type="button"
                  className="btn btn--primary btn--sm"
                  onClick={send}
                  disabled={!canSend}
                >
                  <Icon name="send" size={13} />
                  {t("chat.send")}
                </button>
              )}
            </div>
          </footer>
        </>
      ) : (
        <div className="panel-side__body">
          <p className="field__hint">{t("versions.hint")}</p>
          {versions.length === 0 ? (
            <p className="muted">{t("versions.empty")}</p>
          ) : (
            <ol className="versions">
              {versions.map((version) => (
                <li key={version.id} className="version">
                  <div className="version__head">
                    <span className="version__title">{versionName(version)}</span>
                    <span className="version__meta">
                      {formatRelative(version.createdAt)} · {tn("versions.blocks", version.blocks)}
                    </span>
                  </div>
                  {version.label && (
                    <span className="version__label">
                      {version.kind === "chat"
                        ? `« ${version.label} »`
                        : version.kind === "restore"
                          ? t("versions.restoreOf", { label: version.label })
                          : version.label}
                    </span>
                  )}
                  {version.restorable ? (
                    <button
                      type="button"
                      className="btn btn--outline btn--sm"
                      onClick={() => onRestore(version)}
                      disabled={asking || busy}
                    >
                      <Icon name="restore" size={13} />
                      {t("versions.restore")}
                    </button>
                  ) : (
                    <span className="field__hint">{t("versions.stale")}</span>
                  )}
                </li>
              ))}
            </ol>
          )}
        </div>
      )}
    </aside>
  );
}
