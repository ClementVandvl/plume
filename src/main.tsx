import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { ConfirmProvider } from "./confirm";

// Layout work without Tauri: `npm run dev`, then open `/?mock`. The fake
// backend must be in place before the first render fires its commands.
if (import.meta.env.DEV && new URLSearchParams(window.location.search).has("mock")) {
  const { installDevMock } = await import("./devMock");
  installDevMock();
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ConfirmProvider>
      <App />
    </ConfirmProvider>
  </React.StrictMode>,
);
