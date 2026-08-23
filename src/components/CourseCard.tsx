import { STATUS_LABEL, type PlumeDocument } from "../types";

const dateFormat = new Intl.DateTimeFormat("fr-FR", {
  day: "numeric",
  month: "short",
});

export function CourseCard({
  document,
  onOpen,
}: {
  document: PlumeDocument;
  onOpen: () => void;
}) {
  return (
    <button type="button" className="card" onClick={onOpen}>
      <span className={`badge badge--${document.status}`}>
        {STATUS_LABEL[document.status] ?? document.status}
      </span>
      <span className="card__title">{document.title}</span>
      <span className="card__meta">
        {document.pageCount} page{document.pageCount > 1 ? "s" : ""} ·{" "}
        {dateFormat.format(new Date(document.updatedAt))}
      </span>
    </button>
  );
}
