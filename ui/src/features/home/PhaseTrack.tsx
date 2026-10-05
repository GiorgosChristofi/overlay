import type { SessionStateDto } from "../../bindings/SessionStateDto";

const STEPS: Array<{ state: SessionStateDto; label: string }> = [
  { state: "lobby", label: "Lobby" },
  { state: "champSelect", label: "Champ select" },
  { state: "inGame", label: "In game" },
  { state: "postGame", label: "Post game" },
];

/** The session as a rail: filled up to the current phase. */
export function PhaseTrack({ session }: { session: SessionStateDto }) {
  const current = STEPS.findIndex((s) => s.state === session);
  const progress = current < 0 ? 0 : current / (STEPS.length - 1);
  return (
    <ol className="track" aria-label="Game session" style={{ "--progress": progress } as React.CSSProperties}>
      {STEPS.map((step, i) => (
        <li
          key={step.state}
          className="track-step"
          data-state={i < current ? "past" : i === current ? "current" : "future"}
          aria-current={i === current ? "step" : undefined}
        >
          <span className="track-node" aria-hidden="true" />
          <span className="track-label">{step.label}</span>
        </li>
      ))}
    </ol>
  );
}
