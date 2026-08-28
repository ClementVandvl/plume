/**
 * Localisation.
 *
 * Every string the interface shows lives in a dictionary, one per language —
 * French today, English when the day comes. Components never carry prose:
 * they ask for a key, so translating the app means adding a file, not
 * re-reading every component.
 *
 * The format is deliberately small: `{name}` interpolation and a `.one` /
 * `.other` pair for plurals. The day a language needs real plural categories,
 * this module is the only file that changes.
 */

import { fr } from "./fr";

const LOCALES = { fr } as const;

export type Locale = keyof typeof LOCALES;

/** Every known message key — a typo in a component is a type error. */
export type MessageKey = keyof typeof fr;

/** Keys declared as a `.one` / `.other` pair, addressed by their base name. */
export type PluralKey = {
  [K in MessageKey]: K extends `${infer Base}.one` ? Base : never;
}[MessageKey];

/** BCP 47 tag per locale, for `Intl` formatters. */
const TAGS: Record<Locale, string> = { fr: "fr-FR" };

let current: Locale = "fr";

export function setLocale(locale: Locale) {
  current = locale;
}

export function locale(): Locale {
  return current;
}

function interpolate(raw: string, values?: Record<string, string | number>) {
  return raw.replace(/\{(\w+)\}/g, (whole, name: string) =>
    values && name in values ? String(values[name]) : whole,
  );
}

/** The message for `key`, with `{name}` placeholders filled in. */
export function t(key: MessageKey, values?: Record<string, string | number>): string {
  return interpolate(LOCALES[current][key] ?? key, values);
}

/**
 * The `.one` or `.other` variant of `key`, chosen from `count`.
 * `{count}` is available to the message without repeating it at the call site.
 */
export function tn(
  key: PluralKey,
  count: number,
  values?: Record<string, string | number>,
): string {
  const variant: MessageKey = `${key}.${count > 1 ? "other" : "one"}` as MessageKey;
  return interpolate(LOCALES[current][variant] ?? variant, { count, ...values });
}

/** "12 mars", for lists where the year is noise. */
export function formatDay(at: number): string {
  return new Intl.DateTimeFormat(TAGS[current], { day: "numeric", month: "long" }).format(
    new Date(at),
  );
}

/** "12 mars 2026", for anything that may be old. */
export function formatDate(at: number): string {
  return new Intl.DateTimeFormat(TAGS[current], {
    day: "numeric",
    month: "long",
    year: "numeric",
  }).format(new Date(at));
}

/** "14:02:11", for the technical journal. */
export function formatTime(at: number): string {
  return new Intl.DateTimeFormat(TAGS[current], {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  }).format(new Date(at));
}

/** "0,42 $" — costs come from the API in dollars. */
export function formatMoney(usd: number): string {
  return `${new Intl.NumberFormat(TAGS[current], {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  }).format(usd)} $`;
}

/**
 * "il y a 10 minutes", "hier", or the date — how the course list dates a
 * modification.
 */
export function formatRelative(at: number): string {
  const elapsed = Date.now() - at;
  const minute = 60_000;
  const hour = 60 * minute;
  const day = 24 * hour;
  const relative = new Intl.RelativeTimeFormat(TAGS[current], { numeric: "auto" });

  if (elapsed < minute) return relative.format(0, "minute");
  if (elapsed < hour) return relative.format(-Math.round(elapsed / minute), "minute");
  if (elapsed < day) return relative.format(-Math.round(elapsed / hour), "hour");
  if (elapsed < 7 * day) return relative.format(-Math.round(elapsed / day), "day");
  return formatDate(at);
}
