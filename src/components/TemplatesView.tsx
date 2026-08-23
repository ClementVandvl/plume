import { useEffect, useMemo, useState } from "react";
import { previewPreamble, saveTemplate } from "../api";
import { logError } from "../log";
import type { Template, TemplateKey } from "../types";

type Props = {
  templates: Template[];
  onSaved: () => void;
};

export function TemplatesView({ templates, onSaved }: Props) {
  const [selectedId, setSelectedId] = useState(templates[0]?.id ?? "");
  const [draft, setDraft] = useState<Template | null>(null);
  const [preamble, setPreamble] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const selected = templates.find((t) => t.id === selectedId) ?? templates[0];

  useEffect(() => {
    setDraft(selected ? structuredClone(selected) : null);
    setPreamble(null);
  }, [selected]);

  const groups = useMemo(() => {
    const map = new Map<string, TemplateKey[]>();
    for (const key of draft?.keys ?? []) {
      map.set(key.group, [...(map.get(key.group) ?? []), key]);
    }
    return [...map.entries()];
  }, [draft]);

  const dirty =
    draft && selected
      ? draft.keys.some((k, i) => k.value !== selected.keys[i]?.value)
      : false;

  function edit(key: string, value: string) {
    setDraft((current) =>
      current
        ? { ...current, keys: current.keys.map((k) => (k.key === key ? { ...k, value } : k)) }
        : current,
    );
  }

  async function save() {
    if (!draft) return;
    setSaving(true);
    setError(null);
    try {
      await saveTemplate(draft);
      onSaved();
    } catch (cause) {
      setError(String(cause));
      logError("template", "Enregistrement du modèle impossible", cause);
    } finally {
      setSaving(false);
    }
  }

  if (!draft) {
    return <p className="muted">Aucun modèle installé.</p>;
  }

  return (
    <div className="stack">
      <header className="page-head">
        <div>
          <h1 className="page-title">Modèles</h1>
          <p className="page-subtitle">
            Les réglages de votre charte. Le squelette LaTeX reste le vôtre — seules
            ces valeurs changent.
          </p>
        </div>
        <button
          type="button"
          className="btn btn--primary"
          onClick={save}
          disabled={!dirty || saving}
        >
          {saving ? "Enregistrement…" : "Enregistrer"}
        </button>
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
            </button>
          ))}
        </div>
      )}

      {error && (
        <p className="notice notice--error" role="alert">
          {error}
        </p>
      )}

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

      <section className="stack stack--tight">
        <div className="section-head">
          <h2 className="section-title">Préambule produit</h2>
          <button
            type="button"
            className="btn btn--link"
            onClick={() =>
              previewPreamble(draft.id)
                .then(setPreamble)
                .catch((cause) => setError(String(cause)))
            }
          >
            {preamble ? "Rafraîchir" : "Afficher"}
          </button>
        </div>
        {preamble && <pre className="preamble">{preamble}</pre>}
      </section>
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
