import { useMemo, useState } from "react";
import { STATUS_LABEL, type PlumeDocument, type Route } from "../types";

type Sort = "updated" | "title" | "pages";

const SORTS: { id: Sort; label: string }[] = [
  { id: "updated", label: "Modifié récemment" },
  { id: "title", label: "Titre" },
  { id: "pages", label: "Nombre de pages" },
];

const STATUSES = ["draft", "review", "ready"];

const dateFormat = new Intl.DateTimeFormat("fr-FR", {
  day: "numeric",
  month: "long",
  year: "numeric",
});

type Props = {
  documents: PlumeDocument[];
  onCreate: () => void;
  onNavigate: (route: Route) => void;
};

export function CoursesView({ documents, onCreate, onNavigate }: Props) {
  const [query, setQuery] = useState("");
  const [status, setStatus] = useState<string | null>(null);
  const [sort, setSort] = useState<Sort>("updated");

  const visible = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return documents
      .filter((d) => !status || d.status === status)
      .filter((d) => !needle || d.title.toLowerCase().includes(needle))
      .sort((a, b) => {
        if (sort === "title") return a.title.localeCompare(b.title, "fr");
        if (sort === "pages") return b.pageCount - a.pageCount;
        return b.updatedAt - a.updatedAt;
      });
  }, [documents, query, status, sort]);

  return (
    <div className="stack">
      <header className="page-head">
        <div>
          <h1 className="page-title">Mes cours</h1>
          <p className="page-subtitle">
            {documents.length} cours dans votre classeur
          </p>
        </div>
        <button type="button" className="btn btn--primary" onClick={onCreate}>
          Nouveau cours
        </button>
      </header>

      <div className="filters">
        <input
          className="input input--search"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Rechercher un cours…"
          type="search"
        />

        <div className="chips">
          <button
            type="button"
            className={`chip ${status === null ? "chip--on" : ""}`}
            onClick={() => setStatus(null)}
          >
            Tous
          </button>
          {STATUSES.map((id) => (
            <button
              key={id}
              type="button"
              className={`chip ${status === id ? "chip--on" : ""}`}
              onClick={() => setStatus(status === id ? null : id)}
            >
              {STATUS_LABEL[id]}
            </button>
          ))}
        </div>

        <select
          className="input input--compact"
          value={sort}
          onChange={(e) => setSort(e.target.value as Sort)}
          aria-label="Trier"
        >
          {SORTS.map((s) => (
            <option key={s.id} value={s.id}>
              {s.label}
            </option>
          ))}
        </select>
      </div>

      {visible.length === 0 ? (
        <p className="muted">
          {documents.length === 0
            ? "Aucun cours pour l'instant."
            : "Aucun cours ne correspond à cette recherche."}
        </p>
      ) : (
        <ul className="rows">
          {visible.map((doc) => (
            <li key={doc.id}>
              <button
                type="button"
                className="row"
                onClick={() => onNavigate({ name: "course", id: doc.id })}
              >
                <span className="row__title">{doc.title}</span>
                <span className="row__meta">
                  {doc.pageCount} page{doc.pageCount > 1 ? "s" : ""}
                </span>
                <span className="row__meta">{dateFormat.format(new Date(doc.updatedAt))}</span>
                <span className={`badge badge--${doc.status}`}>
                  {STATUS_LABEL[doc.status] ?? doc.status}
                </span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
