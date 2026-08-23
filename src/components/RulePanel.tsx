import {
  EFFECT_LABEL,
  KIND_LABEL,
  TRIGGER_LABEL,
  type Effect,
  type ReadingRule,
  type Trigger,
} from "../types";

const COLOURED_TRIGGERS = new Set<Trigger["kind"]>([
  "highlight",
  "marginBar",
  "underline",
  "circled",
  "penColour",
]);

type Props = {
  rule: ReadingRule;
  onChange: (rule: ReadingRule) => void;
  onDelete: () => void;
  onClose: () => void;
};

export function RulePanel({ rule, onChange, onDelete, onClose }: Props) {
  const setTrigger = (patch: Partial<Trigger>) =>
    onChange({ ...rule, trigger: { ...rule.trigger, ...patch } });
  const setEffect = (patch: Partial<Effect>) =>
    onChange({ ...rule, effect: { ...rule.effect, ...patch } });

  return (
    <aside className="panel-side">
      <header className="panel-side__head">
        <div>
          <span className="panel-side__kind">Règle de lecture</span>
          <span className="panel-side__meta">
            {rule.enabled ? "active" : "désactivée"}
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
        <label className="check">
          <input
            type="checkbox"
            checked={rule.enabled}
            onChange={(e) => onChange({ ...rule, enabled: e.target.checked })}
          />
          Appliquer cette règle
        </label>

        <section className="stack stack--tight">
          <h3 className="section-title">Quand je vois</h3>

          <label className="field">
            <span className="field__label">Marque</span>
            <select
              className="input"
              value={rule.trigger.kind}
              onChange={(e) => setTrigger({ kind: e.target.value as Trigger["kind"] })}
            >
              {Object.entries(TRIGGER_LABEL).map(([id, label]) => (
                <option key={id} value={id}>
                  {label}
                </option>
              ))}
            </select>
          </label>

          {COLOURED_TRIGGERS.has(rule.trigger.kind) ? (
            <>
              <label className="field">
                <span className="field__label">Couleur</span>
                <span className="key__color">
                  <input
                    type="color"
                    className="swatch"
                    value={rule.trigger.colour || "#F2A93B"}
                    onChange={(e) => setTrigger({ colour: e.target.value.toUpperCase() })}
                  />
                  <input
                    className="input input--compact"
                    value={rule.trigger.colour}
                    onChange={(e) => setTrigger({ colour: e.target.value.toUpperCase() })}
                  />
                </span>
              </label>

              <label className="field">
                <span className="field__label">Nom de la couleur</span>
                <input
                  className="input"
                  value={rule.trigger.label}
                  onChange={(e) => setTrigger({ label: e.target.value })}
                  placeholder="orange"
                />
                <span className="field__hint">
                  Le nom aide autant que le code : une photo rend rarement la
                  couleur exacte.
                </span>
              </label>
            </>
          ) : (
            <label className="field">
              <span className="field__label">Décrivez la marque</span>
              <textarea
                className="input"
                rows={3}
                value={rule.trigger.label}
                onChange={(e) => setTrigger({ label: e.target.value })}
                placeholder="Une accolade tracée à gauche de plusieurs lignes."
              />
            </label>
          )}
        </section>

        <section className="stack stack--tight">
          <h3 className="section-title">Alors</h3>

          <label className="field">
            <span className="field__label">Effet</span>
            <select
              className="input"
              value={rule.effect.kind}
              onChange={(e) => setEffect({ kind: e.target.value as Effect["kind"] })}
            >
              {Object.entries(EFFECT_LABEL).map(([id, label]) => (
                <option key={id} value={id}>
                  {label}
                </option>
              ))}
            </select>
          </label>

          {rule.effect.kind === "blockKind" && (
            <label className="field">
              <span className="field__label">Type de bloc</span>
              <select
                className="input"
                value={rule.effect.value || "definition"}
                onChange={(e) => setEffect({ value: e.target.value })}
              >
                {Object.entries(KIND_LABEL).map(([id, label]) => (
                  <option key={id} value={id}>
                    {label}
                  </option>
                ))}
              </select>
            </label>
          )}

          {rule.effect.kind === "custom" && (
            <label className="field">
              <span className="field__label">Décrivez l'effet</span>
              <textarea
                className="input"
                rows={3}
                value={rule.effect.value}
                onChange={(e) => setEffect({ value: e.target.value })}
                placeholder="Transcrire ce passage en petites capitales."
              />
            </label>
          )}
        </section>
      </div>

      <footer className="panel-side__foot">
        <button type="button" className="btn btn--ghost" onClick={onDelete}>
          Supprimer
        </button>
      </footer>
    </aside>
  );
}
