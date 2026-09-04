/**
 * A fake backend for layout work.
 *
 * `npm run dev` runs the interface without Tauri: no commands, so the app
 * never gets past its welcome screen. Opening `http://localhost:1420/?mock`
 * installs an in-memory stand-in for every command the interface calls, with
 * enough data to reach each screen — home, list, review, settings, trash.
 *
 * Nothing here ships: the module is imported only under `import.meta.env.DEV`,
 * and only when the `mock` query parameter is present.
 */

const now = Date.now();
const day = 86_400_000;

const block = (
  id: string,
  kind: string,
  latex: string,
  extra: Partial<Record<string, unknown>> = {},
) => ({
  id,
  kind,
  title: null,
  number: null,
  latex,
  confidence: 0.97,
  doubt: null,
  audience: [],
  align: null,
  note: null,
  taughtEnd: false,
  reviewed: false,
  ...extra,
});

const transcript = {
  version: 1,
  pages: [
    {
      number: 1,
      sessionId: null,
      blocks: [
        block("b1", "chapter", "", {
          title: "Géométrie dans le plan",
          number: "3",
        }),
        block("b2", "part", "", {
          title: "Vecteurs du plan",
          number: "II",
        }),
        // A heading the page did not number: it used to render as "null – …".
        block("b2b", "subpart", "", { title: "Exercices" }),
        block(
          "b3",
          "text",
          "Dans tout ce paragraphe, le plan est muni d'un repère orthonormé direct. On note $A$, $B$ deux points distincts.",
          // An annotated passage, so the correction panel has something to run on.
          { note: "Reprends la fin : il manque « non confondus »." },
        ),
        // A worked example whose statement and answer share one passage — the
        // case the split exists for.
        block(
          "b3b",
          "example",
          "Soient $\\vec{u}$ et $\\vec{v}$ deux vecteurs.\n\nAlors :\n\n$\\vec{u} + \\vec{v} = \\vec{w}$",
          { title: "Somme de deux vecteurs" },
        ),
        block(
          "b4",
          "definition",
          "On appelle vecteur directeur de la droite $(AB)$ tout vecteur non nul colinéaire à $\\vec{AB}$.",
          { title: "vecteur directeur" },
        ),
        block(
          "b5",
          "property",
          "Deux droites sont parallèles si et seulement si leurs vecteurs directeurs sont colinéaires.",
          // Where the class stopped: page 2 is next week's lesson.
          { taughtEnd: true },
        ),
      ],
    },
    {
      number: 2,
      sessionId: null,
      blocks: [
        block(
          "b6",
          "theorem",
          "Pour tous points $M$ et $N$ du plan, il existe un unique réel $k$ tel que $\\vec{MN} = k\\,\\vec{AB}$, dès que $\\vec{AB} \\neq \\vec{0}$.",
          {
            confidence: 0.71,
            doubt:
              "Le sens de la flèche au-dessus de MN n'est pas net sur la photo. Vérifiez que ce n'est pas NM.",
          },
        ),
        block(
          "b7",
          "remark",
          "Insister sur le cas $k$ négatif : les élèves confondent souvent avec la relation de Chasles.",
          { audience: ["teacher"] },
        ),
      ],
    },
  ],
};

const documents = [
  {
    id: "geometrie",
    title: "Géométrie dans le plan",
    templateId: "charte-maths",
    readingRules: "",
    createdAt: now - 12 * day,
    updatedAt: now - 600_000,
    pageCount: 6,
    status: "review",
    costUsd: 0.42,
    lastPdf: null,
    blockCount: 41,
    doubtfulCount: 3,
    // A course the class is halfway through, so the card shows how far.
    taughtCount: 7,
    taughtHeading: "Vecteurs du plan",
  },
  {
    id: "suites",
    title: "Suites numériques",
    templateId: "charte-maths",
    readingRules: "",
    createdAt: now - 20 * day,
    updatedAt: now - day,
    pageCount: 8,
    status: "review",
    costUsd: 0.61,
    lastPdf: null,
    blockCount: 52,
    doubtfulCount: 5,
    // Marked, but no heading above the boundary: the card falls back to a count.
    taughtCount: 4,
    taughtHeading: null,
  },
  {
    id: "trigo",
    title: "Trigonométrie",
    templateId: "charte-maths",
    readingRules: "",
    createdAt: now - 30 * day,
    updatedAt: now - 16 * day,
    pageCount: 4,
    status: "ready",
    costUsd: 0.28,
    lastPdf: "trigo-teacher.pdf",
    blockCount: 24,
    doubtfulCount: 0,
  },
  {
    id: "probas",
    title: "Probabilités",
    templateId: "charte-maths",
    readingRules: "",
    createdAt: now - 19 * day,
    updatedAt: now - 19 * day,
    pageCount: 3,
    status: "draft",
    costUsd: 0,
    lastPdf: null,
    blockCount: 0,
    doubtfulCount: 0,
  },
];

const template = {
  id: "charte-maths",
  version: 4,
  name: "Ma charte de maths",
  description: "La charte livrée avec Plume",
  engine: "lualatex",
  keys: [
    // Colour and size of the same element, so the paired rows are visible.
    { key: "color.chapter", label: "Titre de chapitre", group: "Structure", type: "color", value: "#A93226" },
    { key: "size.chapter", label: "Titre de chapitre", group: "Structure", type: "size", value: "16" },
    { key: "color.part", label: "Grande partie (I)", group: "Structure", type: "color", value: "#1F618D" },
    { key: "size.part", label: "Grande partie (I)", group: "Structure", type: "size", value: "13" },
    { key: "color.subpart", label: "Sous-partie (1)", group: "Structure", type: "color", value: "#117A65" },
    { key: "size.subpart", label: "Sous-partie (1)", group: "Structure", type: "size", value: "11" },
    { key: "color.definition", label: "Définition", group: "Environnements", type: "color", value: "#A93226" },
    { key: "label.definition", label: "Définition", group: "Environnements", type: "text", value: "Définition" },
    { key: "color.property", label: "Propriété", group: "Environnements", type: "color", value: "#117A65" },
    { key: "label.property", label: "Propriété", group: "Environnements", type: "text", value: "Propriété" },
    { key: "page.fontSize", label: "Taille du texte", group: "Mise en page", type: "choice:10pt|11pt|12pt", value: "11pt" },
    { key: "page.margin", label: "Marges", group: "Mise en page", type: "choice:serrées|normales|larges", value: "normales" },
  ],
  blocks: {
    definition: { mode: "environment", name: "definition" },
    property: { mode: "environment", name: "propriete" },
    theorem: { mode: "environment", name: "theoreme" },
    text: { mode: "raw", name: "" },
  },
  conventions: [
    {
      id: "c1",
      enabled: true,
      title: "Alignements",
      text: "Aligner sur le signe égal les lignes de calcul qui commencent par « = ».",
    },
  ],
};

const settings = {
  rules: [
    {
      id: "r1",
      enabled: true,
      trigger: { kind: "highlight", colour: "#F2A93B", label: "orange" },
      effect: { kind: "bold", value: "" },
    },
    {
      id: "r2",
      enabled: true,
      trigger: { kind: "marginBar", colour: "#1F618D", label: "bleu" },
      effect: { kind: "teacherOnly", value: "" },
    },
    {
      id: "r3",
      enabled: false,
      trigger: { kind: "circled", colour: "", label: "" },
      effect: { kind: "underline", value: "" },
    },
  ],
  conventions: [
    {
      id: "s1",
      enabled: true,
      title: "Démonstrations",
      text: "Ne jamais couper une démonstration entre deux pages.",
    },
    {
      id: "s2",
      enabled: true,
      title: "Vecteurs",
      text: "Écrire les vecteurs avec une flèche, jamais en gras.",
    },
  ],
  defaultModel: "sonnet",
  checkUpdates: true,
  concurrentPages: 0,
  theme: "system",
  uiMode: "simple",
};

const environment = {
  ready: false,
  // A modest machine, so the automatic value is visibly not three.
  autoPages: 2,
  memoryGb: 12,
  tools: [
    {
      key: "claude",
      label: "Le lecteur de pages",
      role: "Installé et connecté à votre abonnement",
      found: true,
      version: "2.1.0",
      path: "/usr/local/bin/claude",
      hint: null,
      installUrl: "https://claude.com/claude-code",
      installable: true,
      required: true,
    },
    {
      key: "engine",
      label: "Le moteur de mise en page",
      role: "Fabrique les PDF",
      found: false,
      version: null,
      path: null,
      hint: "Manquant — sans lui, pas de PDF. Environ 250 Mo.",
      installUrl: "https://tug.org",
      installable: true,
      required: true,
    },
  ],
};

const trash = [
  {
    folder: "derivation",
    title: "Dérivation — ancienne version",
    pageCount: 5,
    trashedAt: now - 3 * day,
  },
  { folder: "essai", title: "Essai photos floues", pageCount: 2, trashedAt: now - 22 * day },
];

const logs = [
  { at: now - 200_000, level: "info", scope: "claude", message: "page 1 — 9 blocs, 0,07 $", detail: null },
  { at: now - 150_000, level: "warn", scope: "ir", message: "bloc 12 — confiance 0,71 < 0,85, signalé pour relecture", detail: null },
  { at: now - 90_000, level: "error", scope: "claude", message: "page 6 — échec : image illisible (flou), page conservée", detail: null },
  { at: now - 20_000, level: "info", scope: "latex", message: "compilation en 2 passes — 9 pages, 0 warning", detail: null },
];

const handlers: Record<string, (args: Record<string, unknown>) => unknown> = {
  check_environment: () => environment,
  list_documents: () => documents,
  // One course pretends to be reading, so the activity indicator is visible
  // while working on the layout.
  reading_documents: () => [documents[1]?.id].filter(Boolean),
  reorder_pages: () => documents[0],
  // Slow on purpose: the correction panel is only worth looking at while it
  // is running.
  apply_corrections: async () => {
    await new Promise((resolve) => setTimeout(resolve, 8000));
    return transcript;
  },
  list_templates: () => [template],
  list_trash: () => trash,
  restore_document: () => documents[0],
  purge_document: () => undefined,
  get_settings: () => settings,
  save_settings: (args) => Object.assign(settings, args.settings as object),
  workspace_path: () => "/Users/vous/Documents/Plume",
  get_document: (args) => documents.find((d) => d.id === args.id) ?? documents[0],
  // Real-looking page paths, so the full-screen viewer can be worked on.
  document_page_paths: () => [
    '/mock/IMG_4021.jpg',
    '/mock/IMG_4022.jpg',
    '/mock/IMG_4023.jpg',
  ],
  load_transcript: () => transcript,
  logs: () => logs,
  clear_logs: () => undefined,
  log_client: () => undefined,
  save_block: () => undefined,
  set_block_note: () => undefined,
  set_taught_end: (args: Record<string, unknown>) => {
    for (const page of transcript.pages) {
      for (const b of page.blocks) b.taughtEnd = b.id === args.blockId;
    }
    // A fresh object, as the real command's JSON round trip always produces.
    // Handing back the same reference let React keep memoised work that reads
    // the mutated blocks, and the mock showed a state the app never has.
    return structuredClone(transcript);
  },
  split_block: () => transcript,
  delete_block: () => transcript,
  insert_block: () => transcript,
  insert_from_photo: () => transcript,
  set_reading_rules: () => undefined,
  updates_configured: () => false,
  os_platform: () => "macos",
  open_course_pdf: () => undefined,
  reveal_workspace: () => undefined,
  reveal_path: () => undefined,
  open_url: () => undefined,
  read_template_preamble: () => "\\documentclass{article}\n% {{color.definition}}\n",
};

export function installDevMock() {
  const internals = {
    invoke: (command: string, args: Record<string, unknown> = {}) => {
      // The file picker hands back fake photos, so the wizard is walkable.
      if (command === "plugin:dialog|open")
        return Promise.resolve([
          "/mock/IMG_4021.jpg",
          "/mock/IMG_4022.jpg",
          "/mock/IMG_4023.jpg",
          "/mock/IMG_4024.jpg",
        ]);
      // Event plumbing (`plugin:event|listen` and friends) just needs an id.
      if (command.startsWith("plugin:")) return Promise.resolve(0);
      const handler = handlers[command];
      if (!handler) return Promise.reject(`mock: commande « ${command} » non simulée`);
      return Promise.resolve(handler(args));
    },
    convertFileSrc: (path: string) => path,
    transformCallback: () => 0,
  };
  (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = internals;
}
