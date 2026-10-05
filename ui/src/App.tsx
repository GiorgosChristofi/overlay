import { useState } from "react";
import { HomePage } from "./features/home/HomePage";
import { SettingsPage } from "./features/settings/SettingsPage";
import { AboutPage } from "./features/about/AboutPage";
import { useCoreState } from "./lib/useCoreState";

type Page = "home" | "settings" | "about";

const PAGES: Array<{ id: Page; label: string }> = [
  { id: "home", label: "Home" },
  { id: "settings", label: "Settings" },
  { id: "about", label: "About" },
];

export function App() {
  const [page, setPage] = useState<Page>("home");
  const core = useCoreState();
  const connected = core.status != null && core.status.session !== "disconnected";

  return (
    <div className="shell">
      <nav className="rail" aria-label="Main">
        <div className="rail-brand">
          <svg className="rail-mark" viewBox="0 0 32 32" aria-hidden="true">
            <path d="M16 6.5 25.5 16 16 25.5 6.5 16Z" fill="none" stroke="currentColor" strokeWidth="2" />
            <path d="M16 12 20 16 16 20 12 16Z" fill="currentColor" />
          </svg>
          <span>LoL Overlay</span>
        </div>
        <ul className="rail-links">
          {PAGES.map((p) => (
            <li key={p.id}>
              <button
                type="button"
                className="rail-link"
                aria-current={page === p.id ? "page" : undefined}
                onClick={() => setPage(p.id)}
              >
                {p.label}
              </button>
            </li>
          ))}
        </ul>
        <p className="rail-connection" data-connected={connected}>
          <span className="dot" aria-hidden="true" />
          {connected ? "Client connected" : "Client not found"}
        </p>
      </nav>
      <main className="page">
        {page === "home" && <HomePage {...core} />}
        {page === "settings" && <SettingsPage />}
        {page === "about" && <AboutPage patch={core.status?.patch ?? null} />}
      </main>
    </div>
  );
}
