import type { SessionStateDto } from "../../bindings/SessionStateDto";

const LABELS: Record<SessionStateDto, string> = {
  disconnected: "Offline",
  clientReady: "In client",
  lobby: "Lobby",
  champSelect: "Champ select",
  inGame: "In game",
  postGame: "Post game",
};

/** Where the session is right now, as one small pill. */
export function PhasePill({ session }: { session: SessionStateDto }) {
  return (
    <p className="phase-pill" data-session={session} role="status" aria-label={`Game session: ${LABELS[session]}`}>
      <span className="dot" aria-hidden="true" />
      {LABELS[session]}
    </p>
  );
}
