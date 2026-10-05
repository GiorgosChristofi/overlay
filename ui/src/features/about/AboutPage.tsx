import { useEffect, useState } from "react";
import { useBackend } from "../../lib/BackendContext";

export const LEGAL_NOTICE =
  // Riot's required wording; keep it verbatim.
  "LoL Overlay is not endorsed by Riot Games and does not reflect the views or opinions of Riot Games or anyone officially involved in producing or managing Riot Games properties. Riot Games and all associated properties are trademarks or registered trademarks of Riot Games, Inc.";

export function AboutPage({ patch }: { patch: string | null }) {
  const backend = useBackend();
  const [version, setVersion] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    backend.appVersion().then(setVersion, () => setVersion(null));
  }, [backend]);

  async function openLogs() {
    setError(null);
    try {
      await backend.openLogsFolder();
    } catch (e) {
      setError((e as { message?: string }).message ?? "Could not open the logs folder");
    }
  }

  return (
    <>
      <header className="page-head">
        <h1>About</h1>
      </header>
      <dl className="facts">
        <dt>Version</dt>
        <dd>{version ?? "Unknown"}</dd>
        <dt>Game data</dt>
        <dd>{patch ? `Riot Data Dragon, patch ${patch}` : "Loading from Riot Data Dragon"}</dd>
      </dl>
      <p className="legal">{LEGAL_NOTICE}</p>
      <button type="button" className="button button-quiet" onClick={openLogs}>
        Open logs folder
      </button>
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
    </>
  );
}
