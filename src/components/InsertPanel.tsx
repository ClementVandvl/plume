import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { t } from "../i18n";
import { latexToHtml } from "../preview/latexToHtml";
import { KIND_LABEL } from "../types";
import { Modal } from "./Modal";

/**
 * Adding a passage a reading missed, between two others.
 *
 * Two ways in, because a teacher notices the gap in two different situations.
 * Either they know what belongs there and type it — a sentence, a formula —
 * or the missing part is still on paper, and photographing it is faster than
 * typing it.
 *
 * The photograph is not an aside: it joins the document's pages, at the position
 * the gap is in, and is read like any other. Pages and transcription have to
 * stay in step — every part of the app relies on it.
 */

type Mode = "latex" | "photo";

/**
 * Headings are their title: no LaTeX body, and a handwritten number the
 * charte writes but never invents. They sit in their own group so the
 * difference reads before the form does.
 */
const HEADINGS = ["chapter", "part", "subpart", "paragraph"];

/** Everything else carries its content in LaTeX. */
const PASSAGES = [
  "text",
  "definition",
  "property",
  "theorem",
  "method",
  "example",
  "application",
  "remark",
  "proof",
  "equation",
  "list",
];

export const isHeading = (kind: string) => HEADINGS.includes(kind);

export function InsertPanel({
  busy,
  onClose,
  onWrite,
  onPhoto,
}: {
  busy: boolean;
  onClose: () => void;
  onWrite: (kind: string, title: string, number: string, latex: string) => Promise<void>;
  onPhoto: (source: string) => Promise<void>;
}) {
  const [mode, setMode] = useState<Mode>("latex");
  const [kind, setKind] = useState("text");
  const [title, setTitle] = useState("");
  const [number, setNumber] = useState("");
  const [latex, setLatex] = useState("");

  // A heading needs a title and nothing else; a passage needs its content.
  const heading = isHeading(kind);
  const complete = heading ? title.trim().length > 0 : latex.trim().length > 0;

  async function pick() {
    const picked = await open({
      multiple: false,
      filters: [
        {
          name: "Images",
          extensions: ["jpg", "jpeg", "png", "heic", "heif", "webp", "tif", "tiff"],
        },
      ],
    });
    const source = Array.isArray(picked) ? picked[0] : picked;
    if (source) await onPhoto(source);
  }

  return (
    <Modal title={t("insert.title")} onClose={onClose}>
      <div className="tabs" role="tablist">
        {(
          [
            ["latex", t("insert.tab.latex")],
            ["photo", t("insert.tab.photo")],
          ] as [Mode, string][]
        ).map(([id, label]) => (
          <button
            key={id}
            type="button"
            role="tab"
            aria-selected={mode === id}
            className={`tab ${mode === id ? "tab--on" : ""}`}
            onClick={() => setMode(id)}
          >
            {label}
          </button>
        ))}
      </div>

      {mode === "latex" ? (
        <section className="stack stack--tight">
          <div className="keys">
            <label className="key">
              <span className="key__label">{t("insert.kind")}</span>
              <select
                className="input input--compact"
                value={kind}
                onChange={(event) => setKind(event.target.value)}
              >
                <optgroup label={t("insert.kind.headings")}>
                  {HEADINGS.map((id) => (
                    <option key={id} value={id}>
                      {KIND_LABEL[id] ?? id}
                    </option>
                  ))}
                </optgroup>
                <optgroup label={t("insert.kind.passages")}>
                  {PASSAGES.map((id) => (
                    <option key={id} value={id}>
                      {KIND_LABEL[id] ?? id}
                    </option>
                  ))}
                </optgroup>
              </select>
            </label>
            <label className="key">
              <span className="key__label">
                {heading ? t("insert.heading.title") : t("insert.name")}
              </span>
              <input
                className="input input--compact"
                value={title}
                placeholder={heading ? t("insert.heading.placeholder") : t("insert.name.placeholder")}
                onChange={(event) => setTitle(event.target.value)}
              />
            </label>
            {heading && (
              <label className="key">
                <span className="key__label">{t("insert.number")}</span>
                <input
                  className="input input--compact"
                  value={number}
                  placeholder={t("insert.number.placeholder")}
                  onChange={(event) => setNumber(event.target.value)}
                />
              </label>
            )}
          </div>

          {heading ? (
            <p className="field__hint">{t("insert.heading.hint")}</p>
          ) : (
            <label className="field">
              <span className="field__label">{t("insert.latex")}</span>
              <textarea
                className="input input--code"
                rows={7}
                spellCheck={false}
                value={latex}
                placeholder={t("insert.latex.placeholder")}
                onChange={(event) => setLatex(event.target.value)}
              />
            </label>
          )}

          {!heading && latex.trim() && (
            <>
              <span className="overline">{t("insert.preview")}</span>
              <div
                className="insert__preview"
                dangerouslySetInnerHTML={{ __html: latexToHtml(latex) }}
              />
            </>
          )}

          <div className="review__actions">
            <button
              type="button"
              className="btn btn--primary"
              onClick={() => onWrite(kind, title, number, heading ? "" : latex)}
              disabled={busy || !complete}
            >
              {busy ? t("common.saving") : t("insert.add")}
            </button>
          </div>
        </section>
      ) : (
        <section className="stack stack--tight">
          <p className="field__hint">{t("insert.photo.hint")}</p>
          <div className="review__actions">
            <button
              type="button"
              className="btn btn--primary"
              onClick={pick}
              disabled={busy}
            >
              {busy ? t("insert.photo.reading") : t("insert.photo.pick")}
            </button>
          </div>
        </section>
      )}
    </Modal>
  );
}
