import { useState } from "react";
import { HomePage } from "./features/home/HomePage";
import { SettingsPage } from "./features/settings/SettingsPage";
import { AboutPage } from "./features/about/AboutPage";
import { useCoreState } from "./lib/useCoreState";

type Page = "home" | "settings" | "about";

const PAGES: Array<{ id: Page; label: string; icon: string }> = [
  { id: "home", label: "Home", icon: "M3 9.5 10 4l7 5.5V16a1 1 0 0 1-1 1h-3.5v-4.5h-5V17H4a1 1 0 0 1-1-1Z" },
  {
    id: "settings",
    label: "Settings",
    icon: "M4 6h12M4 14h12M7.5 4v4M12.5 12v4",
  },
  { id: "about", label: "About", icon: "M10 9v5M10 6.2v.3M10 17.5a7.5 7.5 0 1 0 0-15 7.5 7.5 0 0 0 0 15Z" },
];

/** Remembers whether the player opened the menu. Storage can be missing or
 * blocked; the menu then simply starts collapsed. */
const RAIL_KEY = "rail-expanded";

function storedExpanded(): boolean {
  try {
    return localStorage.getItem(RAIL_KEY) === "true";
  } catch {
    return false;
  }
}

function storeExpanded(expanded: boolean) {
  try {
    localStorage.setItem(RAIL_KEY, String(expanded));
  } catch {
    // Not remembered; nothing else depends on it.
  }
}

export function App() {
  const [page, setPage] = useState<Page>("home");
  const [expanded, setExpanded] = useState(storedExpanded);
  const core = useCoreState();
  const connected = core.status != null && core.status.session !== "disconnected";
  const connection = connected ? "Client connected" : "Client not found";

  function toggleRail() {
    setExpanded((was) => {
      storeExpanded(!was);
      return !was;
    });
  }

  return (
    <div className="shell" data-rail={expanded ? "expanded" : "collapsed"}>
      <nav className="rail" aria-label="Main">
        <div className="rail-brand">
          <svg className="rail-mark" viewBox="0 0 32 32" aria-hidden="true">
            <path d="M16 6.5 25.5 16 16 25.5 6.5 16Z" fill="none" stroke="currentColor" strokeWidth="2" />
            <path d="M16 12 20 16 16 20 12 16Z" fill="currentColor" />
          </svg>
          <span className="rail-text">LoL Overlay</span>
        </div>
        <button
          type="button"
          className="rail-toggle"
          aria-expanded={expanded}
          aria-label={expanded ? "Collapse menu" : "Expand menu"}
          title={expanded ? "Collapse menu" : "Expand menu"}
          onClick={toggleRail}
        >
          <svg viewBox="0 0 20 20" aria-hidden="true">
            <path d={expanded ? "M12 5 7 10l5 5" : "M8 5l5 5-5 5"} />
          </svg>
        </button>
        <ul className="rail-links">
          {PAGES.map((p) => (
            <li key={p.id}>
              <button
                type="button"
                className="rail-link"
                aria-current={page === p.id ? "page" : undefined}
                title={expanded ? undefined : p.label}
                onClick={() => setPage(p.id)}
              >
                <svg className="rail-icon" viewBox="0 0 20 20" aria-hidden="true">
                  <path d={p.icon} />
                </svg>
                <span className="rail-text">{p.label}</span>
              </button>
            </li>
          ))}
        </ul>
        <p className="rail-connection" data-connected={connected} title={expanded ? undefined : connection}>
          <span className="dot" aria-hidden="true" />
          <span className="rail-text">{connection}</span>
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
