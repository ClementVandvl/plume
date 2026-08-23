import type { Convention } from "../types";

type Props = {
  convention: Convention;
  onChange: (convention: Convention) => void;
  onDelete: () => void;
  onClose: () => void;
};

export function ConventionPanel({ convention, onChange, onDelete, onClose }: Props) {
  return (
    <aside className="panel-side">
      <header className="panel-side__head">
        <div>
          <span className="panel-side__kind">Convention</span>
          <span className="panel-side__meta">
            {convention.enabled ? "active" : "désactivée"}
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
            checked={convention.enabled}
            onChange={(e) => onChange({ ...convention, enabled: e.target.checked })}
          />
          Appliquer cette convention
        </label>

        <label className="field">
          <span className="field__label">Nom</span>
          <input
            className="input"
            value={convention.title}
            onChange={(e) => onChange({ ...convention, title: e.target.value })}
            placeholder="Annotations des schémas"
          />
          <span className="field__hint">
            Pour vous y retrouver dans la liste. Transmis au modèle en tête de la
            consigne.
          </span>
        </label>

        <label className="field">
          <span className="field__label">Consigne</span>
          <textarea
            className="input"
            rows={12}
            value={convention.text}
            onChange={(e) => onChange({ ...convention, text: e.target.value })}
            placeholder={
              "Aucune étiquette ne doit toucher ni chevaucher un trait, une flèche, un point ou une autre étiquette…"
            }
          />
          <span className="field__hint">
            Reprise mot pour mot. Écrivez-la comme vous l'expliqueriez à un
            collègue.
          </span>
        </label>
      </div>

      <footer className="panel-side__foot">
        <button type="button" className="btn btn--ghost" onClick={onDelete}>
          Supprimer
        </button>
      </footer>
    </aside>
  );
}
