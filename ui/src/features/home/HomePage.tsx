import type { ImportReportDto } from "../../bindings/ImportReportDto";
import type { StatusDto } from "../../bindings/StatusDto";
import { ImportPanel } from "./ImportPanel";
import { PhaseTrack } from "./PhaseTrack";

interface Props {
  status: StatusDto | null;
  lastImport: ImportReportDto | null;
  setLastImport: (report: ImportReportDto) => void;
}

export function HomePage({ status, lastImport, setLastImport }: Props) {
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
        {summoner?.profileIconUrl && <img className="hero-icon" src={summoner.profileIconUrl} alt="" width={56} height={56} />}
        <div>
          <h1>{summoner ? `Connected as ${summoner.riotId}` : "Connected to the League client"}</h1>
          <p className="hero-sub">
            {summoner && <span>Level {summoner.level}</span>}
            <span>{status.patch ? `Patch ${status.patch}` : "Loading game data"}</span>
          </p>
        </div>
      </section>
      <PhaseTrack session={status.session} />
      <ImportPanel report={lastImport} canReimport={status.session === "champSelect"} onReport={setLastImport} />
    </>
  );
}
