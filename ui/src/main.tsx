import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "@fontsource/barlow/400.css";
import "@fontsource/barlow/500.css";
import "@fontsource/barlow-semi-condensed/600.css";
import "@fontsource/barlow-semi-condensed/700.css";
import "./styles/tokens.css";
import "./styles/app.css";
import { App } from "./App";
import { tauriBackend } from "./lib/backend";
import { demoBackend } from "./lib/demoBackend";
import { BackendProvider } from "./lib/BackendContext";

// Outside the Tauri webview (plain `npm run dev`) there is no Rust core, so use sample data.
const inTauri = "__TAURI_INTERNALS__" in window;

const root = document.getElementById("root");
if (!root) throw new Error("#root is missing from index.html");

createRoot(root).render(
  <StrictMode>
    <BackendProvider backend={inTauri ? tauriBackend : demoBackend}>
      <App />
    </BackendProvider>
  </StrictMode>,
);
