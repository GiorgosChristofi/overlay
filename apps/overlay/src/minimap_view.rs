//! The minimap layer: a second transparent, always-on-top window over the
//! game's minimap that draws the objective timers on their pits.
//!
//! It always passes clicks through to the game, except while the column is
//! open: then it shows a frame the player drags and resizes (bottom-right
//! corner) to fit the minimap, with markers on the two pits to line up.

use crate::app::{ACCENT, BG, MUTED, TEXT};
use crate::minimap::{pit_position, Placement, Shown};
use crate::state::MinimapMode;
use eframe::egui::{
    self, Align2, Color32, Context, CornerRadius, CursorIcon, FontId, Pos2, Rect, ResizeDirection,
    Sense, Stroke, StrokeKind, Ui, Vec2, ViewportBuilder, ViewportCommand, ViewportId,
};
use ipc_types::PitDto;

/// Also how a screenshot script finds the window.
pub const TITLE: &str = "LoL Overlay timers";
/// eframe storage key for the fitted placement.
pub const STORAGE_KEY: &str = "minimap";
/// Grab area of the resize corner, in points.
const HANDLE: f32 = 18.0;

#[derive(Debug, Default)]
pub struct MinimapLayer {
    /// Where the window was created; never changed afterwards, so egui never
    /// moves the window back while the player drags it.
    created_at: Option<Placement>,
    /// Where it is now, saved for next time.
    pub placement: Option<Placement>,
    passthrough: Option<bool>,
}

impl MinimapLayer {
    pub fn new(saved: Option<Placement>) -> Self {
        Self {
            placement: saved,
            ..Self::default()
        }
    }

    /// Draws the layer for this frame. `monitor` is the main monitor's size,
    /// used for a first guess when nothing was saved.
    pub fn show(
        &mut self,
        ctx: &Context,
        mode: MinimapMode,
        shown: &[Shown],
        monitor: Option<Vec2>,
    ) {
        // Not created until the monitor is known when nothing was saved: the
        // first guess depends on it, and a created window is never moved by us.
        let Some(current) = self
            .placement
            .or_else(|| monitor.map(|m| Placement::default_for(m.x, m.y)))
        else {
            return;
        };
        self.placement = Some(current);
        let start = *self.created_at.get_or_insert(current);
        let builder = ViewportBuilder::default()
            .with_title(TITLE)
            .with_position([start.x, start.y])
            .with_inner_size([start.side, start.side])
            .with_decorations(false)
            .with_transparent(true)
            .with_resizable(true)
            .with_always_on_top()
            .with_taskbar(false)
            // Like the column: appearing must not take focus from the game.
            .with_active(false)
            .with_mouse_passthrough(true);
        ctx.show_viewport_immediate(ViewportId::from_hash_of(TITLE), builder, |ui, _class| {
            self.ui(ui, mode, shown)
        });
    }

    fn ui(&mut self, ui: &mut Ui, mode: MinimapMode, shown: &[Shown]) {
        let passthrough = mode != MinimapMode::Adjust;
        if self.passthrough != Some(passthrough) {
            ui.send_viewport_cmd(ViewportCommand::MousePassthrough(passthrough));
            self.passthrough = Some(passthrough);
        }
        let side = ui.max_rect().width().min(ui.max_rect().height());
        let map = Rect::from_min_size(ui.max_rect().min, Vec2::splat(side));
        match mode {
            MinimapMode::Hidden => return,
            MinimapMode::Timers => {}
            MinimapMode::Adjust => self.adjust(ui, map),
        }
        for timer in shown {
            draw_timer(ui, map, timer);
        }
    }

    /// The fitting frame: drag anywhere to move, the corner to resize.
    fn adjust(&mut self, ui: &mut Ui, map: Rect) {
        let painter = ui.painter();
        painter.rect_filled(map, CornerRadius::ZERO, BG.gamma_multiply(0.35));
        painter.rect_stroke(
            map.shrink(1.0),
            CornerRadius::ZERO,
            Stroke::new(2.0, ACCENT),
            StrokeKind::Inside,
        );
        painter.text(
            map.left_top() + Vec2::new(8.0, 6.0),
            Align2::LEFT_TOP,
            "Drag to fit the minimap\nPut the circles on the pits",
            FontId::proportional(11.0),
            TEXT,
        );
        for pit in [PitDto::Baron, PitDto::Dragon] {
            let at = at_pit(map, pit);
            painter.circle_stroke(at, 9.0, Stroke::new(2.0, ACCENT));
            painter.line_segment(
                [at - Vec2::X * 4.0, at + Vec2::X * 4.0],
                Stroke::new(1.5, ACCENT),
            );
            painter.line_segment(
                [at - Vec2::Y * 4.0, at + Vec2::Y * 4.0],
                Stroke::new(1.5, ACCENT),
            );
        }
        let handle =
            Rect::from_min_max(map.right_bottom() - Vec2::splat(HANDLE), map.right_bottom());
        painter.add(egui::Shape::convex_polygon(
            vec![
                handle.right_top(),
                handle.right_bottom(),
                handle.left_bottom(),
            ],
            ACCENT,
            Stroke::NONE,
        ));

        // Like the column's header: hand the drag to Windows on the press.
        let pointer = ui.input(|i| i.pointer.hover_pos());
        let pressed = ui.input(|i| i.pointer.primary_pressed());
        let on_handle = pointer.is_some_and(|p| handle.contains(p));
        let on_map = pointer.is_some_and(|p| map.contains(p));
        if on_handle {
            ui.set_cursor_icon(CursorIcon::ResizeNwSe);
            if pressed {
                ui.send_viewport_cmd(ViewportCommand::BeginResize(ResizeDirection::SouthEast));
            }
        } else if on_map {
            ui.set_cursor_icon(CursorIcon::Grab);
            if pressed {
                ui.send_viewport_cmd(ViewportCommand::StartDrag);
            }
        }
        // Keep the interaction alive for egui (no widget owns this area).
        let _ = ui.interact(map, ui.id().with("minimap-frame"), Sense::click_and_drag());

        self.remember(ui);
    }

    /// Reads back where the player put the window, and keeps it square.
    fn remember(&mut self, ui: &Ui) {
        let (outer, inner, dragging) = ui.input(|i| {
            (
                i.viewport().outer_rect,
                i.viewport().inner_rect,
                i.pointer.any_down(),
            )
        });
        let (Some(outer), Some(inner)) = (outer, inner) else {
            return;
        };
        let placed = Placement::squared(outer.min.x, outer.min.y, inner.width(), inner.height());
        self.placement = Some(placed);
        let square = (inner.width() - inner.height()).abs() < 1.0;
        if !dragging && !square {
            ui.send_viewport_cmd(ViewportCommand::InnerSize(Vec2::splat(placed.side)));
        }
    }
}

fn at_pit(map: Rect, pit: PitDto) -> Pos2 {
    let (fx, fy) = pit_position(pit);
    map.min + Vec2::new(fx * map.width(), fy * map.height())
}

/// A small label centred on the pit: what spawns, and in how long.
fn draw_timer(ui: &Ui, map: Rect, timer: &Shown) {
    let painter = ui.painter();
    let at = at_pit(map, timer.pit);
    // Scale with the minimap so a large one is not cluttered with tiny text.
    let scale = (map.width() / 280.0).clamp(0.8, 1.6);
    let label = painter.layout_no_wrap(
        timer.label.clone(),
        FontId::proportional(9.5 * scale),
        MUTED,
    );
    let time = painter.layout_no_wrap(timer.time.clone(), FontId::monospace(13.0 * scale), TEXT);
    let width = label.size().x.max(time.size().x) + 8.0 * scale;
    let height = label.size().y + time.size().y + 2.0 * scale;
    let bg = Rect::from_center_size(at, Vec2::new(width, height));
    painter.rect_filled(bg, CornerRadius::same(4), Color32::from_black_alpha(190));
    painter.rect_stroke(
        bg,
        CornerRadius::same(4),
        Stroke::new(1.0, ACCENT.gamma_multiply(0.6)),
        StrokeKind::Inside,
    );
    let top = bg.min.y + scale;
    painter.galley(
        Pos2::new(at.x - label.size().x / 2.0, top),
        label.clone(),
        MUTED,
    );
    painter.galley(
        Pos2::new(at.x - time.size().x / 2.0, top + label.size().y),
        time,
        TEXT,
    );
}
