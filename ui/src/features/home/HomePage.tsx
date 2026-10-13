import { useState, type MouseEvent } from "react";
import type { ImportReportDto } from "../../bindings/ImportReportDto";
import type { StatusDto } from "../../bindings/StatusDto";
import { ImportPanel } from "./ImportPanel";
import { PhasePill } from "./PhasePill";

interface Props {
  status: StatusDto | null;
  lastImport: ImportReportDto | null;
  setLastImport: (report: ImportReportDto) => void;
}

/** Shown instead of the Riot ID once the player hides it (e.g. while streaming). */
export const HIDDEN_NAME = "Hidden";

/** Remembered so the name stays hidden after a restart. Storage can be
 * missing or blocked; the name then shows until hidden again. */
const NAME_HIDDEN_KEY = "summoner-name-hidden";

function storedNameHidden(): boolean {
  try {
    return localStorage.getItem(NAME_HIDDEN_KEY) === "true";
  } catch {
    return false;
  }
}

function storeNameHidden(hidden: boolean) {
  try {
    localStorage.setItem(NAME_HIDDEN_KEY, String(hidden));
  } catch {
    // Not remembered; the toggle still works for this session.
  }
}

export function HomePage({ status, lastImport, setLastImport }: Props) {
  const [nameHidden, setNameHidden] = useState(storedNameHidden);

  // `detail` counts consecutive clicks, so this fires on the third of a
  // quick triple click and not on single or double clicks.
  function onIconClick(event: MouseEvent) {
    if (event.detail !== 3) return;
    setNameHidden((was) => {
      storeNameHidden(!was);
      return !was;
    });
  }

  if (!status) {
    return <p className="loading">Starting…</p>;
  }

  if (status.session === "disconnected") {
    return (
      <section className="hero">
        <h1>Waiting for the League client</h1>
        <p className="hero-sub">Start League of Legends and this page connects on its own.</p>
      </section>
    );
  }

  const summoner = status.summoner;
  return (
    <>
      <section className="hero">
        {summoner && (
          <button
            type="button"
            className="hero-icon"
            aria-label={`Summoner icon. Click three times to ${nameHidden ? "show" : "hide"} your name.`}
            aria-pressed={nameHidden}
            onClick={onIconClick}
          >
            {summoner.profileIconUrl ? (
              <img src={summoner.profileIconUrl} alt="" width={56} height={56} draggable={false} />
            ) : (
              <svg viewBox="0 0 24 24" aria-hidden="true">
                <circle cx="12" cy="9" r="4" fill="none" stroke="currentColor" strokeWidth="1.6" />
                <path d="M4.5 20c1.4-3.4 4.2-5 7.5-5s6.1 1.6 7.5 5" fill="none" stroke="currentColor" strokeWidth="1.6" />
              </svg>
            )}
          </button>
        )}
        <div className="hero-text">
          <h1>{summoner ? `Connected as ${nameHidden ? HIDDEN_NAME : summoner.riotId}` : "Connected to the League client"}</h1>
          <p className="hero-sub">
            {summoner && <span>Level {summoner.level}</span>}
            <span>{status.patch ? `Patch ${status.patch}` : "Loading game data"}</span>
          </p>
        </div>
        <PhasePill session={status.session} />
      </section>
      <ImportPanel report={lastImport} canReimport={status.session === "champSelect"} onReport={setLastImport} />
    </>
  );
}
