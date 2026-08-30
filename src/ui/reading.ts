import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { readingDocuments } from "../api";

/**
 * The courses being read right now.
 *
 * A reading lives in the backend, not in the screen that started it: leaving
 * the course view does not stop it. The interface had no way to know that,
 * so a course reopened mid-reading showed its "read the pages" button as if
 * nothing were happening, and the dashboard showed nothing at all.
 *
 * The backend registry is the single source of truth. Events only say "look
 * again" — the set is always re-read rather than patched, so a run that ends
 * by failing (no completion event) cannot leave a card spinning forever.
 */
export function useActiveReadings(): Set<string> {
  const [active, setActive] = useState<Set<string>>(new Set());

  const look = useCallback(() => {
    readingDocuments()
      .then((ids) => setActive(new Set(ids)))
      .catch(() => {});
  }, []);

  useEffect(() => {
    look();

    const subscriptions = ["transcription", "page-state"].map((name) =>
      listen(name, () => look()),
    );

    // A run that dies without a final event would otherwise stay on screen.
    const beat = window.setInterval(look, 10_000);

    return () => {
      window.clearInterval(beat);
      for (const subscription of subscriptions) {
        subscription.then((off) => off()).catch(() => {});
      }
    };
  }, [look]);

  return active;
}
