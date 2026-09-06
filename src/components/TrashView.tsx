import { useState } from "react";
import { purgeDocument, restoreDocument, revealWorkspace } from "../api";
import { useConfirm } from "../confirm";
import { formatRelative, t, tn } from "../i18n";
import { logError } from "../log";
import type { TrashedDocument } from "../types";
import { PageSkeleton } from "../ui/controls";

type Props = {
  trash: TrashedDocument[];
  onChanged: () => void;
};

/**
 * The bin. Nothing here is deleted without a second gesture, and restoring is
 * the most visible button — a document is weeks of handwriting.
 */
export function TrashView({ trash, onChanged }: Props) {
  const [error, setError] = useState<string | null>(null);
  const { confirm } = useConfirm();

  async function restore(document: TrashedDocument) {
    setError(null);
    try {
      await restoreDocument(document.folder);
      onChanged();
    } catch (cause) {
      setError(String(cause));
      logError("workspace", t("error.refresh"), cause);
    }
  }

  async function purge(document: TrashedDocument) {
    const ok = await confirm({
      title: t("trash.purge.title", { title: document.title }),
      message: tn("trash.purge.message", document.pageCount),
      confirmLabel: t("trash.purge.confirm"),
      cancelLabel: t("trash.purge.keep"),
      tone: "danger",
    });
    if (!ok) return;
    setError(null);
    try {
      await purgeDocument(document.folder);
      onChanged();
    } catch (cause) {
      setError(String(cause));
      logError("workspace", t("error.refresh"), cause);
    }
  }

  return (
    <div className="stack">
      <header className="page-head">
        <div>
          <h1 className="page-title">{t("trash.title")}</h1>
          <p className="page-subtitle page-subtitle--wide">{t("trash.subtitle")}</p>
        </div>
        <button
          type="button"
          className="btn btn--ghost"
          onClick={() =>
            revealWorkspace().catch((cause) =>
              logError("interface", t("error.refresh"), cause),
            )
          }
        >
          {t("common.openFolder")}
        </button>
      </header>

      {error && (
        <p className="notice notice--error" role="alert">
          {error}
        </p>
      )}

      {trash.length === 0 ? (
        <p className="muted">{t("trash.empty")}</p>
      ) : (
        <div className="listcard">
          {trash.map((document) => (
            <div key={document.folder} className="listcard__row">
              <PageSkeleton size="sm" />
              <div className="listcard__identity">
                <span className="listcard__title">{document.title}</span>
                <span className="listcard__meta">
                  {t("trash.meta", {
                    pages: tn("common.pages", document.pageCount),
                    when: formatRelative(document.trashedAt),
                  })}
                </span>
              </div>
              <button
                type="button"
                className="btn btn--primary btn--sm"
                onClick={() => restore(document)}
              >
                {t("trash.restore")}
              </button>
              <button
                type="button"
                className="btn btn--outline btn--sm"
                onClick={() => purge(document)}
              >
                {t("trash.purge")}
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
