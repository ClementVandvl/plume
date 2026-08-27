import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { installGlobalErrorReporting, logError } from "./log";
import {
  checkEnvironment,
  getSettings,
  listDocuments,
  listTemplates,
  workspacePath,
} from "./api";
import { Console } from "./components/Console";
import { CourseView } from "./components/CourseView";
import { CoursesView } from "./components/CoursesView";
import { CreateWizard } from "./components/CreateWizard";
import { HomeView } from "./components/HomeView";
import { RulesView } from "./components/RulesView";
import { SettingsModal } from "./components/SettingsModal";
import { Sidebar } from "./components/Sidebar";
import { TemplatesView } from "./components/TemplatesView";
import type { Environment, PlumeDocument, Route, Settings, Template } from "./types";
import "./App.css";

export default function App() {
  const [environment, setEnvironment] = useState<Environment | null>(null);
  const [documents, setDocuments] = useState<PlumeDocument[]>([]);
  const [templates, setTemplates] = useState<Template[]>([]);
  const [workspace, setWorkspace] = useState("");
  const [route, setRoute] = useState<Route>({ name: "home" });
  const [modal, setModal] = useState<null | "create" | "settings">(null);
  const [consoleOpen, setConsoleOpen] = useState(false);
  const [settings, setSettings] = useState<Settings>({
    rules: [],
    conventions: [],
    defaultModel: "sonnet",
    checkUpdates: true,
    concurrentPages: 0,
  });

  const refresh = useCallback(async () => {
    const [env, docs, models, path, stored] = await Promise.all([
      checkEnvironment(),
      listDocuments(),
      listTemplates(),
      workspacePath(),
      getSettings(),
    ]);
    setSettings(stored);
    setEnvironment(env);
    setDocuments(docs);
    setTemplates(models);
    setWorkspace(path);
  }, []);

  useEffect(() => {
    installGlobalErrorReporting();
    refresh().catch((cause) => logError("interface", "Chargement du classeur impossible", cause));
  }, [refresh]);

  // Toggled from the native menu (Outils > Console, Cmd+Alt+C).
  useEffect(() => {
    const stop = listen("toggle-console", () => setConsoleOpen((open) => !open));
    return () => {
      stop.then((off) => off()).catch(() => {});
    };
  }, []);

  // A course is a deep view: it owns the window, sidebar included. Navigation
  // chrome would only compete with the document being reviewed.
  if (route.name === "course") {
    return (
      <div className={`deep ${consoleOpen ? "deep--console" : ""}`}>
        <CourseView
          key={route.id}
          documentId={route.id}
          defaultModel={settings.defaultModel}
          templates={templates}
          onBack={() => setRoute({ name: "courses" })}
          onChanged={() =>
            refresh().catch((cause) =>
              logError("interface", "Rafraîchissement impossible", cause),
            )
          }
          onDeleted={() => {
            setRoute({ name: "courses" });
            refresh().catch((cause) =>
              logError("interface", "Rafraîchissement impossible", cause),
            );
          }}
        />
        <Console open={consoleOpen} onClose={() => setConsoleOpen(false)} />
      </div>
    );
  }

  return (
    <div className={`shell ${consoleOpen ? "shell--console" : ""}`}>
      <Sidebar
        route={route}
        onNavigate={setRoute}
        onSettings={() => setModal("settings")}
        onConsole={() => setConsoleOpen((open) => !open)}
        courseCount={documents.length}
        environmentReady={environment?.ready ?? true}
      />

      <main className="content">
        {route.name === "home" && (
          <HomeView
            documents={documents}
            environment={environment}
            onCreate={() => setModal("create")}
            onNavigate={setRoute}
            onSettings={() => setModal("settings")}
          />
        )}

        {route.name === "courses" && (
          <CoursesView
            documents={documents}
            onCreate={() => setModal("create")}
            onNavigate={setRoute}
          />
        )}

        {route.name === "rules" && (
          <RulesView settings={settings} onSaved={setSettings} />
        )}

        {route.name === "templates" && (
          <TemplatesView templates={templates} onSaved={() => refresh().catch((cause) => logError("interface", "Rafraîchissement impossible", cause))} />
        )}

      </main>

      {modal === "create" && (
        <CreateWizard
          templates={templates}
          onCancel={() => setModal(null)}
          onCreated={(created) => {
            setModal(null);
            setRoute({ name: "course", id: created.id });
            refresh().catch((cause) => logError("interface", "Rafraîchissement impossible", cause));
          }}
        />
      )}

      {modal === "settings" && (
        <SettingsModal
          environment={environment}
          templates={templates}
          workspace={workspace}
          settings={settings}
          onSaved={setSettings}
          onEnvironmentChanged={() =>
            refresh().catch((cause) =>
              logError("interface", "Rafraîchissement impossible", cause),
            )
          }
          onClose={() => setModal(null)}
        />
      )}

      <Console open={consoleOpen} onClose={() => setConsoleOpen(false)} />
    </div>
  );
}
