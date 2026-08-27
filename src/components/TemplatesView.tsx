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
import { logError, logInfo } from "../log";
import type { Template, TemplateKey } from "../types";

/**
 * The template editor.
 *
 * The bundled template is deliberately read-mostly. `seed` rewrites its
 * preamble whenever Plume ships a new version, so an edit made there would
 * vanish at some future update — silently, and months after the fact. Its key
 * values survive an upgrade and stay editable; changing its shape means
 * duplicating it first, which is offered right where the refusal happens.
 */

/** Kept in step with `templates::BUILTIN_ID`. */
const BUILTIN_ID = "charte-maths";

const BLOCK_LABELS: Record<string, string> = {
  chapter: "Chapitre",
  part: "Partie",
  subpart: "Sous-partie",
  paragraph: "Paragraphe",
  definition: "Définition",
  property: "Propriété",
  theorem: "Théorème",
  method: "Méthode",
  example: "Exemple",
  application: "Application",
  remark: "Remarque",
  proof: "Démonstration",
  equation: "Équation",
  list: "Liste",
  figure: "Figure",
  text: "Texte",
};

const MODES: { id: string; label: string }[] = [
  { id: "command", label: "Commande — \\nom{contenu}" },
  { id: "environment", label: "Environnement — \\begin{nom}…" },
  { id: "raw", label: "Brut — le LaTeX du bloc tel quel" },
  { id: "centered", label: "Centré — dans un center" },
];

type Tab = "keys" | "preamble" | "blocks";

export function TemplatesView({
  templates,
  onSaved,
}: {
  templates: Template[];
  onSaved: () => void;
}) {
  const confirm = useConfirm();
  const [selectedId, setSelectedId] = useState(templates[0]?.id ?? "");
  const [draft, setDraft] = useState<Template | null>(null);
  const [tab, setTab] = useState<Tab>("keys");

  const [preamble, setPreamble] = useState("");
  const [savedPreamble, setSavedPreamble] = useState("");
  const [rendered, setRendered] = useState<string | null>(null);

  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [done, setDone] = useState<string | null>(null);

  const selected = templates.find((t) => t.id === selectedId) ?? templates[0];
  const builtin = selected ? selected.id === BUILTIN_ID : false;

  useEffect(() => {
    setDraft(selected ? structuredClone(selected) : null);
    setRendered(null);
    setError(null);
    setDone(null);
    if (!selected) return;
    readTemplatePreamble(selected.id)
      .then((text) => {
        setPreamble(text);
        setSavedPreamble(text);
      })
      .catch((cause) => setError(String(cause)));
  }, [selected]);

  const groups = useMemo(() => {
    const map = new Map<string, TemplateKey[]>();
    for (const key of draft?.keys ?? []) {
      map.set(key.group, [...(map.get(key.group) ?? []), key]);
    }
    return [...map.entries()];
  }, [draft]);

  const keysDirty =
    draft && selected
      ? draft.keys.some((k, i) => k.value !== selected.keys[i]?.value) ||
        draft.name !== selected.name ||
        draft.description !== selected.description ||
        JSON.stringify(draft.blocks) !== JSON.stringify(selected.blocks)
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
      logError("template", "Action impossible sur le modèle", cause);
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
      setDone("Modèle enregistré.");
    });
  }

  async function duplicate() {
    if (!selected) return;
    const name = await confirm.promptFor({
      title: "Dupliquer le modèle",
      message: `Une copie indépendante de « ${selected.name} », que vous pourrez modifier entièrement.`,
      detail: "L'original n'est pas touché.",
      confirmLabel: "Dupliquer",
      input: { label: "Nom du nouveau modèle", value: `${selected.name} (copie)` },
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
      title: `Supprimer « ${selected.name} » ?`,
      message: "Les cours qui l'utilisent devront en choisir un autre.",
      detail: "Le modèle part à la corbeille du classeur, il n'est pas effacé.",
      confirmLabel: "Supprimer",
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
      setDone("Le modèle compile.");
    });
  }

  if (!draft || !selected) {
    return <p className="muted">Aucun modèle installé.</p>;
  }

  return (
    <div className="stack">
      <header className="page-head">
        <div>
          <h1 className="page-title">Modèles</h1>
          <p className="page-subtitle">
            Votre charte : les couleurs, le squelette LaTeX, et la façon dont chaque
            type de bloc est écrit.
          </p>
        </div>
        <div className="review__actions">
          <button
            type="button"
            className="btn btn--ghost"
            onClick={duplicate}
            disabled={busy !== null}
          >
            {busy === "duplicate" ? "Duplication…" : "Dupliquer"}
          </button>
          <button
            type="button"
            className="btn btn--primary"
            onClick={save}
            disabled={!dirty || busy !== null}
          >
            {busy === "save" ? "Enregistrement…" : "Enregistrer"}
          </button>
        </div>
      </header>

      {templates.length > 1 && (
        <div className="chips">
          {templates.map((t) => (
            <button
              key={t.id}
              type="button"
              className={`chip ${t.id === selectedId ? "chip--on" : ""}`}
              onClick={() => setSelectedId(t.id)}
            >
              {t.name}
              {t.id === BUILTIN_ID && <span className="chip__count">livré</span>}
            </button>
          ))}
        </div>
      )}

      {builtin && (
        <p className="notice">
          Modèle livré avec Plume. Ses couleurs et ses valeurs vous appartiennent et
          survivent aux mises à jour — mais son squelette LaTeX est remplacé à chaque
          nouvelle version. Dupliquez-le pour en changer la structure.
        </p>
      )}

      {error && (
        <p className="notice notice--error" role="alert">
          {error}
        </p>
      )}
      {done && <p className="notice notice--ok">{done}</p>}

      <div className="tabs" role="tablist">
        {(
          [
            ["keys", "Apparence"],
            ["preamble", "Préambule"],
            ["blocks", "Blocs"],
          ] as [Tab, string][]
        ).map(([id, label]) => (
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
          <section className="stack stack--tight">
            <h2 className="section-title">Identité</h2>
            <div className="keys">
              <label className="key">
                <span className="key__label">Nom</span>
                <input
                  className="input"
                  value={draft.name}
                  disabled={builtin}
                  onChange={(e) => setDraft({ ...draft, name: e.target.value })}
                />
                <code className="key__id">{draft.id}</code>
              </label>
              <label className="key">
                <span className="key__label">Description</span>
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

          {groups.map(([group, keys]) => (
            <section key={group} className="stack stack--tight">
              <h2 className="section-title">{group}</h2>
              <div className="keys">
                {keys.map((key) => (
                  <label key={key.key} className="key">
                    <span className="key__label">{key.label}</span>
                    <KeyInput keyDef={key} onChange={(value) => edit(key.key, value)} />
                    <code className="key__id">{key.key}</code>
                  </label>
                ))}
              </div>
            </section>
          ))}
        </>
      )}

      {tab === "preamble" && (
        <section className="stack stack--tight">
          <div className="section-head">
            <h2 className="section-title">Squelette LaTeX</h2>
            <div className="review__actions">
              <button
                type="button"
                className="btn btn--ghost"
                onClick={verify}
                disabled={busy !== null}
              >
                {busy === "check" ? "Compilation…" : "Vérifier"}
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
                {rendered ? "Rafraîchir le rendu" : "Voir valeurs substituées"}
              </button>
            </div>
          </div>
          <p className="field__hint">
            Les <code>{"{{clé}}"}</code> sont remplacés par les valeurs de l'onglet
            Apparence. « Vérifier » compile le modèle seul : une erreur se voit ici
            plutôt qu'au moment d'exporter un cours.
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
          <h2 className="section-title">Écriture des blocs</h2>
          <p className="field__hint">
            Comment chaque type de bloc reconnu est écrit en LaTeX. Un bloc sans
            correspondance sort tel quel, sans son environnement.
          </p>
          <div className="keys">
            {Object.entries(draft.blocks)
              .sort(([a], [b]) => (BLOCK_LABELS[a] ?? a).localeCompare(BLOCK_LABELS[b] ?? b))
              .map(([kind, mapping]) => (
                <label key={kind} className="key">
                  <span className="key__label">{BLOCK_LABELS[kind] ?? kind}</span>
                  <span className="key__pair">
                    <select
                      className="input"
                      value={mapping.mode}
                      disabled={builtin}
                      onChange={(e) => editBlock(kind, { mode: e.target.value })}
                    >
                      {MODES.map((mode) => (
                        <option key={mode.id} value={mode.id}>
                          {mode.label}
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

      {!builtin && (
        <section className="stack stack--tight">
          <h2 className="section-title">Zone sensible</h2>
          <div className="folder">
            <p className="folder__path">
              Supprimer « {draft.name} » — le modèle part à la corbeille du classeur.
            </p>
            <button
              type="button"
              className="btn btn--danger"
              onClick={remove}
              disabled={busy !== null}
            >
              Supprimer
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
    return (
      <select
        className="input"
        value={keyDef.value}
        onChange={(e) => onChange(e.target.value)}
      >
        {keyDef.type
          .slice("choice:".length)
          .split("|")
          .map((option) => (
            <option key={option} value={option}>
              {option}
            </option>
          ))}
      </select>
    );
  }

  return (
    <input
      className="input"
      value={keyDef.value}
      onChange={(e) => onChange(e.target.value)}
    />
  );
}
