use serde::{Deserialize, Serialize};

/// The logged-in player (`/lol-summoner/v1/current-summoner`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Summoner {
    pub game_name: String,
    pub tag_line: String,
    pub summoner_level: u32,
    pub profile_icon_id: i64,
    pub puuid: String,
    pub summoner_id: i64,
}

impl Summoner {
    /// Riot ID as shown to players, e.g. `Name#EUW`.
    pub fn riot_id(&self) -> String {
        format!("{}#{}", self.game_name, self.tag_line)
    }
}
