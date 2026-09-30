//! The typing view: the sheet in the machine and its feed, the scale, knobs
//! and plates, the desk icons; and the copy holder beside it.

use eframe::egui::{self, Painter, Pos2, Rect, pos2, vec2};
use typewriter_core::Side;
use typewriter_core::page::Page;

use crate::Edition;
use crate::app::TypewriterApp;
use crate::app::intent::Intent;
use crate::filing::{Keeping, WriteStatus};
use crate::render::calm::{self, Dimming};
use crate::render::{self, feed, folder, holder, knob, machine, notebook, paper, platen, ruler};

/// Room past the window edge for a moving sheet's shadow.
const SHADOW_ROOM: f32 = 30.0;
/// The desk edition's sheet in the machine: just off the wall behind.
const STANDING_LIFT: f32 = 0.25;

impl TypewriterApp {
    pub(super) fn show_typing(&mut self, ui: &mut egui::Ui, now: f64, intents: &mut Vec<Intent>) {
        self.knobs.clear();
        let view = ui.max_rect();
        let machine = &self.desk.project.machine;
        let carriage = machine.carriage();
        let cell = self
            .metrics
            .cell_offset(carriage.half_line, carriage.column);
        let layout = self.platen.layout(view, &self.metrics, cell, now);
        let painter = ui.painter_at(view);
        let calm = ui.ctx().animate_bool_with_time(
            egui::Id::new("calm-mode"),
            self.desk.calm,
            calm::FADE_SECONDS,
        );
        let chrome = 1.0 - calm;
        let seated = self.edition == Edition::Desk;
        if seated {
            machine::paint_platen(
                &painter,
                &self.metrics,
                layout.paper_origin.x,
                layout.strike_point.y,
            );
        }
        let mut paper_origin = layout.paper_origin;
        let mut pointer_opacity = 1.0;
        let flight = self.desk.feed.flight.filter(|f| !f.is_over(now));
        let answer = flight.map_or(folder::Answer::STILL, |f| f.answer(now));
        if let Some(feeding) = &self.desk.feed.feeding {
            let t = now - feeding.started;
            let motion = &feeding.motion;
            if let Some((old_page, old_half_line)) = &feeding.outgoing {
                let old_y = layout.strike_point.y - self.metrics.cell_offset(*old_half_line, 0).y;
                let old_origin = pos2(paper_origin.x, old_y);
                let dimming = self.dimming(*old_half_line, calm);
                // Calm mode hides the folder: the sheet just winds out.
                if let Some(flight) = flight.filter(|_| !self.desk.calm) {
                    let size = self.metrics.paper_size;
                    let route = folder::Route {
                        from: Rect::from_min_size(old_origin, size).center(),
                        size,
                        platen_y: layout.strike_point.y,
                        window_top: view.top(),
                        mouth: folder::icon_body(view).center_top() + vec2(0.0, answer.dip),
                    };
                    match flight.sheet(now, &route) {
                        // Still rolling out, full size: drawn as in the machine.
                        Some(pose) if pose.mouth.is_none() => {
                            let origin = old_origin + (pose.centre - route.from);
                            self.paint_lifted(&painter, view, origin, 0.0, pose.lift);
                            self.paint_page(&painter, old_page, origin, dimming, &paper::dry);
                        }
                        Some(pose) => {
                            self.paint_flying(ui.ctx(), view, (old_page, dimming), pose);
                        }
                        None => {}
                    }
                } else if let Some(rolled) = motion.roll_out(t) {
                    let exit = old_y + self.metrics.paper_size.y - view.top() + SHADOW_ROOM;
                    let old_origin = old_origin - vec2(0.0, rolled * exit);
                    self.paint_lifted(&painter, view, old_origin, 0.0, 1.0);
                    self.paint_page(&painter, old_page, old_origin, dimming, &paper::dry);
                }
            }
            // Rise from below the window until the top margin meets the
            // typing line.
            let placed = layout.strike_point.y - cell.y;
            let below = view.bottom() + SHADOW_ROOM;
            paper_origin.y = below + (placed - below) * motion.progress(t);
            self.paint_lifted(&painter, view, paper_origin, motion.curl(t), motion.lift(t));
            pointer_opacity = motion.pointer_opacity(t);
        } else if seated {
            self.paint_lifted(&painter, view, paper_origin, 0.0, STANDING_LIFT);
        }
        let dimming = self.dimming(carriage.half_line, calm);
        let wetness = |half_line, column| self.desk.project.wetness(now, half_line, column);
        self.paint_page(&painter, machine.page(), paper_origin, dimming, &wetness);
        if seated {
            let top = ruler::top(&self.metrics, layout.strike_point) + ruler::HEIGHT;
            machine::paint_body(&painter, view, &self.metrics, top);
        }
        if machine.slip_in() {
            platen::paint_slip(&painter, &self.metrics, layout.strike_point);
        }
        platen::paint_guides(
            &painter,
            &self.metrics,
            layout.strike_point,
            layout.paper_origin.x,
            self.desk.guides_opacity(now),
        );
        platen::paint_strike_marker(
            &painter,
            &self.metrics,
            layout.strike_point,
            pointer_opacity,
        );

        // One short until the flying sheet is in.
        let finished = machine.document().finished().len();
        let shown = if flight.is_some_and(|f| !f.has_landed(now)) {
            finished.saturating_sub(1)
        } else {
            finished
        };
        let reveal = flight
            .filter(|_| self.desk.calm)
            .map_or(0.0, |f| f.reveal(now));
        if folder::desk_icon(ui, view, shown, answer, chrome.max(reveal)) {
            intents.push(Intent::OpenFolder);
        }
        if notebook::desk_icon(ui, view, chrome) {
            intents.push(Intent::OpenNotebook);
        }
        if calm::calm_icon(ui, view) {
            intents.push(Intent::ToggleCalm);
        }
        if render::settings::gear_icon(ui, view, chrome) {
            intents.push(Intent::OpenSettings);
        }
        if chrome <= 0.0 {
            return;
        }
        let mut painter = painter;
        painter.multiply_opacity(chrome);
        let project = &self.desk.project;
        let machine = &project.machine;
        let carriage = machine.carriage();
        let ruler_top = ruler::top(&self.metrics, layout.strike_point);
        let scale = ruler::Scale::new(
            &self.metrics,
            machine.page().columns(),
            layout.paper_origin.x,
            ruler_top,
        );
        ruler::paint_scale(&painter, &scale, carriage);
        let busy = self.desk.is_busy(now);
        let paper_left = layout.paper_origin.x;
        let knobs = [
            (Side::Left, paper_left),
            (Side::Right, paper_left + self.metrics.paper_size.x),
        ]
        .map(|(side, edge)| knob::Knob::new(&self.metrics, side, edge, ruler_top));
        for knob in &knobs {
            let hovered = chrome >= 1.0 && !busy && ui.rect_contains_pointer(knob.grip());
            knob.paint(
                &painter,
                layout.strike_point.y - layout.paper_origin.y,
                hovered,
            );
        }
        let spacing_plate = ruler::paint_spacing_indicator(
            &painter,
            carriage.line_spacing,
            layout.paper_origin.x,
            ruler_top + ruler::HEIGHT,
        );
        let zoom_plate = ruler::paint_zoom_plate(&painter, self.desk.zoom_percent, spacing_plate);
        let correction_plate = ruler::paint_correction_plate(
            &painter,
            machine.constraints.erase,
            machine.slip_in(),
            zoom_plate,
        );
        let goal_plate =
            ruler::paint_goal_plate(&painter, project.session.progress(), correction_plate);
        let keeping = project
            .filing
            .keeping(self.desk.settings.saving.autosave, now);
        let autosave_plate = ruler::paint_autosave_plate(
            &painter,
            &keeping,
            layout.paper_origin.x + self.metrics.paper_size.x,
            spacing_plate,
            goal_plate.right(),
        );
        // React only when fully shown, not mid-fade.
        if chrome < 1.0 {
            return;
        }
        let spacing = render::button(
            ui,
            spacing_plate,
            "spacing-plate",
            "Line spacing (F1 / F2 / F3). Click for the next notch.",
        );
        if spacing.clicked() {
            intents.push(Intent::NextSpacing);
        }
        let zoom = render::button(
            ui,
            zoom_plate,
            "zoom-plate",
            "Zoom (mouse wheel). Double-click for 100 %.",
        );
        if zoom.double_clicked() {
            intents.push(Intent::ResetZoom);
        }
        let correction = render::button(
            ui,
            correction_plate,
            "correction-plate",
            if self.desk.settings.machine.rules.delete_in_cycle {
                "Click (or F4) for the next way: correction paper, eraser, fluid or delete"
            } else {
                "Click (or F4) for the next way: correction paper, eraser or fluid"
            },
        );
        if correction.clicked() {
            intents.push(Intent::NextCorrection);
        }
        let goal = render::button(
            ui,
            goal_plate,
            "goal-plate",
            "Click for the next: 250, 500 or 1000 words, 15, 25 or 50 minutes of typing, or off",
        );
        if goal.clicked() {
            intents.push(Intent::NextGoal);
        }
        let tip = match &keeping {
            Keeping::Autosave(WriteStatus::Failed(err)) => {
                format!("Could not save: {err}. Click to try again.")
            }
            Keeping::Autosave(_) => {
                format!("Saved to {}. Click to save now.", project.filing.location())
            }
            Keeping::Draft => "Kept in the drafts folder. Click to save it as a file.".to_owned(),
            Keeping::Off { unsaved: true } => "Unsaved changes. Click to save.".to_owned(),
            Keeping::Off { unsaved: false } => "Saved. Click to save now.".to_owned(),
        };
        let autosave = render::button(ui, autosave_plate, "autosave-plate", &tip);
        if autosave.clicked() {
            intents.push(Intent::Save);
        }
        if !busy {
            self.margin_stops(ui, &scale, intents);
            for (name, knob) in ["left", "right"].into_iter().zip(&knobs) {
                self.platen_knob(ui, name, knob.grip(), intents);
            }
        }
    }

    /// The copy holder, if a sheet is on it.
    pub(super) fn show_holder(&mut self, ui: &mut egui::Ui, view: Rect, intents: &mut Vec<Intent>) {
        let desk = &mut self.desk;
        let Some(stand) = &mut desk.overlays.holder else {
            return;
        };
        let (profile, ink_realism) = (
            desk.project.machine.profile(),
            desk.settings.look.ink_realism,
        );
        if holder::show(ui, view, profile, stand, ink_realism) {
            intents.push(Intent::TakeHolderDown);
        }
    }

    /// Drag the knob to roll the paper, a half-line a notch.
    fn platen_knob(&mut self, ui: &egui::Ui, name: &str, knob: Rect, intents: &mut Vec<Intent>) {
        self.knobs.push(knob);
        let response = ui
            .interact(
                knob,
                egui::Id::new(("platen-knob", name)),
                egui::Sense::DRAG,
            )
            .on_hover_text("Platen knob (Up / Down): drag or scroll to roll a half-line");
        if response.hovered() || response.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
        }
        if response.dragged() {
            let per_notch = knob::DRAG_POINTS_PER_NOTCH * self.metrics.points_per_inch / 96.0;
            let points = response.drag_delta().y;
            intents.push(Intent::DragKnob { points, per_notch });
        } else if response.drag_stopped() {
            intents.push(Intent::ReleaseKnob);
        }
    }

    /// The scale's margin stops: click to release the margins, drag to move one.
    fn margin_stops(&self, ui: &egui::Ui, scale: &ruler::Scale, intents: &mut Vec<Intent>) {
        let carriage = self.desk.project.machine.carriage();
        let (left, right) = (carriage.left_margin, carriage.right_margin);
        for (side, name, key) in [(Side::Left, "Left", "Home"), (Side::Right, "Right", "End")] {
            let grip = scale.stop(carriage, side);
            let tip = format!(
                "{name} margin (Shift+{key}). Drag to move; click to release the margins (Home)."
            );
            let stop = ui
                .interact(
                    grip,
                    egui::Id::new(("margin-stop", name)),
                    render::CLICK_AND_DRAG,
                )
                .on_hover_text(tip);
            if stop.hovered() || stop.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
            }
            if stop.clicked() {
                intents.push(Intent::ReleaseMargins);
            }
            let Some(pointer) = stop.interact_pointer_pos().filter(|_| stop.dragged()) else {
                continue;
            };
            // Stop short of the other stop: no jolt per frame.
            let column = scale.column_at(pointer.x);
            let (column, margin) = match side {
                Side::Left => (column.min(right - 1), left),
                Side::Right => (column.max(left + 1), right),
            };
            if column != margin {
                intents.push(Intent::MoveMargin { side, column });
            }
        }
    }

    /// Calm dimming around `half_line`, `amount` of the way on.
    fn dimming(&self, half_line: u16, amount: f32) -> Dimming {
        let look = &self.desk.settings.look;
        Dimming::calm(
            half_line,
            amount,
            look.calm_falloff_lines,
            look.calm_minimum_percent,
        )
    }

    fn paint_page(
        &self,
        painter: &Painter,
        page: &Page,
        origin: Pos2,
        dimming: Dimming,
        wetness: paper::Wetness<'_>,
    ) {
        let machine = &self.desk.project.machine;
        paper::paint_margin_frame(
            painter,
            &self.metrics,
            machine.carriage(),
            machine.profile().margins.top_lines,
            origin,
        );
        paper::paint_sheet(
            painter,
            &self.metrics,
            page,
            origin,
            self.desk.settings.look.ink_realism,
            dimming,
            wetness,
        );
    }

    /// A filed sheet in `pose` on its way into the folder icon, drawn at its
    /// shrinking size over the desk. Not on a scaled layer: egui garbles
    /// scaled text.
    fn paint_flying(
        &self,
        ctx: &egui::Context,
        view: Rect,
        (page, dimming): (&Page, Dimming),
        pose: folder::Pose,
    ) {
        let metrics = self.metrics.scaled(pose.scale);
        let origin = pose.centre - 0.5 * metrics.paper_size;
        let layer = egui::LayerId::new(egui::Order::Middle, egui::Id::new("flying-sheet"));
        let mut painter = ctx.layer_painter(layer);
        if let Some(mouth) = pose.mouth {
            painter = painter.with_clip_rect(Rect::from_min_max(view.min, pos2(view.max.x, mouth)));
        }
        feed::paint_lifted_sheet(
            &painter,
            &self.background,
            view,
            Rect::from_min_size(origin, metrics.paper_size),
            metrics.points_per_inch,
            0.0,
            pose.lift,
        );
        // The margin frame is the machine's, not the paper's: it fades as the
        // sheet leaves.
        let machine = &self.desk.project.machine;
        let mut frame = painter.clone();
        frame.multiply_opacity(render::smoothstep((pose.scale - 0.6) / 0.4));
        let top_lines = machine.profile().margins.top_lines;
        paper::paint_margin_frame(&frame, &metrics, machine.carriage(), top_lines, origin);
        let ink_realism = self.desk.settings.look.ink_realism;
        paper::paint_sheet(
            &painter,
            &metrics,
            page,
            origin,
            ink_realism,
            dimming,
            &paper::dry,
        );
    }

    fn paint_lifted(&self, painter: &Painter, view: Rect, origin: Pos2, curl: f32, lift: f32) {
        feed::paint_lifted_sheet(
            painter,
            &self.background,
            view,
            Rect::from_min_size(origin, self.metrics.paper_size),
            self.metrics.points_per_inch,
            curl,
            lift,
        );
    }
}
