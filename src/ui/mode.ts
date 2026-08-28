import { createContext, useContext } from "react";
import type { UiMode } from "./Titlebar";

/**
 * Simple / Avancé, as a context: the switch lives in the title bar, but what
 * it hides — models, costs, LaTeX, parallelism — is scattered across every
 * screen. Threading a prop through each of them would make the mode look like
 * page state, which it is not.
 */
export const UiModeContext = createContext<UiMode>("simple");

/** True when the window shows its technical underside. */
export function useAdvanced(): boolean {
  return useContext(UiModeContext) === "advanced";
}

export function asUiMode(raw: string | undefined): UiMode {
  return raw === "advanced" ? "advanced" : "simple";
}
