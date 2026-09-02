import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { installGlobalErrorReporting, logError } from "./log";
import {
  checkEnvironment,
  getSettings,
  listDocuments,
  listTemplates,
  listTrash,
  saveSettings,
  workspacePath,
} from "./api";
import { t } from "./i18n";
import { applyTheme, asTheme } from "./theme";
import { Console } from "./components/Console";
import { CourseView } from "./components/CourseView";
import { CoursesView } from "./components/CoursesView";
import { useActiveReadings } from "./ui/reading";
import { CreateWizard } from "./components/CreateWizard";
import { HomeView } from "./components/HomeView";
import { InstructionsView } from "./components/InstructionsView";
import { SettingsModal } from "./components/SettingsModal";
import { Sidebar } from "./components/Sidebar";
import { HouseStyleView } from "./components/HouseStyleView";
import { TrashView } from "./components/TrashView";
import { Icon } from "./ui/Icon";
import { Titlebar, type UiMode } from "./ui/Titlebar";
import { UiModeContext, asUiMode } from "./ui/mode";
import type {
  DocumentSummary,
  Environment,
  Route,
  Settings,
  Template,
  TrashedCourse,
} from "./types";
import "./App.css";

export default function App() {
  const [environment, setEnvironment] = useState<Environment | null>(null);
  const [documents, setDocuments] = useState<DocumentSummary[]>([]);
  const [templates, setTemplates] = useState<Template[]>([]);
  const [trash, setTrash] = useState<TrashedCourse[]>([]);
  const [workspace, setWorkspace] = useState("");
  const [route, setRoute] = useState<Route>({ name: "home" });
  const [modal, setModal] = useState<null | "create" | "settings">(null);
  /** Photos dropped on the home screen, waiting for the wizard. */
  const [seedPages, setSeedPages] = useState<string[]>([]);
  const [consoleOpen, setConsoleOpen] = useState(false);
  /**
   * False until the workbook has been read once.
   *
   * Without it the empty course list is indistinguishable from a workbook with
   * no courses, and the welcome screen flashed for the second or two the disk
   * took to answer — telling a teacher with forty courses that they had none.
   */
  const [loaded, setLoaded] = useState(false);
  const [settings, setSettings] = useState<Settings>({
    rules: [],
    conventions: [],
    defaultModel: "sonnet",
    checkUpdates: true,
    concurrentPages: 0,
    theme: "system",
    uiMode: "simple",
  });

  const refresh = useCallback(async () => {
    const [env, docs, models, path, stored, binned] = await Promise.all([
      checkEnvironment(),
      listDocuments(),
      listTemplates(),
      workspacePath(),
      getSettings(),
      listTrash(),
    ]);
    setSettings(stored);
    setEnvironment(env);
    setDocuments(docs);
    setTemplates(models);
    setWorkspace(path);
    setTrash(binned);
  }, []);

  useEffect(() => {
    installGlobalErrorReporting();
    refresh()
      .catch((cause) => logError("interface", t("error.load"), cause))
      // Loaded either way: a failure has its own message, and holding the
      // screen on "preparing" forever would say nothing at all.
      .finally(() => setLoaded(true));
  }, [refresh]);

  // The theme follows the settings; "system" hands control back to the OS.
  useEffect(() => {
    applyTheme(asTheme(settings.theme));
  }, [settings.theme]);

  // Toggled from the native menu (Outils > Console, Cmd+Alt+C) on macOS, and
  // from the title bar's own menu where there is no native one.
  useEffect(() => {
    const stop = listen("toggle-console", () => setConsoleOpen((open) => !open));
    return () => {
      stop.then((off) => off()).catch(() => {});
    };
  }, []);

  const mode = asUiMode(settings.uiMode);

  // The mode is a window-level preference: applied on the spot, saved quietly.
  function switchMode(next: UiMode) {
    const updated = { ...settings, uiMode: next };
    setSettings(updated);
    saveSettings(updated).catch((cause) =>
      logError("interface", t("error.refresh"), cause),
    );
  }

  // A reading outlives the screen that started it; every list showing courses
  // needs to say which ones are working.
  const reading = useActiveReadings();

  const onRefreshError = (cause: unknown) =>
    logError("interface", t("error.refresh"), cause);

  function openWizard(pages: string[] = []) {
    setSeedPages(pages);
    setModal("create");
  }

  const currentCourse =
    route.name === "course" ? documents.find((d) => d.id === route.id) : undefined;

  const titlebarContext =
    route.name === "course"
      ? currentCourse?.title
      : route.name === "courses"
        ? t("nav.courses")
        : route.name === "houseStyle"
          ? t("nav.houseStyle")
          : route.name === "instructions"
            ? t("nav.instructions")
            : route.name === "trash"
              ? t("nav.trash")
              : undefined;

  // A course is a deep view: it owns the window below the title bar. Navigation
  // chrome would only compete with the document being reviewed.
  if (route.name === "course") {
    return (
      <UiModeContext.Provider value={mode}>
        <div className="frame">
          <Titlebar
            context={titlebarContext}
            mode={mode}
            onMode={switchMode}
            onConsole={() => setConsoleOpen((open) => !open)}
          />
          <div className={`deep ${consoleOpen ? "deep--console" : ""}`}>
            <CourseView
              key={route.id}
              documentId={route.id}
              initialStep={route.step}
              defaultModel={settings.defaultModel}
              concurrentPages={settings.concurrentPages}
              autoPages={environment?.autoPages ?? 1}
              templates={templates}
              onBack={() => setRoute({ name: "courses" })}
              onChanged={() => refresh().catch(onRefreshError)}
              onDeleted={() => {
                setRoute({ name: "courses" });
                refresh().catch(onRefreshError);
              }}
            />
            <Console open={consoleOpen} onClose={() => setConsoleOpen(false)} />
          </div>
        </div>
      </UiModeContext.Provider>
    );
  }

  // The title bar stays: with the native decorations off, its buttons are the
  // only way to move or close the window while the disk is being read.
  if (!loaded) {
    return (
      <UiModeContext.Provider value={mode}>
        <div className="frame">
          <Titlebar mode={mode} onMode={switchMode} bare />
          <div className="booting">
            <Icon name="feather" size={30} />
            <p className="booting__title">{t("boot.title")}</p>
            <p className="booting__hint">{t("boot.hint")}</p>
          </div>
        </div>
      </UiModeContext.Provider>
    );
  }

  // Before the first course there is nothing to navigate: the home view shows
  // the welcome screen alone, full width, and the sidebar waits its turn.
  const firstLaunch = documents.length === 0 && route.name === "home";

  return (
    <UiModeContext.Provider value={mode}>
      <div className="frame">
        <Titlebar
            context={titlebarContext}
            mode={mode}
            onMode={switchMode}
            onConsole={() => setConsoleOpen((open) => !open)}
          />

        <div className={`shell ${consoleOpen ? "shell--console" : ""} ${firstLaunch ? "shell--bare" : ""}`}>
          {!firstLaunch && (
            <Sidebar
              route={route}
              onNavigate={setRoute}
              onSettings={() => setModal("settings")}
              courseCount={documents.length}
              trashCount={trash.length}
              environmentReady={environment?.ready ?? true}
            />
          )}

          <main className="content">
            {route.name === "home" && (
              <HomeView
                documents={documents}
                reading={reading}
                environment={environment}
                onCreate={openWizard}
                onNavigate={setRoute}
                onSettings={() => setModal("settings")}
              />
            )}

            {route.name === "courses" && (
              <CoursesView
                documents={documents}
                reading={reading}
                onCreate={() => openWizard()}
                onNavigate={setRoute}
                onChanged={() => refresh().catch(onRefreshError)}
              />
            )}

            {route.name === "instructions" && (
              <InstructionsView settings={settings} onSaved={setSettings} />
            )}

            {route.name === "houseStyle" && (
              <HouseStyleView
                templates={templates}
                documents={documents}
                onSaved={() => refresh().catch(onRefreshError)}
              />
            )}

            {route.name === "trash" && (
              <TrashView
                trash={trash}
                onChanged={() => refresh().catch(onRefreshError)}
              />
            )}
          </main>

          <Console open={consoleOpen} onClose={() => setConsoleOpen(false)} />
        </div>

        {modal === "create" && (
          <CreateWizard
            templates={templates}
            initialPages={seedPages}
            onCancel={() => setModal(null)}
            onCreated={(created) => {
              setModal(null);
              setRoute({ name: "course", id: created.id });
              refresh().catch(onRefreshError);
            }}
          />
        )}

        {modal === "settings" && (
          <SettingsModal
            environment={environment}
            workspace={workspace}
            settings={settings}
            onSaved={setSettings}
            onEnvironmentChanged={() => refresh().catch(onRefreshError)}
            onClose={() => setModal(null)}
          />
        )}
      </div>
    </UiModeContext.Provider>
  );
}
