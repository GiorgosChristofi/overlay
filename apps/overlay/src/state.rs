//! What the overlay shows and how. Pure, so the rules are tested without a
//! window.

use crate::minimap::Timers;
use ipc_types::{OverlayMessage, OverlayViewDto};
use std::time::Instant;

#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    /// P pressed while the game (or the overlay) is focused.
    TogglePressed,
    /// Esc pressed: the shop closes, so does the full column.
    EscPressed,
    /// The × in the header: back to the compact strip.
    Collapse,
    /// The compact strip was clicked.
    Expand,
    /// Whether the game (or the overlay itself) is the focused window.
    Focus(bool),
    /// A new view from the desktop app.
    Message(OverlayMessage),
    /// The desktop app went away.
    Quit,
}

/// How the overlay is drawn right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Nothing drawn; clicks go through.
    Hidden,
    /// One line: the next item and the gold it still needs.
    Compact,
    /// The whole column (shop open).
    Expanded,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct OverlayState {
    /// The full column is open (toggled with P, like the shop).
    pub expanded: bool,
    /// The game or the overlay is the focused window; otherwise nothing is
    /// shown, so the overlay never floats over the desktop or other apps.
    pub focused: bool,
    /// `None` until a build is known.
    pub view: Option<OverlayViewDto>,
    /// Objective timers, `None` outside a Summoner's Rift game.
    pub objectives: Option<Timers>,
}

/// How the minimap layer is drawn right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MinimapMode {
    /// Nothing drawn; clicks go through.
    Hidden,
    /// The timers only; clicks go through to the minimap.
    Timers,
    /// The column is open: a frame the player drags and resizes to fit the
    /// minimap, with markers on the pits to line it up.
    Adjust,
}

impl OverlayState {
    pub fn apply(&mut self, input: Input) {
        match input {
            Input::TogglePressed => self.expanded = !self.expanded,
            Input::EscPressed | Input::Collapse | Input::Quit => self.expanded = false,
            Input::Expand => self.expanded = true,
            Input::Focus(focused) => self.focused = focused,
            Input::Message(OverlayMessage::Items(view)) => self.view = Some(*view),
            Input::Message(OverlayMessage::Clear) => {
                self.view = None;
                self.expanded = false;
            }
            // Timers have nothing to do with the build: `Clear` keeps them.
            Input::Message(OverlayMessage::Objectives(timers)) => {
                self.objectives = timers.map(|dto| Timers {
                    dto,
                    received: Instant::now(),
                });
            }
        }
    }

    pub fn minimap_mode(&self) -> MinimapMode {
        if !self.focused {
            MinimapMode::Hidden
        } else if self.expanded {
            MinimapMode::Adjust
        } else if self
            .objectives
            .as_ref()
            .is_some_and(|t| !t.dto.timers.is_empty())
        {
            MinimapMode::Timers
        } else {
            MinimapMode::Hidden
        }
    }

    pub fn mode(&self) -> Mode {
        if !self.focused {
            return Mode::Hidden;
        }
        if self.expanded {
            return Mode::Expanded;
        }
        // The strip only makes sense in a running game with something to buy.
        match &self.view {
            Some(v)
                if v.gold.is_some()
                    && (v.starting.is_some() || v.next.is_some() || v.boots.is_some()) =>
            {
                Mode::Compact
            }
            _ => Mode::Hidden,
        }
    }
}

/// Turns "key is down" samples into single presses, so holding P toggles once.
#[derive(Debug, Clone, Copy, Default)]
pub struct KeyEdge {
    down: bool,
}

impl KeyEdge {
    /// Returns true on the sample where the key goes down.
    pub fn pressed(&mut self, down: bool) -> bool {
        let edge = down && !self.down;
        self.down = down;
        edge
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ipc_types::{OverlayItemDto, RecipeNodeDto};

    fn view(in_game: bool) -> OverlayViewDto {
        let item = OverlayItemDto {
            id: 3020,
            name: "Sorcerer's Shoes".into(),
            icon_url: None,
            cost: 1100,
            count: 1,
            pick_rate: None,
        };
        OverlayViewDto {
            champion_name: "Lux".into(),
            role: "Middle".into(),
            source: "OP.GG".into(),
            gold: in_game.then_some(500),
            starting: None,
            boots: None,
            next: Some(RecipeNodeDto {
                item,
                owned: false,
                remaining_cost: 1100,
                affordable: false,
                components: vec![],
            }),
            next_pinned: false,
            removed_count: 0,
            blocks: vec![],
        }
    }

    fn in_game() -> OverlayState {
        OverlayState {
            expanded: false,
            focused: true,
            view: Some(view(true)),
            objectives: None,
        }
    }

    #[test]
    fn in_game_the_strip_shows_until_p_opens_the_column() {
        let mut s = in_game();
        assert_eq!(s.mode(), Mode::Compact);
        s.apply(Input::TogglePressed);
        assert_eq!(s.mode(), Mode::Expanded);
        s.apply(Input::TogglePressed);
        assert_eq!(s.mode(), Mode::Compact, "closing the shop leaves the strip");
    }

    #[test]
    fn esc_and_the_cross_collapse_and_a_click_on_the_strip_expands() {
        let mut s = in_game();
        s.apply(Input::TogglePressed);
        s.apply(Input::EscPressed);
        assert_eq!(s.mode(), Mode::Compact);
        s.apply(Input::Expand);
        s.apply(Input::Collapse);
        assert_eq!(s.mode(), Mode::Compact);
        s.apply(Input::Expand);
        assert_eq!(s.mode(), Mode::Expanded);
    }

    #[test]
    fn nothing_shows_when_another_window_is_focused() {
        let mut s = in_game();
        s.apply(Input::TogglePressed);
        s.apply(Input::Focus(false));
        assert_eq!(s.mode(), Mode::Hidden);
        s.apply(Input::Focus(true));
        assert_eq!(s.mode(), Mode::Expanded, "comes back as it was");
    }

    #[test]
    fn no_strip_outside_a_game_or_without_a_next_item() {
        let mut s = in_game();
        s.view = Some(view(false));
        assert_eq!(s.mode(), Mode::Hidden);
        s.view = Some(OverlayViewDto {
            next: None,
            ..view(true)
        });
        assert_eq!(s.mode(), Mode::Hidden);
        s.apply(Input::TogglePressed);
        assert_eq!(s.mode(), Mode::Expanded, "P still opens the column");
    }

    #[test]
    fn the_strip_shows_starting_items_or_boots_alone_too() {
        let mut s = in_game();
        let starting = ipc_types::StartingItemsDto {
            items: vec![],
            cost: 500,
            affordable: true,
        };
        s.view = Some(OverlayViewDto {
            next: None,
            starting: Some(starting),
            ..view(true)
        });
        assert_eq!(s.mode(), Mode::Compact);
        let boots = view(true).next;
        s.view = Some(OverlayViewDto {
            next: None,
            boots,
            ..view(true)
        });
        assert_eq!(s.mode(), Mode::Compact);
    }

    #[test]
    fn clear_forgets_the_build_and_collapses() {
        let mut s = in_game();
        s.apply(Input::TogglePressed);
        s.apply(Input::Message(OverlayMessage::Clear));
        assert_eq!(s.view, None);
        assert!(!s.expanded);
    }

    #[test]
    fn new_views_do_not_change_the_mode() {
        let mut s = in_game();
        s.apply(Input::Message(OverlayMessage::Items(Box::new(view(true)))));
        assert_eq!(s.mode(), Mode::Compact);
    }

    fn timers(timers: Vec<ipc_types::ObjectiveTimerDto>) -> OverlayMessage {
        OverlayMessage::Objectives(Some(ipc_types::ObjectiveTimersDto {
            game_time_ms: 90_000,
            timers,
        }))
    }

    fn dragon() -> ipc_types::ObjectiveTimerDto {
        ipc_types::ObjectiveTimerDto {
            pit: ipc_types::PitDto::Dragon,
            label: "Dragon".into(),
            spawn_at_ms: 300_000,
        }
    }

    #[test]
    fn minimap_timers_show_in_game_and_pass_clicks_through() {
        let mut s = in_game();
        assert_eq!(s.minimap_mode(), MinimapMode::Hidden, "no timers yet");
        s.apply(Input::Message(timers(vec![dragon()])));
        assert_eq!(s.minimap_mode(), MinimapMode::Timers);
        s.apply(Input::Focus(false));
        assert_eq!(s.minimap_mode(), MinimapMode::Hidden);
        s.apply(Input::Focus(true));
        s.apply(Input::Message(timers(vec![])));
        assert_eq!(s.minimap_mode(), MinimapMode::Hidden, "every monster is up");
    }

    #[test]
    fn opening_the_column_lets_the_minimap_frame_be_fitted() {
        let mut s = in_game();
        s.apply(Input::TogglePressed);
        assert_eq!(s.minimap_mode(), MinimapMode::Adjust, "even before timers");
        s.apply(Input::EscPressed);
        assert_eq!(s.minimap_mode(), MinimapMode::Hidden);
    }

    #[test]
    fn timers_outlive_a_cleared_build_and_end_with_the_game() {
        let mut s = in_game();
        s.apply(Input::Message(timers(vec![dragon()])));
        s.apply(Input::Message(OverlayMessage::Clear));
        assert_eq!(s.minimap_mode(), MinimapMode::Timers);
        s.apply(Input::Message(OverlayMessage::Objectives(None)));
        assert_eq!(s.objectives, None);
        assert_eq!(s.minimap_mode(), MinimapMode::Hidden);
    }

    #[test]
    fn holding_a_key_is_one_press() {
        let mut k = KeyEdge::default();
        let presses: Vec<bool> = [false, true, true, true, false, true]
            .into_iter()
            .map(|down| k.pressed(down))
            .collect();
        assert_eq!(presses, [false, true, false, false, false, true]);
    }
}
