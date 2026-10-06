//! Champion select session (`/lol-champ-select/v1/session`) and the rule for
//! "which champion is the local player going for?".
//!
//! Only the fields we use are modelled; everything else is ignored so new
//! fields from Riot never break parsing.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Top,
    Jungle,
    Middle,
    Bottom,
    #[serde(rename = "utility", alias = "support")]
    Support,
    /// Blind pick, ARAM, or anything without an assigned position.
    #[serde(other)]
    Any,
}

impl Role {
    pub fn from_assigned_position(raw: &str) -> Self {
        match raw.to_ascii_lowercase().as_str() {
            "top" => Self::Top,
            "jungle" => Self::Jungle,
            "middle" | "mid" => Self::Middle,
            "bottom" | "adc" => Self::Bottom,
            "utility" | "support" => Self::Support,
            _ => Self::Any,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Top => "Top",
            Self::Jungle => "Jungle",
            Self::Middle => "Middle",
            Self::Bottom => "Bottom",
            Self::Support => "Support",
            Self::Any => "Any role",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ChampSelectSession {
    pub local_player_cell_id: i64,
    /// True in modes with a champion bench (ARAM and its variants).
    pub bench_enabled: bool,
    pub my_team: Vec<TeamMember>,
    pub actions: Vec<Vec<Action>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TeamMember {
    pub cell_id: i64,
    pub champion_id: i64,
    pub assigned_position: String,
    /// Champion hovered before the player's pick turn starts.
    pub champion_pick_intent: i64,
    pub spell1_id: i64,
    pub spell2_id: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Action {
    pub actor_cell_id: i64,
    pub champion_id: i64,
    pub completed: bool,
    #[serde(rename = "type")]
    pub kind: String,
    pub is_in_progress: bool,
}

/// Which statistics apply to a pick. Lane roles only mean something in
/// Summoner's Rift modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GameMode {
    #[default]
    SummonersRift,
    Aram,
}

/// The champion the local player has hovered or locked in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChampionPick {
    pub champion_id: i64,
    pub role: Role,
    pub mode: GameMode,
}

impl ChampionPick {
    /// What the UI shows next to the champion name.
    pub fn role_label(&self) -> &'static str {
        match self.mode {
            GameMode::Aram => "ARAM",
            GameMode::SummonersRift => self.role.label(),
        }
    }
}

/// Returns the champion the local player has chosen or is hovering, if any.
///
/// - Draft and blind: the locked champion once the pick is completed; before
///   that the hovered one (the pick action's champion during the player's
///   turn, `championPickIntent` before it).
/// - Modes without pick actions (e.g. ARAM): the assigned champion.
pub fn selected_pick(session: &ChampSelectSession) -> Option<ChampionPick> {
    let me = session
        .my_team
        .iter()
        .find(|m| m.cell_id == session.local_player_cell_id)?;
    let role = Role::from_assigned_position(&me.assigned_position);
    let mode = if session.bench_enabled {
        GameMode::Aram
    } else {
        GameMode::SummonersRift
    };

    let my_picks: Vec<&Action> = session
        .actions
        .iter()
        .flatten()
        .filter(|a| a.actor_cell_id == session.local_player_cell_id && a.kind == "pick")
        .collect();

    let champion_id = if my_picks.is_empty() {
        me.champion_id
    } else {
        let locked = my_picks.iter().find(|a| a.completed && a.champion_id > 0);
        let hovered = my_picks.iter().find(|a| !a.completed && a.champion_id > 0);
        locked
            .or(hovered)
            .map_or(me.champion_pick_intent, |a| a.champion_id)
    };
    (champion_id > 0).then_some(ChampionPick {
        champion_id,
        role,
        mode,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(actions: Vec<Vec<Action>>, champion_id: i64) -> ChampSelectSession {
        ChampSelectSession {
            local_player_cell_id: 2,
            my_team: vec![
                TeamMember {
                    cell_id: 1,
                    champion_id: 86,
                    assigned_position: "top".into(),
                    ..Default::default()
                },
                TeamMember {
                    cell_id: 2,
                    champion_id,
                    assigned_position: "middle".into(),
                    ..Default::default()
                },
            ],
            actions,
            ..Default::default()
        }
    }

    fn pick(actor: i64, champion_id: i64, completed: bool) -> Action {
        Action {
            actor_cell_id: actor,
            champion_id,
            completed,
            kind: "pick".into(),
            is_in_progress: !completed,
        }
    }

    #[test]
    fn hovering_during_the_pick_turn_counts() {
        let s = session(vec![vec![pick(2, 99, false)]], 0);
        assert_eq!(selected_pick(&s).map(|p| p.champion_id), Some(99));
    }

    #[test]
    fn hovering_before_the_pick_turn_counts() {
        let mut s = session(vec![vec![pick(1, 86, false)], vec![pick(2, 0, false)]], 0);
        assert_eq!(selected_pick(&s), None);
        s.my_team[1].champion_pick_intent = 103;
        assert_eq!(selected_pick(&s).map(|p| p.champion_id), Some(103));
    }

    #[test]
    fn locked_champion_wins_over_a_stale_intent() {
        let mut s = session(vec![vec![pick(2, 99, true)]], 99);
        s.my_team[1].champion_pick_intent = 103;
        assert_eq!(selected_pick(&s).map(|p| p.champion_id), Some(99));
    }

    #[test]
    fn completed_pick_is_locked_with_role() {
        let s = session(vec![vec![pick(1, 86, true)], vec![pick(2, 99, true)]], 99);
        assert_eq!(
            selected_pick(&s),
            Some(ChampionPick {
                champion_id: 99,
                role: Role::Middle,
                mode: GameMode::SummonersRift,
            })
        );
    }

    #[test]
    fn teammates_pick_does_not_count() {
        let s = session(vec![vec![pick(1, 86, true)], vec![pick(2, 0, false)]], 0);
        assert_eq!(selected_pick(&s), None);
    }

    #[test]
    fn modes_without_pick_actions_use_assigned_champion() {
        let mut s = session(vec![], 222);
        s.my_team[1].assigned_position.clear();
        s.bench_enabled = true;
        let pick = selected_pick(&s).unwrap();
        assert_eq!(
            pick,
            ChampionPick {
                champion_id: 222,
                role: Role::Any,
                mode: GameMode::Aram,
            }
        );
        assert_eq!(pick.role_label(), "ARAM");
    }

    #[test]
    fn ban_actions_are_ignored() {
        let ban = Action {
            actor_cell_id: 2,
            champion_id: 157,
            completed: true,
            kind: "ban".into(),
            is_in_progress: false,
        };
        let s = session(vec![vec![ban], vec![pick(2, 0, false)]], 0);
        assert_eq!(selected_pick(&s), None);
    }

    #[test]
    fn role_parsing_accepts_lcu_and_human_names() {
        assert_eq!(Role::from_assigned_position("UTILITY"), Role::Support);
        assert_eq!(Role::from_assigned_position(""), Role::Any);
        let r: Role = serde_json::from_str("\"support\"").unwrap();
        assert_eq!(r, Role::Support);
        let r: Role = serde_json::from_str("\"somethingnew\"").unwrap();
        assert_eq!(r, Role::Any);
    }
}
