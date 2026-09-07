import { useEffect, useRef, useState } from "react";
import { claudeAuthStatus, openClaudeLogin } from "../api";
import { logError } from "../log";

/**
 * Signing Claude Code back in, from anywhere in the app.
 *
 * The sign-in is the CLI's own browser round trip, run in a terminal Plume
 * opens with the command already typed. Plume cannot see it finish, so it asks
 * the CLI every few seconds whether it is signed in now, and stops asking when
 * it is — or after a couple of minutes, which is longer than any sign-in takes
 * and short enough not to poll forever after a closed window.
 */
export function useClaudeLogin(onSignedIn: () => void) {
  const [pending, setPending] = useState(false);
  const timer = useRef<number | null>(null);
  const attempts = useRef(0);

  useEffect(() => () => {
    if (timer.current !== null) window.clearInterval(timer.current);
  }, []);

  function stop() {
    if (timer.current !== null) window.clearInterval(timer.current);
    timer.current = null;
    setPending(false);
  }

  async function start() {
    try {
      await openClaudeLogin();
    } catch (cause) {
      logError("claude", "Connexion impossible à lancer", cause);
      return;
    }
    setPending(true);
    attempts.current = 0;
    timer.current = window.setInterval(async () => {
      attempts.current += 1;
      try {
        const status = await claudeAuthStatus();
        if (status.loggedIn) {
          stop();
          onSignedIn();
          return;
        }
      } catch {
        // The CLI may be busy signing in; asking again is the whole plan.
      }
      if (attempts.current >= 40) stop();
    }, 3000);
  }

  return { start, pending, stop };
}
