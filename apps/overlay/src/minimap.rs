//! Objective timers drawn over the game's minimap: where each pit is on it,
//! and how long until each spawn on the game's clock. Pure, so the rules
//! are tested without a window.

use ipc_types::{ObjectiveTimersDto, PitDto};
use std::time::{Duration, Instant};

/// Summoner's Rift as the minimap shows it, in game units (Riot's published
/// map bounds). The minimap is never rotated: blue base bottom left.
const MAP_MIN: (f32, f32) = (-120.0, -120.0);
const MAP_MAX: (f32, f32) = (14870.0, 14980.0);
/// Centre of each pit, in game units.
const BARON_PIT: (f32, f32) = (5007.0, 10471.0);
const DRAGON_PIT: (f32, f32) = (9866.0, 4414.0);
/// Where each River Scuttler spawns, in game units. Community-sourced, not
/// published by Riot: within a few pixels on the minimap, which is all a
/// countdown label needs.
const SCUTTLE_TOP: (f32, f32) = (4400.0, 9600.0);
const SCUTTLE_BOTTOM: (f32, f32) = (10500.0, 5170.0);

/// The game clock is read about once a second; between reads the overlay
/// counts on by itself, but never further than this past the last read, so
/// a paused game or a slow read cannot run a timer ahead of the game.
pub const MAX_EXTRAPOLATION: Duration = Duration::from_millis(1500);

/// Where a pit is on the minimap, as a fraction of its width and height
/// from the top left.
pub fn pit_position(pit: PitDto) -> (f32, f32) {
    let (x, y) = match pit {
        PitDto::Baron => BARON_PIT,
        PitDto::Dragon => DRAGON_PIT,
        PitDto::ScuttleTop => SCUTTLE_TOP,
        PitDto::ScuttleBottom => SCUTTLE_BOTTOM,
    };
    (
        (x - MAP_MIN.0) / (MAP_MAX.0 - MAP_MIN.0),
        // Game y grows upwards, screen y downwards.
        1.0 - (y - MAP_MIN.1) / (MAP_MAX.1 - MAP_MIN.1),
    )
}

/// The latest timers and when they arrived.
#[derive(Debug, Clone, PartialEq)]
pub struct Timers {
    pub dto: ObjectiveTimersDto,
    pub received: Instant,
}

/// One timer to draw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shown {
    pub pit: PitDto,
    /// `m:ss`.
    pub time: String,
}

impl Timers {
    /// The game clock at `now`: the last read plus the time since, capped.
    pub fn game_now_ms(&self, now: Instant) -> u64 {
        let since = now
            .saturating_duration_since(self.received)
            .min(MAX_EXTRAPOLATION);
        self.dto.game_time_ms + u64::try_from(since.as_millis()).unwrap_or(0)
    }

    /// The timers still counting at `now`, ready to draw.
    pub fn shown(&self, now: Instant) -> Vec<Shown> {
        let game_now = self.game_now_ms(now);
        self.dto
            .timers
            .iter()
            .filter(|t| t.spawn_at_ms > game_now)
            .map(|t| Shown {
                pit: t.pit,
                time: countdown(t.spawn_at_ms - game_now),
            })
            .collect()
    }
}

/// `m:ss`, rounded up so it reads `0:00` only once spawned.
pub fn countdown(remaining_ms: u64) -> String {
    let secs = remaining_ms.div_ceil(1000);
    format!("{}:{:02}", secs / 60, secs % 60)
}

/// The minimap's place on screen, in points: top-left corner and side.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    pub x: f32,
    pub y: f32,
    pub side: f32,
}

/// Smallest and largest minimap side accepted, in points.
const MIN_SIDE: f32 = 80.0;
const MAX_SIDE: f32 = 1200.0;

impl Placement {
    /// A first guess before the player fits it: the bottom-right corner of
    /// the main monitor, about the size of League's default minimap.
    pub fn default_for(monitor_w: f32, monitor_h: f32) -> Self {
        let side = (monitor_h * 0.26).clamp(MIN_SIDE, MAX_SIDE);
        Self {
            x: (monitor_w - side).max(0.0),
            y: (monitor_h - side).max(0.0),
            side,
        }
    }

    /// Stored as `x,y,side`; anything else (or a nonsense size) is ignored.
    pub fn parse(raw: &str) -> Option<Self> {
        let mut parts = raw.split(',').map(|p| p.trim().parse::<f32>().ok());
        let (x, y, side) = (parts.next()??, parts.next()??, parts.next()??);
        let ok = parts.next().is_none()
            && [x, y, side].iter().all(|v| v.is_finite())
            && (MIN_SIDE..=MAX_SIDE).contains(&side);
        ok.then_some(Self { x, y, side })
    }

    pub fn to_storage(self) -> String {
        format!("{},{},{}", self.x, self.y, self.side)
    }

    /// Kept square (the minimap is) and within sensible sizes.
    pub fn squared(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self {
            x,
            y,
            side: w.min(h).clamp(MIN_SIDE, MAX_SIDE),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ipc_types::ObjectiveTimerDto;

    fn timers(game_time_ms: u64, received: Instant) -> Timers {
        Timers {
            dto: ObjectiveTimersDto {
                game_time_ms,
                timers: vec![
                    ObjectiveTimerDto {
                        pit: PitDto::Dragon,
                        label: "Dragon".into(),
                        spawn_at_ms: 300_000,
                    },
                    ObjectiveTimerDto {
                        pit: PitDto::Baron,
                        label: "Grubs".into(),
                        spawn_at_ms: 480_000,
                    },
                ],
            },
            received,
        }
    }

    #[test]
    fn the_pits_sit_in_the_river_mirrored_across_the_map() {
        let (bx, by) = pit_position(PitDto::Baron);
        let (dx, dy) = pit_position(PitDto::Dragon);
        // Baron top left of centre, Dragon bottom right.
        assert!(bx < 0.5 && by < 0.5, "{bx},{by}");
        assert!(dx > 0.5 && dy > 0.5, "{dx},{dy}");
        // Mirror images of each other through the centre, within 1% (the
        // published bounds are not quite symmetric). The fitting markers sit
        // at these same points, so a fitted frame puts timers on the pits.
        assert!((bx + dx - 1.0).abs() < 0.01, "{}", bx + dx);
        assert!((by + dy - 1.0).abs() < 0.01, "{}", by + dy);
    }

    #[test]
    fn the_scuttlers_sit_in_the_river_beside_their_pit() {
        let near =
            |a: (f32, f32), b: (f32, f32)| ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();
        let (top, bottom) = (
            pit_position(PitDto::ScuttleTop),
            pit_position(PitDto::ScuttleBottom),
        );
        // Each is closer to its own pit than to the other one, and the two
        // mirror each other through the map centre as the pits do (within
        // 1%, under 3 pixels on a 280-point minimap).
        assert!(near(top, pit_position(PitDto::Baron)) < near(top, pit_position(PitDto::Dragon)));
        assert!(
            near(bottom, pit_position(PitDto::Dragon)) < near(bottom, pit_position(PitDto::Baron))
        );
        let (baron, dragon) = (pit_position(PitDto::Baron), pit_position(PitDto::Dragon));
        assert!((top.0 + bottom.0 - (baron.0 + dragon.0)).abs() < 0.01);
        assert!((top.1 + bottom.1 - (baron.1 + dragon.1)).abs() < 0.01);
    }

    #[test]
    fn counts_down_on_the_game_clock_between_reads() {
        let t0 = Instant::now();
        let t = timers(299_000, t0);
        assert_eq!(t.shown(t0)[0].time, "0:01");
        assert_eq!(t.shown(t0 + Duration::from_millis(400))[0].time, "0:01");
        // At 5:00 the dragon is up: only the grubs remain.
        let later = t.shown(t0 + Duration::from_millis(1000));
        assert_eq!(later.len(), 1);
        assert_eq!(later[0].pit, PitDto::Baron, "the grubs' pit");
        assert_eq!(later[0].time, "3:00");
    }

    #[test]
    fn never_runs_ahead_of_a_stalled_or_paused_clock() {
        let t0 = Instant::now();
        let t = timers(200_000, t0);
        let much_later = t0 + Duration::from_secs(30);
        assert_eq!(t.game_now_ms(much_later), 201_500);
        assert_eq!(t.shown(much_later)[0].time, "1:39");
    }

    #[test]
    fn countdowns_round_up_to_whole_seconds() {
        assert_eq!(countdown(0), "0:00");
        assert_eq!(countdown(1), "0:01");
        assert_eq!(countdown(59_001), "1:00");
        assert_eq!(countdown(6 * 60_000 + 5_400), "6:06");
    }

    #[test]
    fn a_placement_survives_storage_and_rejects_nonsense() {
        let p = Placement {
            x: 1620.5,
            y: 780.0,
            side: 300.0,
        };
        assert_eq!(Placement::parse(&p.to_storage()), Some(p));
        for bad in [
            "",
            "1,2",
            "1,2,3,4",
            "a,b,c",
            "0,0,NaN",
            "0,0,inf",
            "0,0,5",
            "0,0,99999",
        ] {
            assert_eq!(Placement::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn the_first_guess_is_the_bottom_right_corner() {
        let p = Placement::default_for(1920.0, 1080.0);
        assert_eq!(p.x + p.side, 1920.0);
        assert_eq!(p.y + p.side, 1080.0);
        assert!((250.0..320.0).contains(&p.side));
    }

    #[test]
    fn a_resized_minimap_stays_square_and_sane() {
        assert_eq!(Placement::squared(0.0, 0.0, 300.0, 280.0).side, 280.0);
        assert_eq!(Placement::squared(0.0, 0.0, 10.0, 10.0).side, 80.0);
    }
}
