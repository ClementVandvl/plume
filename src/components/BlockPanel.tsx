import { useEffect, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import {
  AUDIENCE_LABEL,
  DOUBT_THRESHOLD,
  KIND_LABEL,
  type Block,
} from "../types";

const AUDIENCES = ["teacher", "student"];

type Props = {
  block: Block;
  page: number;
  pageSrc?: string;
  onClose: () => void;
  onSave: (block: Block) => Promise<void>;
  onNote: (note: string | null) => Promise<void>;
};

/**
 * Side panel for one block. The teacher can fix it by hand, confirm it, or
 * annotate it for a targeted re-run — the model is never the last word.
 */
export function BlockPanel({ block, page, pageSrc, onClose, onSave, onNote }: Props) {
  const [draft, setDraft] = useState(block);
  const [note, setNote] = useState(block.note ?? "");
  const [saving, setSaving] = useState(false);
  const [showSource, setShowSource] = useState(false);

  // A correction replaces the block wholesale, so the local draft follows
  // rather than keep showing the version being corrected.
  useEffect(() => {
    setDraft(block);
    setNote(block.note ?? "");
  }, [block]);

  const flagged = block.confidence < DOUBT_THRESHOLD && !block.reviewed;
  const dirty =
    draft.latex !== block.latex ||
    draft.title !== block.title ||
    draft.kind !== block.kind ||
    draft.audience.join() !== block.audience.join();

  async function save() {
    setSaving(true);
    try {
      await onSave(draft);
      if (note.trim() !== (block.note ?? "")) await onNote(note.trim() || null);
    } finally {
      setSaving(false);
    }
  }

  return (
    <aside className="panel-side">
      <header className="panel-side__head">
        <div>
          <span className="panel-side__kind">{KIND_LABEL[block.kind] ?? block.kind}</span>
          <span className="panel-side__meta">
            page {page} · certitude {Math.round(block.confidence * 100)} %
          </span>
        </div>
        <button
          type="button"
          className="icon-btn icon-btn--close"
          onClick={onClose}
          aria-label="Fermer"
        >
          ×
        </button>
      </header>

      <div className="panel-side__body">
        {block.doubt && (
          <p className={`block__doubt ${flagged ? "" : "block__doubt--quiet"}`}>
            {flagged && <span className="block__doubt-mark">!</span>}
            {block.doubt}
          </p>
        )}

        {pageSrc && (
          <div className="stack stack--tight">
            <button
              type="button"
              className="btn btn--ghost"
              onClick={() => setShowSource((open) => !open)}
            >
              {showSource ? "Masquer la photo" : "Comparer à la photo"}
            </button>
            {showSource && (
              <img className="source__image" src={convertFileSrc(pageSrc)} alt={`Page ${page}`} />
            )}
          </div>
        )}

        <div className="block__row">
          <label className="field">
            <span className="field__label">Type</span>
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
            <span className="field__label">Titre (facultatif)</span>
            <input
              className="input"
              value={draft.title ?? ""}
              onChange={(e) => setDraft({ ...draft, title: e.target.value || null })}
            />
          </label>
        </div>

        <label className="field">
          <span className="field__label">Contenu LaTeX</span>
          <textarea
            className="input input--code"
            rows={Math.min(18, Math.max(4, draft.latex.split("\n").length + 1))}
            value={draft.latex}
            onChange={(e) => setDraft({ ...draft, latex: e.target.value })}
          />
        </label>

        <div className="field">
          <span className="field__label">Présent dans</span>
          <div className="checks">
            {AUDIENCES.map((audience) => (
              <label key={audience} className="check">
                <input
                  type="checkbox"
                  checked={draft.audience.includes(audience)}
                  onChange={(e) =>
                    setDraft({
                      ...draft,
                      audience: e.target.checked
                        ? [...draft.audience, audience]
                        : draft.audience.filter((a) => a !== audience),
                    })
                  }
                />
                {AUDIENCE_LABEL[audience]}
              </label>
            ))}
          </div>
        </div>

        <label className="field">
          <span className="field__label">
            Demander une correction — relancée sur ce bloc seul
          </span>
          <textarea
            className="input"
            rows={3}
            value={note}
            onChange={(e) => setNote(e.target.value)}
            placeholder="Le vecteur descend vers M, il ne va pas à l'horizontale."
          />
        </label>
      </div>

      <footer className="panel-side__foot">
        {/* Confirming a doubt is a first-class action: without it the flag could
            only be cleared by editing something, which is wrong when the
            transcription was right all along. */}
        {flagged && (
          <button
            type="button"
            className="btn btn--ghost"
            onClick={() => onSave(block)}
            disabled={saving}
          >
            C'est correct
          </button>
        )}
        <button
          type="button"
          className="btn btn--primary"
          onClick={save}
          disabled={saving || (!dirty && note.trim() === (block.note ?? ""))}
        >
          {saving ? "Enregistrement…" : "Enregistrer"}
        </button>
      </footer>
    </aside>
  );
}
