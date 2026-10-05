//! The client-session state machine. Transitions are pure so the whole
//! lifecycle can be tested without a League client.

use crate::phase::GameflowPhase;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum SessionState {
    Disconnected,
    ClientReady,
    Lobby,
    ChampSelect,
    InGame,
    PostGame,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionEvent {
    ClientConnected,
    ClientDisconnected,
    PhaseChanged(GameflowPhase),
}

/// Returns the next state. Unknown phases keep the current state.
pub fn transition(current: SessionState, event: &SessionEvent) -> SessionState {
    match event {
        SessionEvent::ClientDisconnected => SessionState::Disconnected,
        SessionEvent::ClientConnected => match current {
            SessionState::Disconnected => SessionState::ClientReady,
            other => other,
        },
        SessionEvent::PhaseChanged(phase) => {
            if current == SessionState::Disconnected {
                return current;
            }
            state_for_phase(phase).unwrap_or(current)
        }
    }
}

fn state_for_phase(phase: &GameflowPhase) -> Option<SessionState> {
    use GameflowPhase as P;
    Some(match phase {
        P::None | P::TerminatedInError => SessionState::ClientReady,
        P::Lobby | P::Matchmaking | P::CheckedIntoTournament | P::ReadyCheck => SessionState::Lobby,
        P::ChampSelect => SessionState::ChampSelect,
        P::GameStart | P::InProgress | P::Reconnect => SessionState::InGame,
        P::WaitingForStats | P::PreEndOfGame | P::EndOfGame => SessionState::PostGame,
        P::Unknown(_) => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn phase_strategy() -> impl Strategy<Value = GameflowPhase> {
        prop_oneof![
            Just(GameflowPhase::None),
            Just(GameflowPhase::Lobby),
            Just(GameflowPhase::ReadyCheck),
            Just(GameflowPhase::ChampSelect),
            Just(GameflowPhase::InProgress),
            Just(GameflowPhase::EndOfGame),
            "[A-Z][a-z]{3,8}".prop_map(|s| GameflowPhase::parse(&s)),
        ]
    }

    fn event_strategy() -> impl Strategy<Value = SessionEvent> {
        prop_oneof![
            Just(SessionEvent::ClientConnected),
            Just(SessionEvent::ClientDisconnected),
            phase_strategy().prop_map(SessionEvent::PhaseChanged),
        ]
    }

    #[test]
    fn full_game_lifecycle() {
        let events = [
            SessionEvent::ClientConnected,
            SessionEvent::PhaseChanged(GameflowPhase::Lobby),
            SessionEvent::PhaseChanged(GameflowPhase::ChampSelect),
            SessionEvent::PhaseChanged(GameflowPhase::InProgress),
            SessionEvent::PhaseChanged(GameflowPhase::EndOfGame),
            SessionEvent::PhaseChanged(GameflowPhase::None),
        ];
        let states: Vec<_> = events
            .iter()
            .scan(SessionState::Disconnected, |s, e| {
                *s = transition(*s, e);
                Some(*s)
            })
            .collect();
        assert_eq!(
            states,
            vec![
                SessionState::ClientReady,
                SessionState::Lobby,
                SessionState::ChampSelect,
                SessionState::InGame,
                SessionState::PostGame,
                SessionState::ClientReady,
            ]
        );
    }

    proptest! {
        #[test]
        fn disconnect_always_wins(start in event_strategy(), events in proptest::collection::vec(event_strategy(), 0..20)) {
            let mut state = transition(SessionState::Disconnected, &start);
            for e in &events { state = transition(state, e); }
            prop_assert_eq!(transition(state, &SessionEvent::ClientDisconnected), SessionState::Disconnected);
        }

        #[test]
        fn phases_are_ignored_while_disconnected(phase in phase_strategy()) {
            prop_assert_eq!(
                transition(SessionState::Disconnected, &SessionEvent::PhaseChanged(phase)),
                SessionState::Disconnected
            );
        }

        #[test]
        fn unknown_phase_never_changes_state(name in "Zz[a-z]{2,8}") {
            for s in [SessionState::ClientReady, SessionState::Lobby, SessionState::ChampSelect, SessionState::InGame] {
                prop_assert_eq!(transition(s, &SessionEvent::PhaseChanged(GameflowPhase::parse(&name))), s);
            }
        }
    }
}
