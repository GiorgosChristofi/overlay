//! The column itself: a transparent, always-on-top window sized to what it
//! shows. Expanded while the shop is open, a small strip while it is closed,
//! and nothing (with clicks passing through) outside the game.

use crate::commands;
use crate::minimap::Placement;
use crate::minimap_view::{self, MinimapLayer};
use crate::state::{Input, Mode, OverlayState};
use eframe::egui::{
    self, Align, Color32, CornerRadius, CursorIcon, Frame, Image, Layout, Margin, Rect, RichText,
    ScrollArea, Sense, Shape, Stroke, Ui, Vec2, ViewportCommand,
};
use ipc_types::{
    OverlayBlockDto, OverlayBlockKind, OverlayCommand, OverlayItemDto, OverlayViewDto,
    RecipeNodeDto, StartingItemsDto,
};
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

// Palette of the main window (`ui/src/styles/tokens.css`).
pub(crate) const BG: Color32 = Color32::from_rgb(0x13, 0x22, 0x2a);
const SURFACE: Color32 = Color32::from_rgb(0x1b, 0x2f, 0x38);
const LINE: Color32 = Color32::from_rgb(0x2a, 0x43, 0x50);
pub(crate) const TEXT: Color32 = Color32::from_rgb(0xe4, 0xed, 0xef);
pub(crate) const MUTED: Color32 = Color32::from_rgb(0x8b, 0xa3, 0xab);
pub(crate) const ACCENT: Color32 = Color32::from_rgb(0xa5, 0x8c, 0xff);
const GOOD: Color32 = Color32::from_rgb(0x6f, 0xd3, 0xa0);
const GOLD: Color32 = Color32::from_rgb(0xf0, 0xb8, 0x66);
const BAD: Color32 = Color32::from_rgb(0xf0, 0x7a, 0x6e);

pub const WIDTH: f32 = 270.0;
/// Tallest the expanded column gets; the item list scrolls beyond that.
pub const MAX_HEIGHT: f32 = 700.0;
/// Where the column first appears: the main monitor's left edge, clear of
/// the shop. Windows otherwise opens it at a default spot.
const FIRST_POSITION: egui::Pos2 = egui::pos2(40.0, 140.0);
const PLACEMENT_WINDOW: Duration = Duration::from_secs(2);

pub struct OverlayApp {
    state: OverlayState,
    inputs: Receiver<Input>,
    /// Last mouse-passthrough state sent to the window.
    passthrough: Option<bool>,
    /// No position remembered yet: move the window to [`FIRST_POSITION`],
    /// giving up at this deadline.
    place_until: Option<Instant>,
    /// Objective timers over the game's minimap.
    minimap: MinimapLayer,
}

/// How often running timers are redrawn: often enough that a second never
/// shows late, rarely enough to cost nothing in game.
const TIMER_REPAINT: Duration = Duration::from_millis(250);

impl OverlayApp {
    pub fn new(
        inputs: Receiver<Input>,
        open: bool,
        place_window: bool,
        minimap: Option<Placement>,
    ) -> Self {
        Self {
            state: OverlayState {
                expanded: open,
                focused: open,
                view: None,
                objectives: None,
            },
            inputs,
            passthrough: None,
            place_until: place_window.then(|| Instant::now() + PLACEMENT_WINDOW),
            minimap: MinimapLayer::new(minimap),
        }
    }
}

/// `1234` -> `1,234`.
pub fn thousands(n: u32) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn block_title(kind: OverlayBlockKind, title: &str) -> String {
    match kind {
        // Starting items are their own early-game section.
        OverlayBlockKind::Starter => "START OF GAME".into(),
        _ => title.to_uppercase(),
    }
}

/// `owned` items are dimmed; `ring` draws a coloured outline (affordable).
fn icon(ui: &mut Ui, item: &OverlayItemDto, size: f32, owned: bool, ring: Option<Color32>) {
    let rect = match &item.icon_url {
        Some(url) => {
            let tint = if owned {
                Color32::from_white_alpha(80)
            } else {
                Color32::WHITE
            };
            ui.add(
                Image::new(url.as_str())
                    .fit_to_exact_size(Vec2::splat(size))
                    .corner_radius(4)
                    .tint(tint),
            )
            .rect
        }
        None => {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
            ui.painter().rect_filled(rect, 4, LINE);
            rect
        }
    };
    if let Some(colour) = ring {
        ui.painter().rect_stroke(
            rect.expand(1.0),
            5,
            Stroke::new(1.5_f32, colour),
            egui::StrokeKind::Outside,
        );
    }
}

fn gold_text(node: &RecipeNodeDto, suffix: &str) -> RichText {
    let colour = if node.affordable { GOOD } else { MUTED };
    RichText::new(format!("{} g{suffix}", thousands(node.remaining_cost)))
        .color(colour)
        .strong()
}

fn starting_gold(starting: &StartingItemsDto) -> RichText {
    let colour = if starting.affordable { GOOD } else { MUTED };
    RichText::new(format!("{} g", thousands(starting.cost)))
        .color(colour)
        .strong()
}

/// A panel with the column's background and border.
fn panel(fill: Color32, stroke: Color32) -> Frame {
    Frame::new()
        .fill(fill)
        .stroke(Stroke::new(1.0_f32, stroke))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::same(10))
}

/// A highlighted box inside the column (starting items, boots, next item).
fn slot(highlight: Color32) -> Frame {
    Frame::new()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0_f32, highlight))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::same(8))
}

fn slot_title(ui: &mut Ui, title: &str) {
    ui.label(RichText::new(title).color(ACCENT).size(10.5).strong());
}

impl OverlayApp {
    // ---------- compact strip (shop closed) ----------

    /// One strip line: item, gold left, and its top-level components.
    fn compact_line(ui: &mut Ui, node: &RecipeNodeDto, size: f32) {
        ui.horizontal(|ui| {
            icon(ui, &node.item, size, false, node.affordable.then_some(GOOD));
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(&node.item.name)
                        .color(TEXT)
                        .strong()
                        .size(13.0),
                );
                ui.label(gold_text(node, " left").size(11.5));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                // Top-level components, last first (right to left).
                for c in node.components.iter().rev() {
                    let ring = (c.affordable && !c.owned).then_some(GOOD);
                    icon(ui, &c.item, 20.0, c.owned, ring);
                }
            });
        });
    }

    /// Starting items while the game starts; afterwards the next item and,
    /// until bought, the boots.
    fn compact(&mut self, ui: &mut Ui, view: &OverlayViewDto) -> Rect {
        let affordable = view.starting.as_ref().map_or_else(
            || {
                view.next
                    .as_ref()
                    .or(view.boots.as_ref())
                    .is_some_and(|n| n.affordable)
            },
            |s| s.affordable,
        );
        let response = panel(BG, if affordable { GOOD } else { LINE })
            .show(ui, |ui| {
                ui.set_width(WIDTH - 22.0);
                if let Some(starting) = &view.starting {
                    ui.horizontal(|ui| {
                        for item in &starting.items {
                            icon(ui, item, 26.0, false, None);
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(starting_gold(starting).size(12.0));
                            ui.label(RichText::new("Start with").color(MUTED).size(11.0));
                        });
                    });
                    return;
                }
                if let Some(next) = &view.next {
                    Self::compact_line(ui, next, 30.0);
                }
                if let Some(boots) = &view.boots {
                    Self::compact_line(ui, boots, 24.0);
                }
            })
            .response;
        let click = ui
            .interact(response.rect, ui.id().with("compact"), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_text("Show the whole build");
        if click.clicked() {
            self.state.apply(Input::Expand);
        }
        response.rect
    }

    // ---------- expanded column (shop open) ----------

    fn header(&mut self, ui: &mut Ui, view: Option<&OverlayViewDto>) {
        let (title, sub) =
            view.map_or(("LoL Overlay".to_owned(), "No build yet".to_owned()), |v| {
                (
                    format!("{} · {}", v.champion_name, v.role),
                    v.source.clone(),
                )
            });
        let gold = view.and_then(|v| v.gold);
        let mut collapse = false;
        let mut over_button = false;
        let response = ui
            .horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new(title).color(TEXT).strong().size(15.0));
                    ui.label(RichText::new(sub).color(MUTED).size(10.5));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let button = ui.add(
                        egui::Button::new(RichText::new("–").color(MUTED).size(18.0)).frame(false),
                    );
                    over_button = button.hovered();
                    collapse = button
                        .on_hover_text("Minimize (Esc or P also work)")
                        .clicked();
                    if let Some(gold) = gold {
                        ui.label(
                            RichText::new(format!("{} g", thousands(gold)))
                                .color(GOLD)
                                .strong(),
                        );
                    }
                });
            })
            .response;
        if collapse {
            self.state.apply(Input::Collapse);
        }
        // The header is the drag handle. Hand the drag to Windows on the
        // press itself, like a title bar: waiting for egui's drag threshold
        // makes the first drag (which also activates the window) lag.
        let over_header = ui
            .input(|i| i.pointer.hover_pos())
            .is_some_and(|p| response.rect.contains(p))
            && !over_button;
        if over_header {
            ui.set_cursor_icon(CursorIcon::Grab);
            if ui.input(|i| i.pointer.primary_pressed()) {
                ui.send_viewport_cmd(ViewportCommand::StartDrag);
            }
        }
    }

    fn recipe_row(ui: &mut Ui, node: &RecipeNodeDto, depth: usize) {
        ui.horizontal(|ui| {
            ui.add_space(14.0 * depth as f32);
            let size = if depth == 0 { 34.0 } else { 24.0 };
            let ring = (depth > 0 && node.affordable && !node.owned).then_some(GOOD);
            icon(ui, &node.item, size, node.owned, ring);
            let name = RichText::new(&node.item.name).size(if depth == 0 { 13.5 } else { 12.0 });
            ui.label(if node.owned {
                name.color(MUTED).strikethrough()
            } else if depth == 0 {
                name.color(TEXT).strong()
            } else {
                name.color(TEXT)
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if node.owned {
                    ui.label(RichText::new("owned").color(MUTED).size(11.0));
                } else {
                    let suffix = if depth == 0 { " left" } else { "" };
                    ui.label(gold_text(node, suffix).size(11.5));
                }
            });
        });
        if !node.owned {
            for c in &node.components {
                Self::recipe_row(ui, c, depth + 1);
            }
        }
    }

    /// An item broken into its recipe, in a highlighted box.
    fn recipe_slot(ui: &mut Ui, node: &RecipeNodeDto, title: &str, pinned: bool) {
        slot(if node.affordable { GOOD } else { ACCENT }).show(ui, |ui| {
            ui.horizontal(|ui| {
                slot_title(ui, title);
                if pinned {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let auto = ui.add(
                            egui::Button::new(RichText::new("automatic").color(MUTED).size(10.5))
                                .frame(false),
                        );
                        if auto
                            .on_hover_text("Go back to the build's next item")
                            .clicked()
                        {
                            commands::send(OverlayCommand::ClearNext);
                        }
                    });
                }
            });
            Self::recipe_row(ui, node, 0);
        });
    }

    fn next_slot(ui: &mut Ui, next: &RecipeNodeDto, pinned: bool) {
        let title = if pinned {
            "NEXT ITEM · YOUR PICK"
        } else {
            "NEXT ITEM"
        };
        Self::recipe_slot(ui, next, title, pinned);
    }

    fn starting_slot(ui: &mut Ui, starting: &StartingItemsDto) {
        slot(if starting.affordable { GOOD } else { ACCENT }).show(ui, |ui| {
            ui.horizontal(|ui| {
                slot_title(ui, "START WITH");
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(starting_gold(starting).size(11.5));
                });
            });
            for item in &starting.items {
                Self::item_row(ui, item, false);
            }
        });
    }

    /// One item: click to build it next, × to take it off the list.
    fn item_row(ui: &mut Ui, item: &OverlayItemDto, is_next: bool) {
        // Reserve the background so it can be painted under the row once
        // the row's size is known.
        let background = ui.painter().add(Shape::Noop);
        let mut remove_rect = Rect::NOTHING;
        let row = ui.horizontal(|ui| {
            icon(ui, item, 28.0, false, is_next.then_some(ACCENT));
            let mut name = item.name.clone();
            if item.count > 1 {
                name.push_str(&format!(" ×{}", item.count));
            }
            ui.label(RichText::new(name).color(TEXT).size(12.5));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                remove_rect = ui
                    .add(
                        egui::Label::new(RichText::new("×").color(MUTED).size(15.0))
                            .selectable(false),
                    )
                    .rect
                    .expand(3.0);
                if let Some(rate) = item.pick_rate {
                    ui.label(
                        RichText::new(format!("{:.0}%", rate * 100.0))
                            .color(ACCENT)
                            .size(11.0),
                    );
                }
                if item.cost > 0 {
                    ui.label(RichText::new(thousands(item.cost)).color(MUTED).size(11.0));
                }
            });
        });
        let rect = row.response.rect.expand2(Vec2::new(4.0, 1.0));
        let response = ui.interact(rect, ui.id().with(("item", item.id)), Sense::click());
        let over_remove = ui
            .input(|i| i.pointer.hover_pos())
            .is_some_and(|p| remove_rect.contains(p));
        if response.hovered() {
            let fill = if over_remove {
                BAD.gamma_multiply(0.18)
            } else {
                SURFACE
            };
            ui.painter()
                .set(background, Shape::rect_filled(rect, 4, fill));
        }
        let hint = if over_remove {
            "Remove from the list"
        } else if is_next {
            "Building this next (click to undo)"
        } else {
            "Build this next"
        };
        let response = response
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_text(hint);
        if response.clicked() {
            let item_id = item.id;
            commands::send(if over_remove {
                OverlayCommand::Remove { item_id }
            } else if is_next {
                OverlayCommand::ClearNext
            } else {
                OverlayCommand::SetNext { item_id }
            });
        }
    }

    fn block(ui: &mut Ui, block: &OverlayBlockDto, next_id: Option<i32>, pinned: bool) {
        ui.add_space(6.0);
        ui.label(
            RichText::new(block_title(block.kind, &block.title))
                .color(MUTED)
                .size(10.5)
                .strong(),
        );
        for item in &block.items {
            Self::item_row(ui, item, pinned && next_id == Some(item.id));
        }
    }

    /// What to buy now: the starting items at the start of the game, then
    /// the boots (until bought) and the next item.
    fn buy_now(ui: &mut Ui, view: &OverlayViewDto) {
        if let Some(starting) = &view.starting {
            Self::starting_slot(ui, starting);
            // An item the player chose still shows during the start.
            if let (true, Some(next)) = (view.next_pinned, &view.next) {
                ui.add_space(6.0);
                Self::next_slot(ui, next, true);
            }
            return;
        }
        if let Some(boots) = &view.boots {
            Self::recipe_slot(ui, boots, "BOOTS", false);
            ui.add_space(6.0);
        }
        if let Some(next) = &view.next {
            Self::next_slot(ui, next, view.next_pinned);
        }
    }

    fn expanded(&mut self, ui: &mut Ui) -> Rect {
        let view = self.state.view.clone();
        panel(BG, LINE)
            .show(ui, |ui| {
                ui.set_width(WIDTH - 22.0);
                self.header(ui, view.as_ref());
                ui.separator();
                let Some(view) = view else {
                    ui.label(
                        RichText::new("Pick a champion and the build shows up here.").color(MUTED),
                    );
                    return;
                };
                ScrollArea::vertical()
                    .max_height(MAX_HEIGHT - 80.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        Self::buy_now(ui, &view);
                        let next_id = view.next.as_ref().map(|n| n.item.id);
                        for block in &view.blocks {
                            Self::block(ui, block, next_id, view.next_pinned);
                        }
                        let nothing_left = view.starting.is_none()
                            && view.next.is_none()
                            && view.boots.is_none()
                            && view.blocks.is_empty();
                        if nothing_left {
                            ui.label(
                                RichText::new("Everything in the build is bought.").color(MUTED),
                            );
                        }
                        if view.removed_count > 0 {
                            ui.add_space(6.0);
                            let text = format!(
                                "Restore {} removed item{}",
                                view.removed_count,
                                if view.removed_count == 1 { "" } else { "s" }
                            );
                            let restore = ui.add(
                                egui::Button::new(RichText::new(text).color(MUTED).size(11.0))
                                    .frame(false),
                            );
                            if restore.clicked() {
                                commands::send(OverlayCommand::RestoreRemoved);
                            }
                        }
                    });
            })
            .response
            .rect
    }

    fn place_window(&mut self, ui: &Ui) {
        let Some(until) = self.place_until else {
            return;
        };
        if Instant::now() >= until {
            self.place_until = None;
            return;
        }
        let at = ui.input(|i| i.viewport().outer_rect.map(|r| r.min));
        if at.is_some_and(|p| p.distance(FIRST_POSITION) < 2.0) {
            // In place; from now on the player decides where it goes.
            self.place_until = None;
        } else {
            ui.send_viewport_cmd(ViewportCommand::OuterPosition(FIRST_POSITION));
            ui.request_repaint_after(Duration::from_millis(50));
        }
    }
}

impl eframe::App for OverlayApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0; 4] // fully transparent window
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if let Some(placement) = self.minimap.placement {
            storage.set_string(minimap_view::STORAGE_KEY, placement.to_storage());
        }
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.place_window(ui);
        while let Ok(input) = self.inputs.try_recv() {
            if input == Input::Quit {
                ui.send_viewport_cmd(ViewportCommand::Close);
            }
            self.state.apply(input);
        }

        let mode = self.state.mode();
        // Never hidden at the OS level: showing a hidden window can take
        // focus from the game. Hidden means nothing drawn, clicks pass.
        let passthrough = mode == Mode::Hidden;
        if self.passthrough != Some(passthrough) {
            ui.send_viewport_cmd(ViewportCommand::MousePassthrough(passthrough));
            self.passthrough = Some(passthrough);
        }

        // Lay out at full height whatever the window's size, so the content
        // is measured whole and the window is set to it in one step (sizing
        // to what fits the current window grows it a bit per frame: a slow
        // slide). Anything past the window's edge for that one frame is
        // simply not shown.
        let full = Rect::from_min_size(ui.max_rect().min, Vec2::new(WIDTH, MAX_HEIGHT));
        let drawn = ui
            .scope_builder(egui::UiBuilder::new().max_rect(full), |ui| {
                match (mode, self.state.view.clone()) {
                    (Mode::Hidden, _) | (Mode::Compact, None) => Rect::NOTHING,
                    (Mode::Compact, Some(view)) => self.compact(ui, &view),
                    (Mode::Expanded, _) => self.expanded(ui),
                }
            })
            .inner;

        // Fit the window to what is drawn, so the transparent rest never
        // blocks clicks meant for the game.
        if drawn.is_positive() {
            let height = (drawn.height() + 2.0).min(MAX_HEIGHT);
            // Compared with the window's real size, not what was last asked.
            let current = ui.input(|i| i.viewport().inner_rect.map(|r| r.height()));
            if current.is_none_or(|h| (h - height).abs() > 1.0) {
                ui.send_viewport_cmd(ViewportCommand::InnerSize(Vec2::new(WIDTH, height)));
            }
        }

        let shown = self
            .state
            .objectives
            .as_ref()
            .map(|t| t.shown(Instant::now()))
            .unwrap_or_default();
        let monitor = ui.input(|i| i.viewport().monitor_size);
        let ctx = ui.ctx().clone();
        self.minimap
            .show(&ctx, self.state.minimap_mode(), &shown, monitor);
        if !shown.is_empty() {
            ctx.request_repaint_after(TIMER_REPAINT);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gold_reads_like_the_shop() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(950), "950");
        assert_eq!(thousands(1150), "1,150");
        assert_eq!(thousands(1_234_567), "1,234,567");
    }

    #[test]
    fn starting_items_get_their_own_heading() {
        assert_eq!(
            block_title(OverlayBlockKind::Starter, "Starting items · 51% win rate"),
            "START OF GAME"
        );
        assert_eq!(
            block_title(OverlayBlockKind::Core, "Core build"),
            "CORE BUILD"
        );
    }
}
