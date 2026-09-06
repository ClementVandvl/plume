import { useState } from "react";
import { t } from "../i18n";
import { Icon } from "../ui/Icon";
import { Modal } from "./Modal";

/**
 * What a document is, in the teacher's words.
 *
 * Tags rather than a fixed list of kinds — cours, exercices, DS, DM,
 * interrogation — because every teacher sorts their work differently, and a
 * list Plume chose would be wrong for most of them. A tag exists by being
 * used, so "creating" one is typing it; the ones already in use are offered
 * so that « exercices » does not end up beside « exos ».
 */
type Props = {
  title: string;
  tags: string[];
  /** Every tag in use across the workbook, for the suggestions. */
  known: string[];
  busy?: boolean;
  onCancel: () => void;
  onSave: (tags: string[]) => Promise<void>;
};

/** Splits typed text on commas, trimming and dropping blanks. */
export function splitTags(text: string): string[] {
  return text
    .split(",")
    .map((tag) => tag.trim())
    .filter((tag) => tag.length > 0);
}

const same = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();

export function TagEditor({ title, tags: initial, known, busy, onCancel, onSave }: Props) {
  const [tags, setTags] = useState(initial);
  const [draft, setDraft] = useState("");

  const suggestions = known.filter((tag) => !tags.some((mine) => same(mine, tag)));

  function add(text: string) {
    const fresh = splitTags(text).filter((tag) => !tags.some((mine) => same(mine, tag)));
    if (fresh.length > 0) setTags([...tags, ...fresh]);
    setDraft("");
  }

  return (
    <Modal
      title={t("tags.title")}
      subtitle={t("tags.subtitle", { title })}
      onClose={onCancel}
      footer={
        <>
          <span className="modal__note">{tags.length === 0 && t("tags.empty")}</span>
          <div className="modal__buttons">
            <button type="button" className="btn btn--outline" onClick={onCancel} disabled={busy}>
              {t("common.cancel")}
            </button>
            <button
              type="button"
              className="btn btn--primary"
              onClick={() => onSave(draft.trim() ? [...tags, ...splitTags(draft)] : tags)}
              disabled={busy}
            >
              {busy ? t("common.saving") : t("common.save")}
            </button>
          </div>
        </>
      }
    >
      <section className="stack stack--tight">
        <div className="tagrow">
          {tags.map((tag) => (
            <span key={tag} className="tagpill tagpill--editable">
              {tag}
              <button
                type="button"
                className="tagpill__remove"
                onClick={() => setTags(tags.filter((kept) => kept !== tag))}
                aria-label={t("tags.remove", { tag })}
              >
                <Icon name="close" size={10} />
              </button>
            </span>
          ))}
        </div>

        <div className="tagadd">
          <input
            className="input input--compact"
            value={draft}
            placeholder={t("tags.add.placeholder")}
            list="tag-suggestions"
            onChange={(event) => setDraft(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter" || event.key === ",") {
                event.preventDefault();
                add(draft);
              }
            }}
          />
          <datalist id="tag-suggestions">
            {suggestions.map((tag) => (
              <option key={tag} value={tag} />
            ))}
          </datalist>
          <button
            type="button"
            className="btn btn--outline btn--sm"
            onClick={() => add(draft)}
            disabled={!draft.trim()}
          >
            {t("tags.add")}
          </button>
        </div>

        {suggestions.length > 0 && (
          <div className="tagknown">
            <span className="overline">{t("tags.known")}</span>
            <div className="tagrow">
              {suggestions.map((tag) => (
                <button
                  key={tag}
                  type="button"
                  className="tagpill tagpill--suggest"
                  onClick={() => add(tag)}
                >
                  <Icon name="plus" size={10} />
                  {tag}
                </button>
              ))}
            </div>
          </div>
        )}
      </section>
    </Modal>
  );
}
