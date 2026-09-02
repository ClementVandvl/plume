import { createContext, useCallback, useContext, useRef, useState, type ReactNode } from "react";
import { t } from "./i18n";
import { Icon } from "./ui/Icon";

/**
 * Confirmation for anything that destroys work.
 *
 * Native `window.confirm` was doing this job: out of the app's typography, no
 * room to say what is actually lost, and no way to name the button after the
 * action. Here the consequence is spelled out and the button says what it does
 * — "Mettre à la corbeille", not "OK".
 */

type Tone = "danger" | "neutral";

type Ask = {
  title: string;
  message: string;
  /** What exactly is lost or kept. Shown under the message. */
  detail?: string;
  confirmLabel: string;
  /** "Garder le cours" beats "Annuler" when the stake is a deletion. */
  cancelLabel?: string;
  tone?: Tone;
  /** Present for a rename-style dialog. */
  input?: { label: string; value: string; placeholder?: string };
  /**
   * What is about to be lost, shown as it appears in the course.
   *
   * Naming a passage is not enough to recognise it: an id means nothing, and a
   * title is often absent. Seeing the passage itself is the only way to be sure
   * of what a deletion takes.
   */
  preview?: ReactNode;
};

type Pending = Ask & { resolve: (value: string | null) => void };

type Api = {
  /** Resolves true when confirmed. */
  confirm: (ask: Omit<Ask, "input">) => Promise<boolean>;
  /** Resolves the typed value, or null when cancelled. */
  promptFor: (ask: Ask & { input: NonNullable<Ask["input"]> }) => Promise<string | null>;
};

const ConfirmContext = createContext<Api | null>(null);

export function useConfirm(): Api {
  const api = useContext(ConfirmContext);
  if (!api) throw new Error("useConfirm must be used inside ConfirmProvider");
  return api;
}

export function ConfirmProvider({ children }: { children: ReactNode }) {
  const [pending, setPending] = useState<Pending | null>(null);
  const [value, setValue] = useState("");
  const api = useRef<Api>({
    confirm: () => Promise.resolve(false),
    promptFor: () => Promise.resolve(null),
  });

  const request = useCallback((ask: Ask) => {
    setValue(ask.input?.value ?? "");
    return new Promise<string | null>((resolve) => {
      setPending({ ...ask, resolve });
    });
  }, []);

  api.current = {
    confirm: (ask) => request(ask).then((answer) => answer !== null),
    promptFor: (ask) => request(ask),
  };

  const close = (answer: string | null) => {
    pending?.resolve(answer);
    setPending(null);
  };

  return (
    <ConfirmContext.Provider value={api.current}>
      {children}

      {pending && (
        <div className="backdrop" onMouseDown={() => close(null)}>
          <div
            className="confirm"
            role="alertdialog"
            aria-modal="true"
            aria-label={pending.title}
            onMouseDown={(event) => event.stopPropagation()}
          >
            <div className="confirm__body">
              {pending.tone === "danger" && (
                <span className="confirm__badge">
                  <Icon name="warning" size={19} />
                </span>
              )}
              <div className="confirm__copy">
                <h2 className="confirm__title">{pending.title}</h2>
                <p className="confirm__message">{pending.message}</p>
                {pending.detail && <p className="confirm__detail">{pending.detail}</p>}

                {pending.preview && (
                  <div className="confirm__preview">{pending.preview}</div>
                )}

                {pending.input && (
                  <label className="field">
                    <span className="field__label">{pending.input.label}</span>
                    <input
                      className="input"
                      value={value}
                      placeholder={pending.input.placeholder}
                      onChange={(event) => setValue(event.target.value)}
                      autoFocus
                      onKeyDown={(event) => {
                        if (event.key === "Enter" && value.trim()) close(value);
                        if (event.key === "Escape") close(null);
                      }}
                    />
                  </label>
                )}
              </div>
            </div>

            <footer className="confirm__foot">
              <button type="button" className="btn btn--outline" onClick={() => close(null)}>
                {pending.cancelLabel ?? t("common.cancel")}
              </button>
              <button
                type="button"
                className={`btn ${pending.tone === "danger" ? "btn--danger" : "btn--primary"}`}
                onClick={() => close(pending.input ? value : "")}
                disabled={pending.input ? value.trim().length === 0 : false}
              >
                {pending.confirmLabel}
              </button>
            </footer>
          </div>
        </div>
      )}
    </ConfirmContext.Provider>
  );
}
