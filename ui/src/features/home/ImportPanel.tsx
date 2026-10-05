import { useState } from "react";
import type { ImportReportDto } from "../../bindings/ImportReportDto";
import type { StepResultDto } from "../../bindings/StepResultDto";
import { useBackend } from "../../lib/BackendContext";

const STEPS: Array<{ key: "runes" | "itemSet" | "summonerSpells"; label: string }> = [
  { key: "runes", label: "Runes" },
  { key: "itemSet", label: "Item set" },
  { key: "summonerSpells", label: "Summoner spells" },
];

function StepGlyph({ result }: { result: StepResultDto }) {
  const label = { done: "Done", skipped: "Skipped", failed: "Failed" }[result.status];
  return (
    <span className="glyph" data-status={result.status} role="img" aria-label={label}>
      <svg viewBox="0 0 16 16" aria-hidden="true">
        {result.status === "done" && <path d="M3.5 8.5 6.5 11.5 12.5 4.5" />}
        {result.status === "skipped" && <path d="M4 8h8" />}
        {result.status === "failed" && <path d="M8 3.5v6M8 12v.5" />}
      </svg>
    </span>
  );
}

function stepText(result: StepResultDto): string {
  switch (result.status) {
    case "done":
      return result.detail;
    case "skipped":
      return result.reason;
    case "failed":
      return result.error;
  }
}

interface Props {
  report: ImportReportDto | null;
  canReimport: boolean;
  onReport: (report: ImportReportDto) => void;
}

export function ImportPanel({ report, canReimport, onReport }: Props) {
  const backend = useBackend();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function reimport() {
    setBusy(true);
    setError(null);
    try {
      onReport(await backend.reimport());
    } catch (e) {
      setError((e as { message?: string }).message ?? "Import failed");
    } finally {
      setBusy(false);
    }
  }

  if (!report) {
    return (
      <section className="import import-empty" aria-labelledby="import-title">
        <h2 id="import-title">Champion select</h2>
        <p>Lock in a champion and your runes, item set and summoner spells are set for you.</p>
      </section>
    );
  }

  return (
    <section className="import" aria-labelledby="import-title">
      <header className="import-head">
        <h2 id="import-title">
          {report.championName} <span className="import-role">{report.role}</span>
        </h2>
        {canReimport && (
          <button type="button" className="button" onClick={reimport} disabled={busy}>
            {busy ? "Importing…" : "Import again"}
          </button>
        )}
      </header>
      <ul className="steps">
        {STEPS.map(({ key, label }) => (
          <li key={key} className="step">
            <StepGlyph result={report[key]} />
            <span className="step-label">{label}</span>
            <span className="step-text">{stepText(report[key])}</span>
          </li>
        ))}
      </ul>
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
    </section>
  );
}
