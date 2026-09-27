import { t } from "../i18n";
import { Icon } from "../ui/Icon";

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
  tags: string[];
  onChange: (tags: string[]) => void;
  /** What is typed but not yet added: the caller saves it too, so a tag typed
   *  and never confirmed with Entrée is not lost on « Enregistrer ». */
  draft: string;
  onDraft: (draft: string) => void;
  /** Every tag in use across the workbook, for the suggestions. */
  known: string[];
};

/** Splits typed text on commas, trimming and dropping blanks. */
export function splitTags(text: string): string[] {
  return text
    .split(",")
    .map((tag) => tag.trim())
    .filter((tag) => tag.length > 0);
}

const same = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();

/** The tags as saved, with whatever is still typed in the field. */
export function withDraft(tags: string[], draft: string): string[] {
  return draft.trim() ? [...tags, ...splitTags(draft)] : tags;
}

export function TagField({ tags, onChange, draft, onDraft, known }: Props) {
  const suggestions = known.filter((tag) => !tags.some((mine) => same(mine, tag)));

  function add(text: string) {
    const fresh = splitTags(text).filter((tag) => !tags.some((mine) => same(mine, tag)));
    if (fresh.length > 0) onChange([...tags, ...fresh]);
    onDraft("");
  }

  return (
    <section className="stack stack--tight">
      <div className="tagrow">
        {tags.map((tag) => (
          <span key={tag} className="tagpill tagpill--editable">
            {tag}
            <button
              type="button"
              className="tagpill__remove"
              onClick={() => onChange(tags.filter((kept) => kept !== tag))}
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
          onChange={(event) => onDraft(event.target.value)}
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

      {tags.length === 0 && <p className="field__hint">{t("tags.empty")}</p>}

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
  );
}
