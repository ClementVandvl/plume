import { useEffect, useMemo, useState } from "react";
import {
  checkTemplate,
  deleteTemplate,
  duplicateTemplate,
  previewPreamble,
  readTemplatePreamble,
  saveTemplate,
  writeTemplatePreamble,
} from "../api";
import { useConfirm } from "../confirm";
import { t, tn } from "../i18n";
import { logError, logInfo } from "../log";
import { useAdvanced } from "../ui/mode";
import { ConventionPanel } from "./ConventionPanel";
import type { Convention, DocumentSummary, Template, TemplateKey } from "../types";
import { KIND_LABEL } from "../types";

/**
 * The house-style editor — "Mes chartes".
 *
 * A charte is more than a look: it carries the colours, the LaTeX skeleton,
 * how each kind of passage is written, and the typesetting rules the model is
 * told about. The stored object stays a `Template`, which is what the backend
 * and the workbook folder call it.
 *
 * The bundled template is deliberately read-mostly. `seed` rewrites its
 * preamble whenever Plume ships a new version, so an edit made there would
 * vanish at some future update — silently, and months after the fact. Its key
 * values survive an upgrade and stay editable; changing its shape means
 * duplicating it first, which is offered right where the refusal happens.
 *
 * In simple mode the template is its visual settings; the LaTeX skeleton and
 * the block mappings appear with the advanced mode.
 */

/** Kept in step with `templates::BUILTIN_ID`. */
const BUILTIN_ID = "charte-maths";

const MODES = [
  { id: "command", labelKey: "houseStyle.mode.command" },
  { id: "environment", labelKey: "houseStyle.mode.environment" },
  { id: "raw", labelKey: "houseStyle.mode.raw" },
  { id: "centered", labelKey: "houseStyle.mode.centered" },
] as const;

type Tab = "keys" | "rules" | "preamble" | "blocks";

/** Kept in step with `templates::size_command`. */
const SIZES = ["small", "normal", "large", "xlarge", "xxlarge"] as const;

export function HouseStyleView({
  templates,
  documents,
  onSaved,
}: {
  templates: Template[];
  documents: DocumentSummary[];
  onSaved: () => void;
}) {
  const confirm = useConfirm();
  const advanced = useAdvanced();
  const [selectedId, setSelectedId] = useState(templates[0]?.id ?? "");
  const [draft, setDraft] = useState<Template | null>(null);
  const [tab, setTab] = useState<Tab>("keys");

  const [preamble, setPreamble] = useState("");
  const [savedPreamble, setSavedPreamble] = useState("");
  const [rendered, setRendered] = useState<string | null>(null);

  const [openConvention, setOpenConvention] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [done, setDone] = useState<string | null>(null);

  const selected = templates.find((tpl) => tpl.id === selectedId) ?? templates[0];
  const builtin = selected ? selected.id === BUILTIN_ID : false;
  const usedBy = selected
    ? documents.filter((doc) => doc.templateId === selected.id).length
    : 0;

  // The technical tabs only exist in advanced mode; falling back keeps the
  // interface honest when the mode flips underneath an open tab.
  useEffect(() => {
    if (!advanced && (tab === "preamble" || tab === "blocks")) setTab("keys");
  }, [advanced, tab]);

  useEffect(() => {
    setDraft(selected ? structuredClone(selected) : null);
    setRendered(null);
    setError(null);
    setDone(null);
    setOpenConvention(null);
    if (!selected) return;
    readTemplatePreamble(selected.id)
      .then((text) => {
        setPreamble(text);
        setSavedPreamble(text);
      })
      .catch((cause) => setError(String(cause)));
  }, [selected]);

  /**
   * One row per element of the course, not one box per key.
   *
   * Colour, size and label of the same thing were three cells of a grid, in two
   * different sections. Keys sharing a suffix — `color.chapter`, `size.chapter`
   * — describe one element, so they are gathered on a single full-width line
   * and the sections collapse to the groups that remain.
   */
  const groups = useMemo(() => {
    const rows = new Map<string, { label: string; group: string; keys: TemplateKey[] }>();

    for (const key of draft?.keys ?? []) {
      const dot = key.key.indexOf(".");
      const subject = dot === -1 ? key.key : key.key.slice(dot + 1);
      const row = rows.get(subject);
      if (row) row.keys.push(key);
      else rows.set(subject, { label: key.label, group: key.group, keys: [key] });
    }

    const sections = new Map<string, { label: string; keys: TemplateKey[] }[]>();
    for (const row of rows.values()) {
      sections.set(row.group, [...(sections.get(row.group) ?? []), row]);
    }
    return [...sections.entries()];
  }, [draft]);

  const keysDirty =
    draft && selected
      ? draft.keys.some((k, i) => k.value !== selected.keys[i]?.value) ||
        draft.name !== selected.name ||
        draft.description !== selected.description ||
        JSON.stringify(draft.blocks) !== JSON.stringify(selected.blocks) ||
        JSON.stringify(draft.conventions) !== JSON.stringify(selected.conventions)
      : false;
  const preambleDirty = preamble !== savedPreamble;
  const dirty = keysDirty || preambleDirty;

  function edit(key: string, value: string) {
    setDraft((current) =>
      current
        ? { ...current, keys: current.keys.map((k) => (k.key === key ? { ...k, value } : k)) }
        : current,
    );
  }

  function addConvention() {
    if (!draft) return;
    const convention: Convention = {
      // Same as the reading rules: an index would collide after a deletion.
      id: crypto.randomUUID(),
      enabled: true,
      title: "",
      text: "",
    };
    setDraft({ ...draft, conventions: [...draft.conventions, convention] });
    setOpenConvention(convention.id);
  }

  function editBlock(kind: string, patch: { mode?: string; name?: string }) {
    setDraft((current) =>
      current
        ? {
            ...current,
            blocks: {
              ...current.blocks,
              [kind]: { ...current.blocks[kind], ...patch },
            },
          }
        : current,
    );
  }

  async function run(label: string, work: () => Promise<void>) {
    setBusy(label);
    setError(null);
    setDone(null);
    try {
      await work();
    } catch (cause) {
      setError(String(cause));
      logError("template", t("error.refresh"), cause);
    } finally {
      setBusy(null);
    }
  }

  async function save() {
    if (!draft) return;
    await run("save", async () => {
      // The preamble first: if it is refused, nothing else has been written and
      // the two halves cannot end up out of step.
      if (preambleDirty) {
        await writeTemplatePreamble(draft.id, preamble);
        setSavedPreamble(preamble);
      }
      if (keysDirty) await saveTemplate(draft);
      onSaved();
      setDone(t("houseStyle.saved"));
    });
  }

  async function duplicate() {
    if (!selected) return;
    const name = await confirm.promptFor({
      title: t("houseStyle.duplicate.title"),
      message: t("houseStyle.duplicate.message", { name: selected.name }),
      detail: t("houseStyle.duplicate.detail"),
      confirmLabel: t("common.duplicate"),
      input: {
        label: t("houseStyle.duplicate.field"),
        value: t("houseStyle.duplicate.default", { name: selected.name }),
      },
    });
    if (!name) return;

    await run("duplicate", async () => {
      const copy = await duplicateTemplate(selected.id, name);
      logInfo("template", `Modèle « ${copy.name} » créé`);
      onSaved();
      setSelectedId(copy.id);
    });
  }

  async function remove() {
    if (!selected) return;
    const ok = await confirm.confirm({
      title: t("houseStyle.delete.title", { name: selected.name }),
      message: t("houseStyle.delete.message"),
      detail: t("houseStyle.delete.detail"),
      confirmLabel: t("common.delete"),
      tone: "danger",
    });
    if (!ok) return;

    await run("delete", async () => {
      await deleteTemplate(selected.id);
      setSelectedId(BUILTIN_ID);
      onSaved();
    });
  }

  async function verify() {
    if (!draft) return;
    await run("check", async () => {
      // Saved first, so what compiles is what is on disk rather than a draft
      // that could differ from it.
      if (preambleDirty) {
        await writeTemplatePreamble(draft.id, preamble);
        setSavedPreamble(preamble);
      }
      if (keysDirty) await saveTemplate(draft);
      await checkTemplate(draft.id);
      onSaved();
      setDone(t("houseStyle.compiles"));
    });
  }

  if (!draft || !selected) {
    return <p className="muted">{t("houseStyle.none")}</p>;
  }

  const TABS: [Tab, string][] = advanced
    ? [
        ["keys", t("houseStyle.tab.keys")],
        ["rules", t("houseStyle.tab.rules")],
        ["preamble", t("houseStyle.tab.preamble")],
        ["blocks", t("houseStyle.tab.blocks")],
      ]
    : [
        ["keys", t("houseStyle.tab.keys")],
        ["rules", t("houseStyle.tab.rules")],
      ];

  return (
    <div className="stack">
      <header className="page-head">
        <div>
          <h1 className="page-title">{draft.name}</h1>
          <p className="page-subtitle">
            {builtin ? t("houseStyle.subtitle.builtin") : draft.description}
            {usedBy > 0 && <> · {tn("houseStyle.usedBy", usedBy)}</>}
          </p>
        </div>
        <div className="page-head__tools">
          <button
            type="button"
            className="btn btn--outline"
            onClick={duplicate}
            disabled={busy !== null}
          >
            {busy === "duplicate" ? t("houseStyle.duplicating") : t("common.duplicate")}
          </button>
          <button
            type="button"
            className="btn btn--primary"
            onClick={save}
            disabled={!dirty || busy !== null}
          >
            {busy === "save" ? t("common.saving") : t("common.save")}
          </button>
        </div>
      </header>

      {templates.length > 1 && (
        <div className="chips">
          {templates.map((tpl) => (
            <button
              key={tpl.id}
              type="button"
              className={`chip ${tpl.id === selectedId ? "chip--on" : ""}`}
              onClick={() => setSelectedId(tpl.id)}
            >
              {tpl.name}
              {tpl.id === BUILTIN_ID && (
                <span className="chip__count">{t("houseStyle.builtinFlag")}</span>
              )}
            </button>
          ))}
        </div>
      )}

      {builtin && advanced && <p className="notice">{t("houseStyle.builtin.notice")}</p>}

      {error && (
        <p className="notice notice--error" role="alert">
          {error}
        </p>
      )}
      {done && <p className="notice notice--ok">{done}</p>}

      <div className="tabs" role="tablist">
        {TABS.map(([id, label]) => (
          <button
            key={id}
            type="button"
            role="tab"
            aria-selected={tab === id}
            className={`tab ${tab === id ? "tab--on" : ""}`}
            onClick={() => setTab(id)}
          >
            {label}
          </button>
        ))}
      </div>

      {tab === "keys" && (
        <>
          {advanced && (
            <section className="stack stack--tight">
              <h2 className="section-title">{t("houseStyle.identity.title")}</h2>
              <div className="keys">
                <label className="key">
                  <span className="key__label">{t("houseStyle.identity.name")}</span>
                  <input
                    className="input"
                    value={draft.name}
                    disabled={builtin}
                    onChange={(e) => setDraft({ ...draft, name: e.target.value })}
                  />
                  <code className="key__id">{draft.id}</code>
                </label>
                <label className="key">
                  <span className="key__label">{t("houseStyle.identity.description")}</span>
                  <input
                    className="input"
                    value={draft.description}
                    disabled={builtin}
                    onChange={(e) => setDraft({ ...draft, description: e.target.value })}
                  />
                  <code className="key__id">{draft.engine}</code>
                </label>
              </div>
            </section>
          )}

          {groups.map(([group, rows]) => (
            <section key={group} className="stack stack--tight">
              <h2 className="section-title">{group}</h2>
              <div className="stylelines">
                {rows.map((row) => (
                  <div key={row.keys[0].key} className="styleline">
                    <span className="styleline__name">{row.label}</span>
                    <div className="styleline__controls">
                      {row.keys.map((key) => (
                        <span key={key.key} className="styleline__control">
                          <KeyInput
                            keyDef={key}
                            onChange={(value) => edit(key.key, value)}
                          />
                          {advanced && <code className="key__id">{key.key}</code>}
                        </span>
                      ))}
                    </div>
                  </div>
                ))}
              </div>
            </section>
          ))}

          {!advanced && (
            <p className="field__hint">
              {t("houseStyle.advanced.row")} — {t("titlebar.mode.advanced")}
            </p>
          )}
        </>
      )}

      {tab === "rules" && (
        <>
          {/* Outside the card, like "Mes consignes": inside it, the description
              ran straight into the first rule and the two read as one list. */}
          <p className="page-subtitle page-subtitle--wide">{t("houseStyle.rules.hint")}</p>

          <section className="listcard">
            {draft.conventions.length === 0 ? (
              <p className="listcard__empty">{t("houseStyle.rules.empty")}</p>
            ) : (
              draft.conventions.map((convention) => (
                <div
                  key={convention.id}
                  className={`arule ${convention.enabled ? "" : "arule--off"} ${
                    openConvention === convention.id ? "arule--selected" : ""
                  }`}
                  onClick={() => setOpenConvention(convention.id)}
                  role="button"
                  tabIndex={0}
                  onKeyDown={(e) => e.key === "Enter" && setOpenConvention(convention.id)}
                >
                  <span className="arule__sentence">
                    <strong>{convention.title.trim() || "—"}</strong>
                    <span className="arule__text">
                      {convention.text.trim().split("\n")[0] || "—"}
                    </span>
                  </span>
                  {!convention.enabled && (
                    <span className="flag">{t("instructions.disabled")}</span>
                  )}
                </div>
              ))
            )}

            <button type="button" className="listcard__add" onClick={addConvention}>
              {t("houseStyle.rules.add")}
            </button>
          </section>

          {openConvention !== null &&
            (() => {
              const convention = draft.conventions.find((c) => c.id === openConvention);
              if (!convention) return null;
              return (
                <ConventionPanel
                  convention={convention}
                  onChange={(next) =>
                    setDraft({
                      ...draft,
                      conventions: draft.conventions.map((c) =>
                        c.id === next.id ? next : c,
                      ),
                    })
                  }
                  onDelete={() => {
                    setDraft({
                      ...draft,
                      conventions: draft.conventions.filter((c) => c.id !== openConvention),
                    });
                    setOpenConvention(null);
                  }}
                  onClose={() => setOpenConvention(null)}
                  float
                />
              );
            })()}
        </>
      )}

      {tab === "preamble" && (
        <section className="stack stack--tight">
          <div className="section-head">
            <h2 className="section-title">{t("houseStyle.tab.preamble")}</h2>
            <div className="page-head__tools">
              <button
                type="button"
                className="btn btn--outline btn--sm"
                onClick={verify}
                disabled={busy !== null}
              >
                {busy === "check" ? t("houseStyle.verifying") : t("houseStyle.verify")}
              </button>
              <button
                type="button"
                className="btn btn--link"
                onClick={() =>
                  previewPreamble(draft.id)
                    .then(setRendered)
                    .catch((cause) => setError(String(cause)))
                }
              >
                {rendered ? t("houseStyle.preview.refresh") : t("houseStyle.preview.substituted")}
              </button>
            </div>
          </div>
          <p className="field__hint">
            {t("houseStyle.preamble.hint", { syntax: "{{clé}}" })}
          </p>
          <textarea
            className="input code-area"
            spellCheck={false}
            value={preamble}
            disabled={builtin}
            onChange={(e) => setPreamble(e.target.value)}
          />
          {rendered && <pre className="preamble">{rendered}</pre>}
        </section>
      )}

      {tab === "blocks" && (
        <section className="stack stack--tight">
          <h2 className="section-title">{t("houseStyle.blocks.title")}</h2>
          <p className="field__hint">{t("houseStyle.blocks.hint")}</p>
          <div className="keys">
            {Object.entries(draft.blocks)
              .sort(([a], [b]) => (KIND_LABEL[a] ?? a).localeCompare(KIND_LABEL[b] ?? b))
              .map(([kind, mapping]) => (
                <label key={kind} className="key">
                  <span className="key__label">{KIND_LABEL[kind] ?? kind}</span>
                  <span className="key__pair">
                    <select
                      className="input"
                      value={mapping.mode}
                      disabled={builtin}
                      onChange={(e) => editBlock(kind, { mode: e.target.value })}
                    >
                      {MODES.map((mode) => (
                        <option key={mode.id} value={mode.id}>
                          {t(mode.labelKey)}
                        </option>
                      ))}
                    </select>
                    <input
                      className="input input--compact"
                      value={mapping.name}
                      disabled={builtin || mapping.mode === "raw" || mapping.mode === "centered"}
                      placeholder="nom"
                      onChange={(e) => editBlock(kind, { name: e.target.value })}
                    />
                  </span>
                  <code className="key__id">{kind}</code>
                </label>
              ))}
          </div>
        </section>
      )}

      {!builtin && advanced && (
        <section className="stack stack--tight">
          <h2 className="section-title">{t("houseStyle.danger.title")}</h2>
          <div className="folder">
            <p className="folder__path">{t("houseStyle.danger.text", { name: draft.name })}</p>
            <button
              type="button"
              className="btn btn--danger"
              onClick={remove}
              disabled={busy !== null}
            >
              {t("common.delete")}
            </button>
          </div>
        </section>
      )}
    </div>
  );
}

function KeyInput({
  keyDef,
  onChange,
}: {
  keyDef: TemplateKey;
  onChange: (value: string) => void;
}) {
  // The stored value is a plain name; the ladder is shown in French, and the
  // LaTeX command is the backend's business.
  if (keyDef.type === "size") {
    return (
      <select
        className="input input--compact"
        value={keyDef.value}
        onChange={(e) => onChange(e.target.value)}
      >
        {SIZES.map((size) => (
          <option key={size} value={size}>
            {t(`houseStyle.size.${size}`)}
          </option>
        ))}
      </select>
    );
  }

  if (keyDef.type === "color") {
    return (
      <span className="key__color">
        <input
          type="color"
          className="swatch"
          value={keyDef.value}
          onChange={(e) => onChange(e.target.value.toUpperCase())}
        />
        <input
          className="input input--compact"
          value={keyDef.value}
          onChange={(e) => onChange(e.target.value.toUpperCase())}
        />
      </span>
    );
  }

  if (keyDef.type.startsWith("choice:")) {
    const options = keyDef.type.slice("choice:".length).split("|");
    // Few options read better as a segmented control than a dropdown.
    if (options.length <= 4) {
      return (
        <span className="seg seg--field">
          {options.map((option) => (
            <button
              key={option}
              type="button"
              className={`seg__opt ${keyDef.value === option ? "seg__opt--on" : ""}`}
              onClick={() => onChange(option)}
            >
              {option}
            </button>
          ))}
        </span>
      );
    }
    return (
      <select
        className="input"
        value={keyDef.value}
        onChange={(e) => onChange(e.target.value)}
      >
        {options.map((option) => (
          <option key={option} value={option}>
            {option}
          </option>
        ))}
      </select>
    );
  }

  // Compact like its neighbours: a full-height field made its row taller than
  // the ones beside it, for no reason a reader could see.
  return (
    <input
      className="input input--compact"
      value={keyDef.value}
      onChange={(e) => onChange(e.target.value)}
    />
  );
}
