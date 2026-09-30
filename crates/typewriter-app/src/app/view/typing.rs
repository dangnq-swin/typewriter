//! The typing view, in the steps every edition goes through: behind the
//! sheets, the sheets and their feed, over them, the platen's marks, the
//! desk icons; then what fades in calm: the scale, the knobs, the plates,
//! and what they answer. The [`Stage`](crate::Stage) draws its own at each
//! step. And the copy holder beside it.

use eframe::egui::{self, Painter, Pos2, Rect, pos2, vec2};
use typewriter_core::Side;
use typewriter_core::page::Page;

use crate::app::TypewriterApp;
use crate::app::intent::Intent;
use crate::filing::{Keeping, WriteStatus};
use crate::render::calm::{self, Dimming};
use crate::render::folder::{Answer, Flight};
use crate::render::platen::Layout;
use crate::render::{self, feed, folder, holder, knob, notebook, paper, platen, ruler};
use crate::stage::{Controls, Scene};

/// Room past the window edge for a moving sheet's shadow.
const SHADOW_ROOM: f32 = 30.0;

/// The sheets as drawn, for the steps after.
struct Sheets {
    /// Paper rolled through the platen: what turns the knobs.
    knob_rolled: f32,
    pointer_opacity: f32,
    /// A filed sheet on its way into the folder, or just in.
    flight: Option<Flight>,
    answer: Answer,
}

/// What fades in calm, as drawn: for its clicks and drags.
struct Chrome {
    scale: ruler::Scale,
    knobs: [knob::Knob; 2],
    /// The plates, and what the save plate reads. `None`: the stage has its
    /// own controls.
    plates: Option<([Rect; 5], Keeping)>,
}

impl TypewriterApp {
    pub(super) fn show_typing(&mut self, ui: &mut egui::Ui, now: f64, intents: &mut Vec<Intent>) {
        self.knobs.clear();
        let view = ui.max_rect();
        let layout = self.lay_out(view, now);
        let calm = ui.ctx().animate_bool_with_time(
            egui::Id::new("calm-mode"),
            self.desk.calm,
            calm::FADE_SECONDS,
        );
        let chrome = 1.0 - calm;
        let scene = Scene {
            view,
            metrics: &self.metrics,
            typing_y: layout.strike_point.y,
            carriage_x: layout.paper_origin.x + self.metrics.paper_size.x / 2.0,
            last_return: self.desk.last_return,
            now,
        };
        let painter = ui.painter_at(view);
        let sheet_painter = self.paint_behind_sheets(&painter, &scene);
        let sheets = self.paint_sheets(ui, &sheet_painter, &scene, &layout, calm);
        let own_controls = self.show_over_sheets(ui, &painter, &scene, intents);
        self.paint_platen_marks(&painter, &layout, sheets.pointer_opacity, now);
        self.show_desk_icons(ui, view, &sheets, chrome, now, intents);
        if chrome <= 0.0 {
            return;
        }
        let mut painter = painter;
        painter.multiply_opacity(chrome);
        let drawn = self.paint_chrome(ui, &painter, &scene, &layout, chrome, &sheets, own_controls);
        // React only when fully shown, not mid-fade.
        if chrome < 1.0 {
            return;
        }
        if let Some((rects, keeping)) = &drawn.plates {
            self.control_buttons(ui, *rects, keeping, intents);
        }
        if !self.desk.is_busy(now) {
            self.margin_stops(ui, &drawn.scale, intents);
            for (name, knob) in ["left", "right"].into_iter().zip(&drawn.knobs) {
                self.platen_knob(ui, name, knob.grip(), intents);
            }
        }
    }

    /// The platen's layout this frame, its typing line where the stage has
    /// it.
    fn lay_out(&mut self, view: Rect, now: f64) -> Layout {
        let zoom = self.desk.zoom_percent;
        if let Some(height) = self.stage.typing_line_height(view, &self.metrics, zoom) {
            self.platen.typing_line_height = height;
        }
        let carriage = self.desk.project.machine.carriage();
        let cell = self
            .metrics
            .cell_offset(carriage.half_line, carriage.column);
        self.platen.layout(view, &self.metrics, cell, now)
    }

    /// Behind the sheets: the stage's. Returns the sheets' painter, cut off
    /// where they go out of sight.
    fn paint_behind_sheets(&self, painter: &Painter, scene: &Scene) -> Painter {
        match self.stage.paint_behind_sheets(painter, scene) {
            Some(bottom) => {
                let view = scene.view;
                painter.with_clip_rect(Rect::from_min_max(view.min, pos2(view.max.x, bottom)))
            }
            None => painter.clone(),
        }
    }

    /// The sheets: a finished one rolling out, or flying into the folder,
    /// and the next winding in; else the one in the machine.
    fn paint_sheets(
        &self,
        ui: &egui::Ui,
        painter: &Painter,
        scene: &Scene,
        layout: &Layout,
        calm: f32,
    ) -> Sheets {
        let (view, now) = (scene.view, scene.now);
        let machine = &self.desk.project.machine;
        let carriage = machine.carriage();
        let cell = self
            .metrics
            .cell_offset(carriage.half_line, carriage.column);
        let mut paper_origin = layout.paper_origin;
        let mut pointer_opacity = 1.0;
        let mut knob_rolled = layout.strike_point.y - layout.paper_origin.y;
        let flight = self.desk.feed.flight.filter(|f| !f.is_over(now));
        let answer = flight.map_or(folder::Answer::STILL, |f| f.answer(now));
        if let Some(feeding) = &self.desk.feed.feeding {
            let t = now - feeding.started;
            let motion = &feeding.motion;
            // Points the platen has rolled the old sheet out so far, and in all.
            let mut wound_out = (0.0, 0.0);
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
                    wound_out = flight.pulled_out(now, &route);
                    match flight.sheet(now, &route) {
                        // Still rolling out, full size: drawn as in the machine.
                        Some(pose) if pose.mouth.is_none() => {
                            let origin = old_origin + (pose.centre - route.from);
                            self.paint_lifted(painter, view, origin, 0.0, pose.lift);
                            self.paint_page(painter, old_page, origin, dimming, &paper::dry);
                        }
                        Some(pose) => {
                            self.paint_flying(ui.ctx(), view, (old_page, dimming), pose);
                        }
                        None => {}
                    }
                } else {
                    let exit = old_y + self.metrics.paper_size.y - view.top() + SHADOW_ROOM;
                    let rolled = motion.roll_out(t);
                    wound_out = (rolled.unwrap_or(1.0) * exit, exit);
                    if let Some(rolled) = rolled {
                        let old_origin = old_origin - vec2(0.0, rolled * exit);
                        self.paint_lifted(painter, view, old_origin, 0.0, 1.0);
                        self.paint_page(painter, old_page, old_origin, dimming, &paper::dry);
                    }
                }
            }
            // Rise from below the window until the top margin meets the
            // typing line.
            let placed = layout.strike_point.y - cell.y;
            let below = view.bottom() + SHADOW_ROOM;
            paper_origin.y = below + (placed - below) * motion.progress(t);
            // The knob turns with each sheet in turn, but not while a hand
            // puts the new one in.
            let resting = |half_line| self.metrics.cell_offset(half_line, 0).y;
            let rise = below - placed;
            let by_hand = motion.by_hand();
            let wound_in = (motion.progress(t) - by_hand).max(0.0) * rise;
            knob_rolled = knob::feed_roll(
                &self.metrics,
                feeding
                    .outgoing
                    .as_ref()
                    .map(|(_, half_line)| resting(*half_line)),
                layout.strike_point.y - placed,
                wound_out.0 + wound_in,
                wound_out.1 + (1.0 - by_hand) * rise,
            );
            let (curl, lift) = (motion.curl(t), motion.lift(t));
            self.paint_lifted(painter, view, paper_origin, curl, lift);
            pointer_opacity = motion.pointer_opacity(t);
        } else if let Some(lift) = self.stage.sheet_lift() {
            self.paint_lifted(painter, view, paper_origin, 0.0, lift);
        }
        let dimming = self.dimming(carriage.half_line, calm);
        let wetness = |half_line, column| self.desk.project.wetness(now, half_line, column);
        self.paint_page(painter, machine.page(), paper_origin, dimming, &wetness);
        Sheets {
            knob_rolled,
            pointer_opacity,
            flight,
            answer,
        }
    }

    /// Over the sheets, calm or not: the stage's, and its own controls,
    /// which answer at once. False: it has none, the plates stand in.
    fn show_over_sheets(
        &self,
        ui: &egui::Ui,
        painter: &Painter,
        scene: &Scene,
        intents: &mut Vec<Intent>,
    ) -> bool {
        self.stage.paint_over_sheets(painter, scene);
        let (desk, project) = (&self.desk, &self.desk.project);
        let keeping = project
            .filing
            .keeping(desk.settings.saving.autosave, scene.now);
        let goals = desk.settings.goals.cycle();
        let controls = Controls {
            spacing: project.machine.carriage().line_spacing,
            zoom_percent: desk.zoom_percent,
            erase: project.machine.constraints.erase,
            slip_in: project.machine.slip_in(),
            delete_in_cycle: desk.settings.machine.rules.delete_in_cycle,
            goal: project.session.goal(),
            goals: &goals,
            progress: project.session.progress(),
            keeping: &keeping,
        };
        let Some(rects) = self.stage.controls(ui, painter, scene, &controls) else {
            return false;
        };
        self.control_buttons(ui, rects, &keeping, intents);
        true
    }

    /// The platen's marks over the sheet: the correction slip, the guides
    /// while the knob turns, the typing point.
    fn paint_platen_marks(
        &self,
        painter: &Painter,
        layout: &Layout,
        pointer_opacity: f32,
        now: f64,
    ) {
        if self.desk.project.machine.slip_in() {
            platen::paint_slip(painter, &self.metrics, layout.strike_point);
        }
        platen::paint_guides(
            painter,
            &self.metrics,
            layout.strike_point,
            layout.paper_origin.x,
            self.desk.guides_opacity(now),
        );
        platen::paint_strike_marker(painter, &self.metrics, layout.strike_point, pointer_opacity);
    }

    /// The folder, notebook, calm and settings icons, faded to `chrome`.
    fn show_desk_icons(
        &self,
        ui: &mut egui::Ui,
        view: Rect,
        sheets: &Sheets,
        chrome: f32,
        now: f64,
        intents: &mut Vec<Intent>,
    ) {
        // One short until the flying sheet is in.
        let finished = self.desk.project.machine.document().finished().len();
        let shown = if sheets.flight.is_some_and(|f| !f.has_landed(now)) {
            finished.saturating_sub(1)
        } else {
            finished
        };
        let reveal = sheets
            .flight
            .filter(|_| self.desk.calm)
            .map_or(0.0, |f| f.reveal(now));
        if folder::desk_icon(ui, view, shown, sheets.answer, chrome.max(reveal)) {
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
    }

    /// What fades in calm: the scale, on its plate or printed on the stage's
    /// machine; the knobs, with what the stage has behind and in front of
    /// them; the plates, unless the stage has `own_controls`.
    #[allow(clippy::too_many_arguments)]
    fn paint_chrome(
        &self,
        ui: &egui::Ui,
        painter: &Painter,
        scene: &Scene,
        layout: &Layout,
        chrome: f32,
        sheets: &Sheets,
        own_controls: bool,
    ) -> Chrome {
        let project = &self.desk.project;
        let machine = &project.machine;
        let carriage = machine.carriage();
        // The plates hang below the typing line, the scale's plate above them.
        let plates_top = ruler::top(&self.metrics, layout.strike_point) + ruler::HEIGHT;
        let printed = self.stage.scale_top(scene);
        let scale_top = printed.unwrap_or(plates_top - ruler::HEIGHT);
        let columns = machine.page().columns();
        let scale = ruler::Scale::new(&self.metrics, columns, layout.paper_origin.x, scale_top);
        if printed.is_some() {
            ruler::paint_scale_marks(painter, &scale, carriage);
        } else {
            ruler::paint_scale(painter, &scale, carriage);
        }
        let knobs = match self.stage.platen_ends(scene) {
            Some([left, right]) => [(Side::Left, left), (Side::Right, right)]
                .map(|(side, edge)| knob::Knob::on_axis(&self.metrics, side, edge, scene.typing_y)),
            None => {
                let paper_left = layout.paper_origin.x;
                [
                    (Side::Left, paper_left),
                    (Side::Right, paper_left + self.metrics.paper_size.x),
                ]
                .map(|(side, edge)| knob::Knob::new(&self.metrics, side, edge, scale_top))
            }
        };
        self.stage.paint_behind_knobs(painter, scene);
        let busy = self.desk.is_busy(scene.now);
        for knob in &knobs {
            let hovered = chrome >= 1.0 && !busy && ui.rect_contains_pointer(knob.grip());
            knob.paint(painter, sheets.knob_rolled, hovered);
        }
        self.stage.paint_over_knobs(painter, scene);
        let plates = (!own_controls).then(|| {
            let spacing_plate = ruler::paint_spacing_indicator(
                painter,
                carriage.line_spacing,
                layout.paper_origin.x,
                plates_top,
            );
            let zoom_plate =
                ruler::paint_zoom_plate(painter, self.desk.zoom_percent, spacing_plate);
            let correction_plate = ruler::paint_correction_plate(
                painter,
                machine.constraints.erase,
                machine.slip_in(),
                zoom_plate,
            );
            let goal_plate =
                ruler::paint_goal_plate(painter, project.session.progress(), correction_plate);
            let keeping = project
                .filing
                .keeping(self.desk.settings.saving.autosave, scene.now);
            let autosave_plate = ruler::paint_autosave_plate(
                painter,
                &keeping,
                layout.paper_origin.x + self.metrics.paper_size.x,
                spacing_plate,
                goal_plate.right(),
            );
            (
                [
                    spacing_plate,
                    zoom_plate,
                    correction_plate,
                    goal_plate,
                    autosave_plate,
                ],
                keeping,
            )
        });
        Chrome {
            scale,
            knobs,
            plates,
        }
    }

    /// Clicks on the spacing, zoom, correction, goal and save controls,
    /// plates or knobs.
    fn control_buttons(
        &self,
        ui: &egui::Ui,
        [spacing, zoom, correction, goal, save]: [Rect; 5],
        keeping: &Keeping,
        intents: &mut Vec<Intent>,
    ) {
        let filing = &self.desk.project.filing;
        let spacing_button = render::button(
            ui,
            spacing,
            "spacing-plate",
            "Line spacing (F1 / F2 / F3). Click for the next notch.",
        );
        if spacing_button.clicked() {
            intents.push(Intent::NextSpacing);
        }
        let zoom_button = render::button(
            ui,
            zoom,
            "zoom-plate",
            "Zoom (mouse wheel). Double-click for 100 %.",
        );
        if zoom_button.double_clicked() {
            intents.push(Intent::ResetZoom);
        }
        let correction_button = render::button(
            ui,
            correction,
            "correction-plate",
            if self.desk.settings.machine.rules.delete_in_cycle {
                "Click (or F4) for the next way: correction paper, eraser, fluid or delete"
            } else {
                "Click (or F4) for the next way: correction paper, eraser or fluid"
            },
        );
        if correction_button.clicked() {
            intents.push(Intent::NextCorrection);
        }
        let goal_button = render::button(
            ui,
            goal,
            "goal-plate",
            "Click for the next: 250, 500 or 1000 words, 15, 25 or 50 minutes of typing, or off",
        );
        if goal_button.clicked() {
            intents.push(Intent::NextGoal);
        }
        let tip = match keeping {
            Keeping::Autosave(WriteStatus::Failed(err)) => {
                format!("Could not save: {err}. Click to try again.")
            }
            Keeping::Autosave(_) => {
                format!("Saved to {}. Click to save now.", filing.location())
            }
            Keeping::Draft => "Kept in the drafts folder. Click to save it as a file.".to_owned(),
            Keeping::Off { unsaved: true } => "Unsaved changes. Click to save.".to_owned(),
            Keeping::Off { unsaved: false } => "Saved. Click to save now.".to_owned(),
        };
        let autosave = render::button(ui, save, "autosave-plate", &tip);
        if autosave.clicked() {
            intents.push(Intent::Save);
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
