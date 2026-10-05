//! Champion select session (`/lol-champ-select/v1/session`) and the rule for
//! "has the local player locked a champion yet?".
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
    pub my_team: Vec<TeamMember>,
    pub actions: Vec<Vec<Action>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TeamMember {
    pub cell_id: i64,
    pub champion_id: i64,
    pub assigned_position: String,
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

/// A champion the local player has committed to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LockedPick {
    pub champion_id: i64,
    pub role: Role,
}

/// Returns the local player's locked champion, if any.
///
/// - Draft and blind: locked once the player's pick action is completed.
/// - Modes without pick actions (e.g. ARAM): the assigned champion counts as locked.
pub fn locked_pick(session: &ChampSelectSession) -> Option<LockedPick> {
    let me = session
        .my_team
        .iter()
        .find(|m| m.cell_id == session.local_player_cell_id)?;
    let role = Role::from_assigned_position(&me.assigned_position);

    let mut my_picks = session
        .actions
        .iter()
        .flatten()
        .filter(|a| a.actor_cell_id == session.local_player_cell_id && a.kind == "pick")
        .peekable();

    if my_picks.peek().is_none() {
        return (me.champion_id > 0).then_some(LockedPick {
            champion_id: me.champion_id,
            role,
        });
    }

    my_picks
        .find(|a| a.completed && a.champion_id > 0)
        .map(|a| LockedPick {
            champion_id: a.champion_id,
            role,
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
    fn hovering_is_not_locking() {
        let s = session(vec![vec![pick(2, 99, false)]], 99);
        assert_eq!(locked_pick(&s), None);
    }

    #[test]
    fn completed_pick_is_locked_with_role() {
        let s = session(vec![vec![pick(1, 86, true)], vec![pick(2, 99, true)]], 99);
        assert_eq!(
            locked_pick(&s),
            Some(LockedPick {
                champion_id: 99,
                role: Role::Middle
            })
        );
    }

    #[test]
    fn teammates_pick_does_not_count() {
        let s = session(vec![vec![pick(1, 86, true)], vec![pick(2, 0, false)]], 0);
        assert_eq!(locked_pick(&s), None);
    }

    #[test]
    fn modes_without_pick_actions_use_assigned_champion() {
        let mut s = session(vec![], 222);
        s.my_team[1].assigned_position.clear();
        assert_eq!(
            locked_pick(&s),
            Some(LockedPick {
                champion_id: 222,
                role: Role::Any
            })
        );
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
        let s = session(vec![vec![ban], vec![pick(2, 99, false)]], 99);
        assert_eq!(locked_pick(&s), None);
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
