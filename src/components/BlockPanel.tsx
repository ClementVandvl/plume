import { useEffect, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { t } from "../i18n";
import { useAdvanced } from "../ui/mode";
import { SplitPanel } from "./SplitPanel";
import { Icon } from "../ui/Icon";
import { AdvancedRow } from "../ui/controls";
import { DOUBT_THRESHOLD, KIND_LABEL, type Block } from "../types";

type Props = {
  block: Block;
  page: number;
  /** Where this block sits in the course, 1-based, and how many there are. */
  position: number;
  total: number;
  pageSrc?: string;
  onClose: () => void;
  onPrev: () => void;
  onNext: () => void;
  onSave: (block: Block) => Promise<void>;
  onNote: (note: string | null) => Promise<void>;
  /** Replaces this passage with two. */
  onSplit: (head: string, tail: string) => Promise<void>;
};

/** Canned starts for the note — the frequent reasons a passage is wrong. */
const WRONG_KEYS = [
  "panel.wrong.formula",
  "panel.wrong.kind",
  "panel.wrong.missing",
  "panel.wrong.teacher",
] as const;

/**
 * Side panel for one passage. The teacher corrects in French first — the
 * photo, the doubt, and "qu'est-ce qui ne va pas ?" — and the LaTeX stays
 * underneath for when they want the scalpel. The model is never the last word.
 */
export function BlockPanel({
  block,
  page,
  position,
  total,
  pageSrc,
  onClose,
  onPrev,
  onNext,
  onSave,
  onNote,
  onSplit,
}: Props) {
  const advanced = useAdvanced();
  const [draft, setDraft] = useState(block);
  const [note, setNote] = useState(block.note ?? "");
  const [saving, setSaving] = useState(false);
  const [photoLarge, setPhotoLarge] = useState(false);
  const [latexOpen, setLatexOpen] = useState(advanced);
  const [splitting, setSplitting] = useState(false);

  // A correction replaces the block wholesale, so the local draft follows
  // rather than keep showing the version being corrected.
  useEffect(() => {
    setDraft(block);
    setNote(block.note ?? "");
    setSplitting(false);
  }, [block]);

  const flagged = block.confidence < DOUBT_THRESHOLD && !block.reviewed;
  const dirty =
    draft.latex !== block.latex ||
    draft.title !== block.title ||
    draft.kind !== block.kind ||
    draft.audience.join() !== block.audience.join();
  const noteDirty = note.trim() !== (block.note ?? "");

  async function save() {
    setSaving(true);
    try {
      await onSave(draft);
      if (noteDirty) await onNote(note.trim() || null);
    } finally {
      setSaving(false);
    }
  }

  async function approve() {
    setSaving(true);
    try {
      await onSave({ ...block, reviewed: true });
      onNext();
    } finally {
      setSaving(false);
    }
  }

  // An empty audience means "everywhere". The panel shows that as both boxes
  // ticked, and normalises "both ticked" back to empty on the way out.
  function toggleAudience(who: string) {
    setDraft((current) => {
      const effective =
        current.audience.length === 0 ? ["teacher", "student"] : current.audience;
      const next = effective.includes(who)
        ? effective.filter((a) => a !== who)
        : [...effective, who];
      return { ...current, audience: next.length >= 2 ? [] : next };
    });
  }

  const inAudience = (who: string) =>
    draft.audience.length === 0 || draft.audience.includes(who);

  return (
    <aside className="panel-side">
      <header className="panel-side__head">
        <div className="panel-side__lead">
          <span className="panel-side__kind">
            {t("panel.position", { index: position, total })}
          </span>
          <span className="panel-side__meta">
            {t("panel.meta", { kind: KIND_LABEL[block.kind] ?? block.kind, page })}
          </span>
        </div>
        <div className="panel-side__nav">
          <button
            type="button"
            className="icon-btn"
            onClick={onPrev}
            aria-label={t("panel.previous")}
          >
            <Icon name="arrow-up" size={13} />
          </button>
          <button
            type="button"
            className="icon-btn"
            onClick={onNext}
            aria-label={t("panel.next")}
          >
            <Icon name="arrow-down" size={13} />
          </button>
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

      <div className="panel-side__body">
        {block.doubt && (
          <div className={`doubt ${flagged ? "" : "doubt--quiet"}`}>
            <Icon name="info" size={17} />
            <span>{block.doubt}</span>
          </div>
        )}

        {pageSrc && (
          <div className="stack stack--tight">
            <span className="overline">{t("panel.photo.title")}</span>
            <div className={`excerpt ${photoLarge ? "excerpt--large" : ""}`}>
              <img src={convertFileSrc(pageSrc)} alt={t("pages.page", { number: page })} />
              <button
                type="button"
                className="excerpt__zoom"
                onClick={() => setPhotoLarge((large) => !large)}
              >
                {photoLarge ? t("panel.photo.reduce") : t("panel.photo.enlarge")}
              </button>
            </div>
          </div>
        )}

        <div className="stack stack--tight">
          <span className="overline">{t("panel.wrong.title")}</span>
          <div className="chips">
            {WRONG_KEYS.map((key) => (
              <button
                key={key}
                type="button"
                className="chip"
                onClick={() =>
                  setNote((current) =>
                    current.trim() ? `${current.trim()} ${t(key)} : ` : `${t(key)} : `,
                  )
                }
              >
                {t(key)}
              </button>
            ))}
          </div>
          <textarea
            className="input"
            rows={3}
            value={note}
            onChange={(e) => setNote(e.target.value)}
            placeholder={t("panel.note.placeholder")}
          />
          <span className="field__hint">{t("panel.note.hint")}</span>
        </div>

        <div className="block__row">
          <label className="field">
            <span className="field__label">{t("panel.kind.label")}</span>
            <select
              className="input"
              value={draft.kind}
              onChange={(e) => setDraft({ ...draft, kind: e.target.value })}
            >
              {Object.entries(KIND_LABEL).map(([id, label]) => (
                <option key={id} value={id}>
                  {label}
                </option>
              ))}
            </select>
          </label>

          <label className="field">
            <span className="field__label">{t("panel.title.label")}</span>
            <input
              className="input"
              value={draft.title ?? ""}
              onChange={(e) => setDraft({ ...draft, title: e.target.value || null })}
            />
          </label>
        </div>

        <div className="stack stack--tight">
          <span className="overline">{t("panel.audience.title")}</span>
          <div className="audience">
            {(
              [
                ["teacher", t("panel.audience.teacher")],
                ["student", t("panel.audience.student")],
              ] as const
            ).map(([who, label]) => (
              <button
                key={who}
                type="button"
                className={`audience__opt ${inAudience(who) ? "audience__opt--on" : ""}`}
                onClick={() => toggleAudience(who)}
              >
                <span className="audience__box">
                  {inAudience(who) && <Icon name="check" size={10} />}
                </span>
                {label}
              </button>
            ))}
          </div>
        </div>

        <div className="panel-side__tool">
          <button
            type="button"
            className="btn btn--outline btn--sm"
            onClick={() => setSplitting((current) => !current)}
            disabled={saving}
          >
            {splitting ? t("common.cancel") : t("split.action")}
          </button>
          <span className="field__hint">{t("split.why")}</span>
        </div>

        {splitting && (
          <SplitPanel
            latex={block.latex}
            onCancel={() => setSplitting(false)}
            onSplit={async (head, tail) => {
              await onSplit(head, tail);
              setSplitting(false);
            }}
          />
        )}

        <AdvancedRow
          text={t("panel.latex.title")}
          open={latexOpen}
          onToggle={() => setLatexOpen((current) => !current)}
        >
          <div className="adv__body">
            <label className="field">
              <span className="field__label">{t("panel.latex.label")}</span>
              <textarea
                className="input input--code"
                rows={Math.min(18, Math.max(4, draft.latex.split("\n").length + 1))}
                value={draft.latex}
                onChange={(e) => setDraft({ ...draft, latex: e.target.value })}
              />
            </label>
          </div>
        </AdvancedRow>
      </div>

      <footer className="panel-side__foot">
        <div className="panel-side__buttons">
          <button
            type="button"
            className="btn btn--ok"
            onClick={approve}
            disabled={saving}
          >
            {t("panel.confirm")}
          </button>
          <button
            type="button"
            className="btn btn--primary"
            onClick={save}
            disabled={saving || (!dirty && !noteDirty)}
          >
            {saving
              ? t("common.saving")
              : note.trim()
                ? t("panel.fix")
                : t("panel.save")}
          </button>
        </div>
        <span className="panel-side__shortcut">{t("panel.shortcuts")}</span>
      </footer>
    </aside>
  );
}
