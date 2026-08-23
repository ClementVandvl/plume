export type ToolStatus = {
  key: string;
  label: string;
  role: string;
  found: boolean;
  version: string | null;
  path: string | null;
  hint: string | null;
  installUrl: string;
  /** Plume can install this one itself. */
  installable: boolean;
  required: boolean;
};

export type Environment = {
  tools: ToolStatus[];
  ready: boolean;
};

export type TemplateKey = {
  key: string;
  label: string;
  group: string;
  /** `color` | `text` | `length` | `choice:a|b|c` */
  type: string;
  value: string;
};

export type Template = {
  id: string;
  name: string;
  description: string;
  engine: string;
  keys: TemplateKey[];
};

export type DocumentStatus = "draft" | "review" | "ready";

export type PlumeDocument = {
  id: string;
  title: string;
  templateId: string;
  readingRules: string;
  createdAt: number;
  updatedAt: number;
  pageCount: number;
  status: DocumentStatus;
};

export type Block = {
  id: string;
  kind: string;
  title: string | null;
  latex: string;
  confidence: number;
  doubt: string | null;
  audience: string[];
  /** The teacher's pending instruction, consumed by a targeted re-run. */
  note: string | null;
  reviewed: boolean;
};

export type Page = { number: number; blocks: Block[]; sessionId: string | null };

export type Transcript = { version: number; pages: Page[] };

export type BuildResult = {
  texPath: string;
  pdfPath: string | null;
  error: string | null;
};

/** Emitted by the Rust side while a document is being read. */
export type TranscriptionProgress = {
  documentId: string;
  phase: "page" | "done" | "failed" | "cancelled";
  page: number;
  total: number;
  blocks: number;
  costUsd: number;
  message: string | null;
};

/** Below this, a block is surfaced for review. Mirrors ir::DOUBT_THRESHOLD. */
export const DOUBT_THRESHOLD = 0.85;

/** Technical status -> label shown to the user. */
export const STATUS_LABEL: Record<string, string> = {
  draft: "Brouillon",
  review: "À relire",
  ready: "Prêt",
};

/** Block kind -> what the user calls it. */
export const KIND_LABEL: Record<string, string> = {
  chapter: "Chapitre",
  part: "Partie",
  subpart: "Sous-partie",
  paragraph: "Paragraphe",
  text: "Texte",
  list: "Liste",
  equation: "Équation",
  definition: "Définition",
  property: "Propriété",
  theorem: "Théorème",
  method: "Méthode",
  example: "Exemple",
  application: "Application",
  remark: "Remarque",
  proof: "Démonstration",
  figure: "Schéma",
};

export type LogEntry = {
  at: number;
  level: "debug" | "info" | "warn" | "error";
  scope: string;
  message: string;
  detail: string | null;
};

/** Emitted while annotated blocks are being re-run. */
export type CorrectionProgress = {
  documentId: string;
  phase: "block" | "done" | "failed" | "cancelled";
  blockId: string;
  done: number;
  total: number;
  message: string | null;
};

export const AUDIENCE_LABEL: Record<string, string> = {
  teacher: "Professeur",
  student: "Élève",
};

export type Route =
  | { name: "home" }
  | { name: "courses" }
  | { name: "templates" }
  | { name: "rules" }
  | { name: "course"; id: string };

/** Steps of the recognition wizard, in order. */
export const STEPS = [
  { id: "pages", label: "Pages" },
  { id: "read", label: "Lecture" },
  { id: "review", label: "Relecture" },
  { id: "export", label: "Export" },
] as const;

export type StepId = (typeof STEPS)[number]["id"];

export type Trigger = {
  kind: "highlight" | "marginBar" | "underline" | "circled" | "penColour" | "custom";
  colour: string;
  label: string;
};

export type Effect = {
  kind:
    | "bold"
    | "italic"
    | "underline"
    | "teacherOnly"
    | "studentOnly"
    | "blockKind"
    | "skip"
    | "custom";
  value: string;
};

export type ReadingRule = {
  id: string;
  enabled: boolean;
  trigger: Trigger;
  effect: Effect;
};

/** A standing instruction with no visual trigger. */
export type Convention = {
  id: string;
  enabled: boolean;
  title: string;
  text: string;
};

export type Settings = {
  /** Marker conventions: a trigger and an effect. */
  rules: ReadingRule[];
  /** Standing instructions, applied to every course. */
  conventions: Convention[];
  defaultModel: string;
};

export const TRIGGER_LABEL: Record<Trigger["kind"], string> = {
  highlight: "Surligné",
  marginBar: "Trait en marge",
  underline: "Souligné à la main",
  circled: "Entouré",
  penColour: "Écrit à l'encre",
  custom: "Autre marque",
};

export const EFFECT_LABEL: Record<Effect["kind"], string> = {
  bold: "Mettre en gras",
  italic: "Mettre en italique",
  underline: "Souligner",
  teacherOnly: "Réserver à la version professeur",
  studentOnly: "Réserver à la version élève",
  blockKind: "Forcer le type de bloc",
  skip: "Ne pas transcrire",
  custom: "Effet décrit à la main",
};

/** Emitted while photos are being imported and normalised. */
export type ImportProgress = { done: number; total: number };
