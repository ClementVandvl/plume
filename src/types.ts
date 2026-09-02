import { t } from "./i18n";

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
  /** Pages this machine reads at once on the automatic setting. */
  autoPages: number;
  /** Memory found, in gigabytes; null when it could not be measured. */
  memoryGb: number | null;
};

export type TemplateKey = {
  key: string;
  label: string;
  group: string;
  /** `color` | `text` | `length` | `choice:a|b|c` */
  type: string;
  value: string;
};

/** How one IR block kind is written in this template's LaTeX. */
export type BlockMapping = {
  /** `command` | `environment` | `raw` | `centered` */
  mode: string;
  /** Command or environment name; empty for `raw` and `centered`. */
  name: string;
};

export type Template = {
  id: string;
  version: number;
  name: string;
  description: string;
  engine: string;
  keys: TemplateKey[];
  blocks: Record<string, BlockMapping>;
  /**
   * Typesetting instructions belonging to this house style.
   *
   * Reading rules and standing conventions describe how the teacher writes,
   * whatever template they use; these describe what the template wants of the
   * LaTeX, and follow the template when a course changes style.
   */
  conventions: Convention[];
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
  /** Every dollar spent reading and correcting this course. */
  costUsd: number;
  /** File name of the last compiled PDF, when one exists. */
  lastPdf?: string | null;
};

/** A course as the list returns it: the document plus its review arithmetic. */
export type DocumentSummary = PlumeDocument & {
  /** Blocks in the transcript; 0 when the course has not been read yet. */
  blockCount: number;
  /** Blocks below the doubt threshold and not yet confirmed. */
  doubtfulCount: number;
};

/** A course sitting in the bin. */
export type TrashedCourse = {
  /** Folder name inside the bin — the restore/purge handle. */
  folder: string;
  title: string;
  pageCount: number;
  trashedAt: number;
};

export type Block = {
  id: string;
  kind: string;
  title: string | null;
  /**
   * For a heading: the number written on the page — "3", "II", "1", "a".
   *
   * Plume numbers nothing itself. A course photographed from the middle of a
   * notebook opens on "Chapitre 3", and renumbering it would contradict every
   * other document the class holds. Null when the page shows none.
   */
  number: string | null;
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
  draft: t("status.draft"),
  review: t("status.review"),
  ready: t("status.ready"),
};

/** Block kind -> what the user calls it. */
export const KIND_LABEL: Record<string, string> = {
  chapter: t("kind.chapter"),
  part: t("kind.part"),
  subpart: t("kind.subpart"),
  paragraph: t("kind.paragraph"),
  text: t("kind.text"),
  list: t("kind.list"),
  equation: t("kind.equation"),
  definition: t("kind.definition"),
  property: t("kind.property"),
  theorem: t("kind.theorem"),
  method: t("kind.method"),
  example: t("kind.example"),
  application: t("kind.application"),
  remark: t("kind.remark"),
  proof: t("kind.proof"),
  figure: t("kind.figure"),
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
  /** `start` opens a passage, `block` closes it. */
  phase: "start" | "block" | "done" | "failed" | "cancelled";
  blockId: string;
  done: number;
  total: number;
  message: string | null;
};

export const AUDIENCE_LABEL: Record<string, string> = {
  teacher: t("audience.teacher"),
  student: t("audience.student"),
};

export type Route =
  | { name: "home" }
  | { name: "courses" }
  | { name: "houseStyle" }
  | { name: "instructions" }
  | { name: "trash" }
  | { name: "course"; id: string; step?: StepId };

/** Steps of the recognition wizard, in order. Labels live in the dictionary. */
export const STEPS = [
  { id: "pages", labelKey: "steps.pages" },
  { id: "read", labelKey: "steps.read" },
  { id: "review", labelKey: "steps.review" },
  { id: "export", labelKey: "steps.export" },
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

export type UiTheme = "light" | "dark" | "system";

export type Settings = {
  /** Marker conventions: a trigger and an effect. */
  rules: ReadingRule[];
  /** Standing instructions, applied to every course. */
  conventions: Convention[];
  defaultModel: string;
  /** Look for a new version at start-up. Installing always needs a click. */
  checkUpdates: boolean;
  /** Pages read at once; 0 = automatic, from the machine's memory. */
  concurrentPages: number;
  /** `light` | `dark` | `system` — the interface theme. */
  theme: string;
  /** `simple` | `advanced` — what level of detail the window shows. */
  uiMode: string;
};

export const TRIGGER_LABEL: Record<Trigger["kind"], string> = {
  highlight: t("trigger.highlight"),
  marginBar: t("trigger.marginBar"),
  underline: t("trigger.underline"),
  circled: t("trigger.circled"),
  penColour: t("trigger.penColour"),
  custom: t("trigger.custom"),
};

export const EFFECT_LABEL: Record<Effect["kind"], string> = {
  bold: t("effect.bold"),
  italic: t("effect.italic"),
  underline: t("effect.underline"),
  teacherOnly: t("effect.teacherOnly"),
  studentOnly: t("effect.studentOnly"),
  blockKind: t("effect.blockKind"),
  skip: t("effect.skip"),
  custom: t("effect.custom"),
};

/** Emitted while photos are being imported and normalised. */
export type ImportProgress = { done: number; total: number };

/** Lifecycle of one page during a read. */
export type PageStateEvent = {
  documentId: string;
  page: number;
  state: "reading" | "done" | "failed" | "cancelled";
  blocks: number;
  message: string | null;
};

/** Proof of life while a page is being read; the label is from a fixed set. */
export type HeartbeatEvent = {
  documentId: string;
  page: number;
  label: string;
};

/** What the timeline knows about one page. */
export type ScanInfo = {
  state: PageStateEvent["state"] | "waiting";
  blocks?: number;
  message?: string;
  label?: string;
};
