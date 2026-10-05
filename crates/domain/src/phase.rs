use serde::{Deserialize, Deserializer, Serialize};

/// The League client's gameflow phase (`/lol-gameflow/v1/gameflow-phase`).
///
/// Riot adds phases without notice, so unknown values are kept as
/// [`GameflowPhase::Unknown`] instead of failing to parse.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum GameflowPhase {
    None,
    Lobby,
    Matchmaking,
    CheckedIntoTournament,
    ReadyCheck,
    ChampSelect,
    GameStart,
    InProgress,
    Reconnect,
    WaitingForStats,
    PreEndOfGame,
    EndOfGame,
    TerminatedInError,
    Unknown(String),
}

impl GameflowPhase {
    pub fn parse(raw: &str) -> Self {
        match raw {
            "None" => Self::None,
            "Lobby" => Self::Lobby,
            "Matchmaking" => Self::Matchmaking,
            "CheckedIntoTournament" => Self::CheckedIntoTournament,
            "ReadyCheck" => Self::ReadyCheck,
            "ChampSelect" => Self::ChampSelect,
            "GameStart" => Self::GameStart,
            "InProgress" => Self::InProgress,
            "Reconnect" => Self::Reconnect,
            "WaitingForStats" => Self::WaitingForStats,
            "PreEndOfGame" => Self::PreEndOfGame,
            "EndOfGame" => Self::EndOfGame,
            "TerminatedInError" => Self::TerminatedInError,
            other => Self::Unknown(other.to_owned()),
        }
    }
}

impl<'de> Deserialize<'de> for GameflowPhase {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Ok(Self::parse(&raw))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_phase() {
        let phase: GameflowPhase = serde_json::from_str("\"ChampSelect\"").unwrap();
        assert_eq!(phase, GameflowPhase::ChampSelect);
    }

    #[test]
    fn keeps_unknown_phase_instead_of_failing() {
        let phase: GameflowPhase = serde_json::from_str("\"BrandNewPhase\"").unwrap();
        assert_eq!(phase, GameflowPhase::Unknown("BrandNewPhase".into()));
    }
}
