import type { Environment, PlumeDocument, Route } from "../types";
import { CourseCard } from "./CourseCard";
import { Icon } from "./Sidebar";

type Props = {
  documents: PlumeDocument[];
  environment: Environment | null;
  onCreate: () => void;
  onNavigate: (route: Route) => void;
  onSettings: () => void;
};

export function HomeView({
  documents,
  environment,
  onCreate,
  onNavigate,
  onSettings,
}: Props) {
  const recent = documents.slice(0, 4);
  const toReview = documents.filter((d) => d.status === "review");
  const missing = environment?.tools.filter((t) => !t.found) ?? [];

  return (
    <div className="stack">
      <header className="page-head">
        <div>
          <h1 className="page-title">Accueil</h1>
          <p className="page-subtitle">
            Photographiez une page, Plume la réécrit dans votre charte.
          </p>
        </div>
        <button type="button" className="btn btn--primary" onClick={onCreate}>
          Nouveau cours
        </button>
      </header>

      {missing.length > 0 && (
        <button type="button" className="banner banner--warn" onClick={onSettings}>
          <Icon name="settings" />
          <span>
            {missing.length === 1
              ? `${missing[0].label} n'est pas installé.`
              : `${missing.length} prérequis manquants.`}{" "}
            Plume ne pourra pas lire vos pages. Ouvrir les réglages.
          </span>
        </button>
      )}

      {documents.length === 0 ? (
        <section className="empty">
          <h2 className="empty__title">Votre premier cours</h2>
          <p className="empty__text">
            Prenez vos pages en photo, importez-les, et Plume les lit puis les
            réécrit dans votre charte LaTeX.
          </p>
          <button type="button" className="btn btn--primary" onClick={onCreate}>
            Créer un cours
          </button>
        </section>
      ) : (
        <>
          {toReview.length > 0 && (
            <section className="stack stack--tight">
              <h2 className="section-title">À relire</h2>
              <div className="cards">
                {toReview.slice(0, 3).map((doc) => (
                  <CourseCard
                    key={doc.id}
                    document={doc}
                    onOpen={() => onNavigate({ name: "course", id: doc.id })}
                  />
                ))}
              </div>
            </section>
          )}

          <section className="stack stack--tight">
            <div className="section-head">
              <h2 className="section-title">Récents</h2>
              <button
                type="button"
                className="btn btn--link"
                onClick={() => onNavigate({ name: "courses" })}
              >
                Tout voir
              </button>
            </div>
            <div className="cards">
              {recent.map((doc) => (
                <CourseCard
                  key={doc.id}
                  document={doc}
                  onOpen={() => onNavigate({ name: "course", id: doc.id })}
                />
              ))}
            </div>
          </section>
        </>
      )}
    </div>
  );
}
