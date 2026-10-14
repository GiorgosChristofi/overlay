//! Epic objective timers on Summoner's Rift: when the next monster in each
//! pit spawns, from the game clock and the kills the game reports.
//!
//! Only what the Live Client Data API states exactly is used: the game time
//! and `DragonKill` / `BaronKill` events. Normal camps are not reported by
//! that API, so they are not timed here.
//!
//! All times are game time in milliseconds (`gameData.gameTime`), never the
//! PC clock, so pauses and slow polls cannot shift a timer.
//!
//! Timings as of patch 26.1 (season 2026). Change them here when Riot does.

/// First dragon.
pub const DRAGON_FIRST_SPAWN_MS: u64 = 5 * 60_000;
/// Next dragon after a dragon (not Elder) is slain.
pub const DRAGON_RESPAWN_MS: u64 = 5 * 60_000;
/// Elder after a team's fourth dragon (soul), and after each Elder kill.
pub const ELDER_RESPAWN_MS: u64 = 6 * 60_000;
/// Dragons one team must slay before Elder replaces them.
pub const DRAGONS_FOR_SOUL: u32 = 4;
/// Voidgrubs: one group, no respawn (since 26.1).
pub const GRUBS_SPAWN_MS: u64 = 8 * 60_000;
/// Rift Herald: once, in the top pit, no respawn.
pub const HERALD_SPAWN_MS: u64 = 15 * 60_000;
/// Baron (20:00 since 26.1).
pub const BARON_FIRST_SPAWN_MS: u64 = 20 * 60_000;
pub const BARON_RESPAWN_MS: u64 = 6 * 60_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Team {
    /// Blue side (`ORDER`).
    Order,
    /// Red side (`CHAOS`).
    Chaos,
}

impl Team {
    /// The Live Client Data API's `team` field.
    pub fn from_api(raw: &str) -> Option<Self> {
        match raw {
            "ORDER" => Some(Self::Order),
            "CHAOS" => Some(Self::Chaos),
            _ => None,
        }
    }
}

/// An epic monster slain, as the game reported it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectiveKill {
    /// Game time of the kill.
    pub at_ms: u64,
    pub kind: KillKind,
    /// The killer's team; `None` when the killer is not a listed player.
    pub team: Option<Team>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KillKind {
    /// An elemental dragon.
    Dragon,
    Elder,
    Baron,
}

/// The two pits that hold epic monsters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Pit {
    /// Bottom river: dragons, then Elder.
    Dragon,
    /// Top river: Voidgrubs, then Herald, then Baron.
    Baron,
}

/// What spawns next in a pit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Monster {
    Dragon,
    Elder,
    Voidgrubs,
    Herald,
    Baron,
}

impl Monster {
    pub fn label(self) -> &'static str {
        match self {
            Self::Dragon => "Dragon",
            Self::Elder => "Elder",
            Self::Voidgrubs => "Grubs",
            Self::Herald => "Herald",
            Self::Baron => "Baron",
        }
    }
}

/// The next spawn in a pit, still to come.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Upcoming {
    pub pit: Pit,
    pub monster: Monster,
    /// Game time it spawns.
    pub spawn_at_ms: u64,
}

/// The next spawn in each pit at `now_ms`, or nothing for a pit whose
/// monster is up (the game shows those itself).
///
/// `kills` may come in any order; kills after `now_ms` are ignored.
pub fn upcoming(kills: &[ObjectiveKill], now_ms: u64) -> Vec<Upcoming> {
    let mut kills: Vec<&ObjectiveKill> = kills.iter().filter(|k| k.at_ms <= now_ms).collect();
    kills.sort_by_key(|k| k.at_ms);
    [
        next_in_dragon_pit(&kills),
        next_in_baron_pit(&kills, now_ms),
    ]
    .into_iter()
    .filter(|u| u.spawn_at_ms > now_ms)
    .collect()
}

fn next_in_dragon_pit(kills: &[&ObjectiveKill]) -> Upcoming {
    let mut next = (Monster::Dragon, DRAGON_FIRST_SPAWN_MS);
    let (mut order, mut chaos) = (0u32, 0u32);
    for kill in kills {
        next = match kill.kind {
            KillKind::Elder => (Monster::Elder, kill.at_ms + ELDER_RESPAWN_MS),
            KillKind::Dragon => {
                // A dragon by an unknown killer cannot count towards a soul.
                let count = match kill.team {
                    Some(Team::Order) => Some(&mut order),
                    Some(Team::Chaos) => Some(&mut chaos),
                    None => None,
                };
                let soul = count.is_some_and(|c| {
                    *c += 1;
                    *c >= DRAGONS_FOR_SOUL
                });
                if soul || next.0 == Monster::Elder {
                    (Monster::Elder, kill.at_ms + ELDER_RESPAWN_MS)
                } else {
                    (Monster::Dragon, kill.at_ms + DRAGON_RESPAWN_MS)
                }
            }
            KillKind::Baron => continue,
        };
    }
    Upcoming {
        pit: Pit::Dragon,
        monster: next.0,
        spawn_at_ms: next.1,
    }
}

/// Grubs, then Herald, then Baron share the top pit. Each early monster's
/// countdown runs until it spawns; while one is up the next one counts down
/// (the game shows the live monster itself), so the pit always says what
/// comes next.
fn next_in_baron_pit(kills: &[&ObjectiveKill], now_ms: u64) -> Upcoming {
    let at = |monster, spawn_at_ms| Upcoming {
        pit: Pit::Baron,
        monster,
        spawn_at_ms,
    };
    if now_ms < GRUBS_SPAWN_MS {
        return at(Monster::Voidgrubs, GRUBS_SPAWN_MS);
    }
    if now_ms < HERALD_SPAWN_MS {
        return at(Monster::Herald, HERALD_SPAWN_MS);
    }
    let last_baron = kills.iter().rev().find(|k| k.kind == KillKind::Baron);
    match last_baron {
        Some(kill) => at(Monster::Baron, kill.at_ms + BARON_RESPAWN_MS),
        None => at(Monster::Baron, BARON_FIRST_SPAWN_MS),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const MIN: u64 = 60_000;

    fn dragon(at_ms: u64, team: Option<Team>) -> ObjectiveKill {
        ObjectiveKill {
            at_ms,
            kind: KillKind::Dragon,
            team,
        }
    }

    fn kill(at_ms: u64, kind: KillKind) -> ObjectiveKill {
        ObjectiveKill {
            at_ms,
            kind,
            team: Some(Team::Order),
        }
    }

    fn pit(kills: &[ObjectiveKill], now_ms: u64, pit: Pit) -> Option<Upcoming> {
        upcoming(kills, now_ms).into_iter().find(|u| u.pit == pit)
    }

    #[test]
    fn at_the_start_the_first_dragon_and_the_grubs_count_down() {
        let next = upcoming(&[], 90_000);
        assert_eq!(
            next,
            [
                Upcoming {
                    pit: Pit::Dragon,
                    monster: Monster::Dragon,
                    spawn_at_ms: 5 * MIN
                },
                Upcoming {
                    pit: Pit::Baron,
                    monster: Monster::Voidgrubs,
                    spawn_at_ms: 8 * MIN
                },
            ]
        );
    }

    #[test]
    fn a_pit_whose_monster_is_up_shows_nothing() {
        assert_eq!(pit(&[], 6 * MIN, Pit::Dragon), None);
        assert_eq!(pit(&[], 5 * MIN, Pit::Dragon), None, "spawns exactly now");
    }

    #[test]
    fn a_slain_dragon_comes_back_five_minutes_after_the_kill() {
        let kills = [dragon(6 * MIN + 12_345, Some(Team::Order))];
        let next = pit(&kills, 7 * MIN, Pit::Dragon).unwrap();
        assert_eq!(next.monster, Monster::Dragon);
        assert_eq!(next.spawn_at_ms, 11 * MIN + 12_345);
    }

    #[test]
    fn a_fourth_dragon_for_one_team_brings_elder_six_minutes_later() {
        let kills = [
            dragon(5 * MIN, Some(Team::Order)),
            dragon(10 * MIN, Some(Team::Chaos)),
            dragon(15 * MIN, Some(Team::Order)),
            dragon(20 * MIN, Some(Team::Order)),
            dragon(25 * MIN, Some(Team::Order)),
        ];
        let next = pit(&kills, 26 * MIN, Pit::Dragon).unwrap();
        assert_eq!(next.monster, Monster::Elder);
        assert_eq!(next.spawn_at_ms, 31 * MIN);
    }

    #[test]
    fn four_dragons_split_between_teams_are_no_soul() {
        let kills = [
            dragon(5 * MIN, Some(Team::Order)),
            dragon(10 * MIN, Some(Team::Chaos)),
            dragon(15 * MIN, Some(Team::Order)),
            dragon(20 * MIN, Some(Team::Chaos)),
        ];
        assert_eq!(
            pit(&kills, 21 * MIN, Pit::Dragon).unwrap().monster,
            Monster::Dragon
        );
    }

    #[test]
    fn elder_respawns_six_minutes_after_it_is_slain() {
        let mut kills: Vec<ObjectiveKill> = (1..=4)
            .map(|i| dragon(i * 5 * MIN, Some(Team::Chaos)))
            .collect();
        kills.push(kill(27 * MIN, KillKind::Elder));
        let next = pit(&kills, 28 * MIN, Pit::Dragon).unwrap();
        assert_eq!(next.monster, Monster::Elder);
        assert_eq!(next.spawn_at_ms, 33 * MIN);
    }

    #[test]
    fn a_dragon_with_an_unknown_killer_still_restarts_the_timer() {
        let kills = [dragon(5 * MIN, None)];
        assert_eq!(
            pit(&kills, 6 * MIN, Pit::Dragon).unwrap().spawn_at_ms,
            10 * MIN
        );
    }

    #[test]
    fn the_top_pit_counts_down_grubs_then_herald_then_baron() {
        let at = |now| pit(&[], now, Pit::Baron).unwrap();
        assert_eq!(at(MIN).monster, Monster::Voidgrubs);
        assert_eq!(at(MIN).spawn_at_ms, 8 * MIN);
        assert_eq!(at(9 * MIN).monster, Monster::Herald);
        assert_eq!(at(9 * MIN).spawn_at_ms, 15 * MIN);
        assert_eq!(at(16 * MIN).monster, Monster::Baron);
        assert_eq!(at(16 * MIN).spawn_at_ms, 20 * MIN);
        assert_eq!(pit(&[], 21 * MIN, Pit::Baron), None, "Baron is up");
    }

    #[test]
    fn a_slain_baron_comes_back_six_minutes_after_the_kill() {
        let kills = [kill(23 * MIN + 500, KillKind::Baron)];
        let next = pit(&kills, 24 * MIN, Pit::Baron).unwrap();
        assert_eq!(next.monster, Monster::Baron);
        assert_eq!(next.spawn_at_ms, 29 * MIN + 500);
        assert_eq!(pit(&kills, 30 * MIN, Pit::Baron), None);
    }

    #[test]
    fn kills_arrive_in_any_order_and_future_ones_are_ignored() {
        let kills = [
            dragon(20 * MIN, Some(Team::Order)),
            dragon(5 * MIN, Some(Team::Order)),
            dragon(12 * MIN, Some(Team::Order)),
        ];
        let next = pit(&kills, 13 * MIN, Pit::Dragon).unwrap();
        assert_eq!(
            next.spawn_at_ms,
            17 * MIN,
            "the 20:00 kill has not happened yet"
        );
    }

    fn any_kill() -> impl Strategy<Value = ObjectiveKill> {
        (
            0u64..60 * MIN,
            prop_oneof![
                Just(KillKind::Dragon),
                Just(KillKind::Elder),
                Just(KillKind::Baron)
            ],
            prop_oneof![Just(None), Just(Some(Team::Order)), Just(Some(Team::Chaos))],
        )
            .prop_map(|(at_ms, kind, team)| ObjectiveKill { at_ms, kind, team })
    }

    proptest! {
        /// Whatever happened, every timer shown is in the future, no more
        /// than the longest respawn away (or the first spawn), and at most
        /// one per pit.
        #[test]
        fn timers_are_always_ahead_and_never_too_far(
            kills in proptest::collection::vec(any_kill(), 0..12),
            now in 0u64..60 * MIN,
        ) {
            let next = upcoming(&kills, now);
            prop_assert!(next.len() <= 2);
            prop_assert!(next.iter().filter(|u| u.pit == Pit::Dragon).count() <= 1);
            for u in &next {
                prop_assert!(u.spawn_at_ms > now);
                let longest = BARON_FIRST_SPAWN_MS.max(BARON_RESPAWN_MS).max(ELDER_RESPAWN_MS);
                prop_assert!(u.spawn_at_ms - now <= longest, "{u:?} at {now}");
            }
        }
    }
}
