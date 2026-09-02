import { invoke } from "@tauri-apps/api/core";
import type {
  BuildResult,
  DocumentSummary,
  Environment,
  PlumeDocument,
  Template,
  Transcript,
  TrashedCourse,
  LogEntry,
  Block,
  Settings,
} from "./types";

export const checkEnvironment = () => invoke<Environment>("check_environment");
export const listDocuments = () => invoke<DocumentSummary[]>("list_documents");
export const listTemplates = () => invoke<Template[]>("list_templates");

export const listTrash = () => invoke<TrashedCourse[]>("list_trash");
export const restoreDocument = (folder: string) =>
  invoke<PlumeDocument>("restore_document", { folder });
export const purgeDocument = (folder: string) =>
  invoke<void>("purge_document", { folder });

export const openCoursePdf = (id: string) => invoke<void>("open_course_pdf", { id });

export const createDocument = (title: string, templateId: string, sources: string[]) =>
  invoke<PlumeDocument>("create_document", { title, templateId, sources });

export const previewPreamble = (templateId: string) =>
  invoke<string>("preview_preamble", { templateId });

export const workspacePath = () => invoke<string>("workspace_path");
export const revealWorkspace = () => invoke<void>("reveal_workspace");
export const openUrl = (url: string) => invoke<void>("open_url", { url });

export const getDocument = (id: string) => invoke<PlumeDocument>("get_document", { id });

export const documentPages = (id: string) => invoke<string[]>("document_pages", { id });

export const loadTranscript = (id: string) =>
  invoke<Transcript | null>("load_transcript", { id });

export const transcribeDocument = (id: string, model: string) =>
  invoke<Transcript>("transcribe_document", { id, model });

export const buildDocument = (id: string, audience: string) =>
  invoke<BuildResult>("build_document", { id, audience });

export const revealPath = (path: string) => invoke<void>("reveal_path", { path });

export const logs = () => invoke<LogEntry[]>("logs");
export const clearLogs = () => invoke<void>("clear_logs");

export const saveBlock = (id: string, block: Block) =>
  invoke<void>("save_block", { id, block });

export const setBlockNote = (id: string, blockId: string, note: string | null) =>
  invoke<void>("set_block_note", { id, blockId, note });

export const applyCorrections = (id: string, model: string) =>
  invoke<Transcript>("apply_corrections", { id, model });

export const documentPagePaths = (id: string) =>
  invoke<string[]>("document_page_paths", { id });

export const setReadingRules = (id: string, rules: string) =>
  invoke<void>("set_reading_rules", { id, rules });

export const saveTemplate = (template: Template) =>
  invoke<void>("save_template", { template });

export const duplicateTemplate = (sourceId: string, name: string) =>
  invoke<Template>("duplicate_template", { sourceId, name });

export const deleteTemplate = (id: string) => invoke<void>("delete_template", { id });

export const readTemplatePreamble = (id: string) =>
  invoke<string>("read_template_preamble", { id });

export const writeTemplatePreamble = (id: string, text: string) =>
  invoke<void>("write_template_preamble", { id, text });

export const checkTemplate = (id: string) => invoke<void>("check_template", { id });

/** Removes one passage. Returns the whole transcript. */
export const deleteBlock = (id: string, blockId: string) =>
  invoke<Transcript>("delete_block", { id, blockId });

/** Replaces one passage with two. Returns the whole transcript. */
export const splitBlock = (id: string, blockId: string, head: string, tail: string) =>
  invoke<Transcript>("split_block", { id, blockId, head, tail });

export const reorderPages = (id: string, order: number[]) =>
  invoke<PlumeDocument>("reorder_pages", { id, order });

/** Ids of the courses being read right now. */
export const readingDocuments = () => invoke<string[]>("reading_documents");

export const renderFigure = (id: string, tikz: string) =>
  invoke<string>("render_figure", { id, tikz });

export const getSettings = () => invoke<Settings>("get_settings");
export const saveSettings = (settings: Settings) =>
  invoke<void>("save_settings", { settings });

export const deleteDocument = (id: string) => invoke<void>("delete_document", { id });

export const renameDocument = (id: string, title: string) =>
  invoke<PlumeDocument>("rename_document", { id, title });

export const addPages = (id: string, sources: string[]) =>
  invoke<PlumeDocument>("add_pages", { id, sources });

export const removePage = (id: string, number: number) =>
  invoke<PlumeDocument>("remove_page", { id, number });

export const cancelTranscription = (id: string) =>
  invoke<number>("cancel_transcription", { id });

export const cancelCorrections = (id: string) =>
  invoke<number>("cancel_corrections", { id });

export const installEngine = () => invoke<string>("install_engine");
export const installClaude = () => invoke<string>("install_claude");
export const openClaudeLogin = () => invoke<void>("open_claude_login");
export const removeEngine = () => invoke<void>("remove_engine");

export const updatesConfigured = () => invoke<boolean>("updates_configured");
