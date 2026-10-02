//! The typing view, in the steps every mode goes through: behind the
//! sheets, the sheets and their feed, over them, the platen's marks, the
//! desk icons; then what fades in calm: the scale, the knobs, the plates,
//! and what they answer. The [`Stage`](crate::Stage) draws its own at each
//! step. And the copy holder beside it.

use eframe::egui::epaint::Vertex;
use eframe::egui::{self, Color32, Painter, Pos2, Rect, pos2, vec2};
use typewriter_core::Side;
use typewriter_core::page::Page;

use crate::app::TypewriterApp;
use crate::app::intent::Intent;
use crate::filing::{Keeping, WriteStatus};
use crate::render::calm::{self, Dimming};
use crate::render::folder::{Answer, Flight};
use crate::render::platen::Layout;
use crate::render::{self, feed, folder, holder, knob, notebook, paper, platen, ruler};
use crate::stage::{Controls, FlatSheet, PaperTable, Scene};

/// Room past the window edge for a moving sheet's shadow.
const SHADOW_ROOM: f32 = 30.0;

/// A new sheet's way in, as its top edge's height on screen: put in by hand
/// from `hand_from`, then wound by the knob from `knob_from` to `placed`.
struct WayIn {
    hand_from: f32,
    knob_from: f32,
    placed: f32,
    /// The wind-in's share (0..=1) made by hand.
    by_hand: f32,
}

impl WayIn {
    /// Up from below the window, the hand then the knob moving it on alike.
    fn from_below(below: f32, placed: f32, by_hand: f32) -> Self {
        Self {
            hand_from: below,
            knob_from: egui::lerp(below..=placed, by_hand),
            placed,
            by_hand,
        }
    }

    /// Slid down `table` from out of sight over its top, then wound round
    /// the platen from where it goes in.
    fn round_platen(
        table: &PaperTable,
        typing_y: f32,
        points_per_inch: f32,
        placed: f32,
        by_hand: f32,
    ) -> Self {
        let per_mm = points_per_inch / render::MM_PER_INCH;
        let knob_from = typing_y + table.wrap_mm * per_mm;
        Self {
            hand_from: knob_from + table.seen_mm * per_mm,
            knob_from,
            placed,
            by_hand,
        }
    }

    /// The top edge at `progress` (0..=1) of the wind-in.
    fn at(&self, progress: f32) -> f32 {
        if progress < self.by_hand {
            let share = progress / self.by_hand;
            return egui::lerp(self.hand_from..=self.knob_from, share);
        }
        let rest = 1.0 - self.by_hand;
        let share = if rest > 0.0 {
            ((progress - self.by_hand) / rest).min(1.0)
        } else {
            1.0
        };
        egui::lerp(self.knob_from..=self.placed, share)
    }

    /// Points the knob winds in all.
    fn knob_travel(&self) -> f32 {
        self.knob_from - self.placed
    }

    /// Points the knob has wound at `progress`.
    fn knob_done(&self, progress: f32) -> f32 {
        (self.knob_from - self.at(progress)).max(0.0)
    }
}

/// The sheets as drawn, for the steps after.
struct Sheets {
    /// Paper rolled through the platen: what turns the knobs.
    knob_rolled: f32,
    pointer_opacity: f32,
    /// A filed sheet on its way into the folder, or just in.
    flight: Option<Flight>,
    answer: Answer,
}

/// What the stage draws over the sheets, calm or not: whether it has its
/// own controls, its knobs' grips, and where it printed the scale.
struct Over {
    own_controls: bool,
    knobs: Option<[Rect; 2]>,
    scale: Option<ruler::Scale>,
}

/// What fades in calm, as drawn: for its clicks and drags.
struct Chrome {
    scale: ruler::Scale,
    /// The knobs' grips, the stage's if it drew them.
    knobs: [Rect; 2],
    /// The plates, and what the save plate reads. `None`: the stage has its
    /// own controls.
    plates: Option<([Rect; 5], Keeping)>,
}

impl TypewriterApp {
    pub(super) fn show_typing(&mut self, ui: &mut egui::Ui, now: f64, intents: &mut Vec<Intent>) {
        self.knobs.clear();
        let view = ui.max_rect();
        let layout = self.lay_out(ui.ctx(), view, now);
        let calm = ui.ctx().animate_bool_with_time(
            egui::Id::new("calm-mode"),
            self.model.calm,
            calm::FADE_SECONDS,
        );
        let chrome = 1.0 - calm;
        let scene = Scene {
            view,
            metrics: &self.metrics,
            typing_y: layout.strike_point.y,
            carriage_x: layout.paper_origin.x + self.metrics.paper_size.x / 2.0,
            last_return: self.model.last_return,
            now,
        };
        let painter = ui.painter_at(view);
        let sheet_painter = self.paint_behind_sheets(&painter, &scene);
        let sheets = self.paint_sheets(ui, &sheet_painter, &scene, &layout, calm);
        let active = chrome >= 1.0 && !self.model.is_busy(now);
        let scale = self.lay_out_scale(&layout);
        let on = (&sheets, &scale, active);
        let over = self.show_over_sheets(ui, &painter, &scene, on, intents);
        self.paint_platen_marks(&painter, &layout, sheets.pointer_opacity, now);
        self.show_desk_icons(ui, view, &sheets, chrome, now, intents);
        if chrome <= 0.0 {
            return;
        }
        let mut painter = painter;
        painter.multiply_opacity(chrome);
        let drawn = self.paint_chrome(
            ui,
            &painter,
            &scene,
            &layout,
            chrome,
            &sheets,
            (scale, &over),
        );
        // React only when fully shown, not mid-fade.
        if chrome < 1.0 {
            return;
        }
        if let Some((rects, keeping)) = &drawn.plates {
            self.control_buttons(ui, *rects, keeping, intents);
        }
        if !self.model.is_busy(now) {
            self.margin_stops(ui, &drawn.scale, intents);
            for (name, grip) in ["left", "right"].into_iter().zip(drawn.knobs) {
                self.platen_knob(ui, name, grip, intents);
            }
        }
    }

    /// The platen's layout this frame, its typing line where the stage has
    /// it.
    fn lay_out(&mut self, ctx: &egui::Context, view: Rect, now: f64) -> Layout {
        let zoom = self.model.zoom_percent;
        if let Some(height) = self.stage.typing_line_height(view, &self.metrics, zoom) {
            self.platen.typing_line_height = height;
        }
        let carriage = self.model.project.machine.carriage();
        let cell = self
            .metrics
            .cell_offset(carriage.half_line, carriage.column);
        self.platen.layout(ctx, view, &self.metrics, cell, now)
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
        let machine = &self.model.project.machine;
        let carriage = machine.carriage();
        let cell = self
            .metrics
            .cell_offset(carriage.half_line, carriage.column);
        let mut paper_origin = layout.paper_origin;
        let mut pointer_opacity = 1.0;
        let mut knob_rolled = layout.strike_point.y - layout.paper_origin.y;
        let table = self.stage.paper_table(scene);
        let table = table.as_ref();
        // The stage draws them: handed over as they come.
        let mut handed = self.stage.draws_sheets().then(Vec::new);
        let flight = self.model.feed.flight.filter(|f| !f.is_over(now));
        let answer = flight.map_or(folder::Answer::STILL, |f| f.answer(now));
        if let Some(feeding) = &self.model.feed.feeding {
            let t = now - feeding.started;
            let motion = &feeding.motion;
            // Points the platen has rolled the old sheet out so far, and in all.
            let mut wound_out = (0.0, 0.0);
            if let Some((old_page, old_half_line)) = &feeding.outgoing {
                let old_y = layout.strike_point.y - self.metrics.cell_offset(*old_half_line, 0).y;
                let old_origin = pos2(paper_origin.x, old_y);
                let dimming = self.dimming(*old_half_line, calm);
                // Calm mode hides the folder: the sheet just winds out.
                if let Some(flight) = flight.filter(|_| !self.model.calm) {
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
                            let sheet = (old_page, dimming);
                            let (on, lift) = (handed.as_mut(), pose.lift);
                            self.paint_going_out(painter, on, scene, origin, lift, sheet);
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
                        let sheet = (old_page, dimming);
                        let on = handed.as_mut();
                        self.paint_going_out(painter, on, scene, old_origin, 1.0, sheet);
                    }
                }
            }
            // In until the top margin meets the typing line: round the
            // platen from the paper table, or up from below the window.
            let placed = layout.strike_point.y - cell.y;
            let by_hand = motion.by_hand();
            let way_in = match table {
                Some(table) => WayIn::round_platen(
                    table,
                    scene.typing_y,
                    self.metrics.points_per_inch,
                    placed,
                    by_hand,
                ),
                None => WayIn::from_below(view.bottom() + SHADOW_ROOM, placed, by_hand),
            };
            let progress = motion.progress(t);
            paper_origin.y = way_in.at(progress);
            // The knob turns with each sheet in turn, but not while a hand
            // puts the new one in.
            let resting = |half_line| self.metrics.cell_offset(half_line, 0).y;
            knob_rolled = knob::feed_roll(
                &self.metrics,
                feeding
                    .outgoing
                    .as_ref()
                    .map(|(_, half_line)| resting(*half_line)),
                layout.strike_point.y - placed,
                wound_out.0 + way_in.knob_done(progress),
                wound_out.1 + way_in.knob_travel(),
            );
            if handed.is_none() {
                let (curl, lift) = (motion.curl(t), motion.lift(t));
                self.paint_lifted(painter, view, paper_origin, curl, lift);
            }
            pointer_opacity = motion.pointer_opacity(t);
        }
        let dimming = self.dimming(carriage.half_line, calm);
        let wetness = |half_line, column| self.model.project.wetness(now, half_line, column);
        if let Some(mut handed) = handed {
            let sheet = (machine.page(), dimming, &wetness as paper::Wetness);
            handed.push(self.flat_sheet(painter, paper_origin, sheet));
            self.stage.paint_sheets(painter, scene, handed);
        } else {
            self.paint_page(painter, machine.page(), paper_origin, dimming, &wetness);
        }
        Sheets {
            knob_rolled,
            pointer_opacity,
            flight,
            answer,
        }
    }

    /// The scale on its plate, hanging from the typing line, travelling with
    /// the paper.
    fn lay_out_scale(&self, layout: &Layout) -> ruler::Scale {
        let top = ruler::top(&self.metrics, layout.strike_point);
        let columns = self.model.project.machine.page().columns();
        ruler::Scale::new(&self.metrics, columns, layout.paper_origin.x, top)
    }

    /// Over the sheets, calm or not: the stage's knobs, lit under the pointer
    /// if `active`; its printing of `scale`; what it has in front of the
    /// sheets; its own controls, which answer at once.
    fn show_over_sheets(
        &self,
        ui: &egui::Ui,
        painter: &Painter,
        scene: &Scene,
        (sheets, scale, active): (&Sheets, &ruler::Scale, bool),
        intents: &mut Vec<Intent>,
    ) -> Over {
        let knobs = self
            .stage
            .knobs(ui, painter, scene, sheets.knob_rolled, active);
        let carriage = self.model.project.machine.carriage();
        let printed = self.stage.scale(painter, scene, scale, carriage);
        self.stage.paint_over_sheets(painter, scene);
        let (model, project) = (&self.model, &self.model.project);
        let keeping = project
            .filing
            .keeping(model.settings.saving.autosave, scene.now);
        let goals = model.settings.goals.cycle();
        let controls = Controls {
            spacing: project.machine.carriage().line_spacing,
            zoom_percent: model.zoom_percent,
            erase: project.machine.constraints.erase,
            slip_in: project.machine.slip_in(),
            delete_in_cycle: model.settings.machine.rules.delete_in_cycle,
            goal: project.session.goal(),
            goals: &goals,
            progress: project.session.progress(),
            keeping: &keeping,
        };
        let own_controls = self.stage.controls(ui, painter, scene, &controls);
        if let Some(rects) = own_controls {
            self.control_buttons(ui, rects, &keeping, intents);
        }
        Over {
            own_controls: own_controls.is_some(),
            knobs,
            scale: printed,
        }
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
        if self.model.project.machine.slip_in() {
            platen::paint_slip(painter, &self.metrics, layout.strike_point);
        }
        platen::paint_guides(
            painter,
            &self.metrics,
            layout.strike_point,
            layout.paper_origin.x,
            self.model.guides_opacity(now),
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
        let finished = self.model.project.machine.document().finished().len();
        let shown = if sheets.flight.is_some_and(|f| !f.has_landed(now)) {
            finished.saturating_sub(1)
        } else {
            finished
        };
        let reveal = sheets
            .flight
            .filter(|_| self.model.calm)
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

    /// What fades in calm: the scale on its plate, the knobs and the plates,
    /// each unless the stage has its own (`over`): `scale` as laid out.
    #[allow(clippy::too_many_arguments)]
    fn paint_chrome(
        &self,
        ui: &egui::Ui,
        painter: &Painter,
        scene: &Scene,
        layout: &Layout,
        chrome: f32,
        sheets: &Sheets,
        (scale, over): (ruler::Scale, &Over),
    ) -> Chrome {
        let project = &self.model.project;
        let machine = &project.machine;
        let carriage = machine.carriage();
        // The plates hang below the typing line, the scale's plate above them.
        let scale_top = ruler::top(&self.metrics, layout.strike_point);
        let plates_top = scale_top + ruler::HEIGHT;
        let scale = over.scale.unwrap_or_else(|| {
            ruler::paint_scale(painter, &scale, carriage);
            scale
        });
        let knobs = over.knobs.unwrap_or_else(|| {
            let paper_left = layout.paper_origin.x;
            let busy = self.model.is_busy(scene.now);
            [
                (Side::Left, paper_left),
                (Side::Right, paper_left + self.metrics.paper_size.x),
            ]
            .map(|(side, edge)| {
                let knob = knob::Knob::new(&self.metrics, side, edge, scale_top);
                let hovered = chrome >= 1.0 && !busy && ui.rect_contains_pointer(knob.grip());
                knob.paint(painter, sheets.knob_rolled, hovered);
                knob.grip()
            })
        });
        let plates = (!over.own_controls).then(|| {
            let spacing_plate = ruler::paint_spacing_indicator(
                painter,
                carriage.line_spacing,
                layout.paper_origin.x,
                plates_top,
            );
            let zoom_plate =
                ruler::paint_zoom_plate(painter, self.model.zoom_percent, spacing_plate);
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
                .keeping(self.model.settings.saving.autosave, scene.now);
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
        let filing = &self.model.project.filing;
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
            if self.model.settings.machine.rules.delete_in_cycle {
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
        let model = &mut self.model;
        let Some(stand) = &mut model.overlays.holder else {
            return;
        };
        let (profile, ink_realism) = (
            model.project.machine.profile(),
            model.settings.look.ink_realism,
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
            // The front face up rolls on, as on a platen.
            let points = -response.drag_delta().y;
            intents.push(Intent::DragKnob { points, per_notch });
        } else if response.drag_stopped() {
            intents.push(Intent::ReleaseKnob);
        }
    }

    /// The scale's margin stops: click to release the margins, drag to move one.
    fn margin_stops(&self, ui: &egui::Ui, scale: &ruler::Scale, intents: &mut Vec<Intent>) {
        let carriage = self.model.project.machine.carriage();
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
        let look = &self.model.settings.look;
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
        let machine = &self.model.project.machine;
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
            self.model.settings.look.ink_realism,
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
        let machine = &self.model.project.machine;
        let mut frame = painter.clone();
        frame.multiply_opacity(render::smoothstep((pose.scale - 0.6) / 0.4));
        let top_lines = machine.profile().margins.top_lines;
        paper::paint_margin_frame(&frame, &metrics, machine.carriage(), top_lines, origin);
        let ink_realism = self.model.settings.look.ink_realism;
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

    /// A finished sheet at `origin`, on its way out as if in the machine:
    /// `handed` to the stage, else `lift`ed.
    fn paint_going_out(
        &self,
        painter: &Painter,
        handed: Option<&mut Vec<FlatSheet>>,
        scene: &Scene,
        origin: Pos2,
        lift: f32,
        (page, dimming): (&Page, Dimming),
    ) {
        if let Some(handed) = handed {
            let sheet = (page, dimming, &paper::dry as paper::Wetness);
            handed.push(self.flat_sheet(painter, origin, sheet));
            return;
        }
        self.paint_lifted(painter, scene.view, origin, 0.0, lift);
        self.paint_page(painter, page, origin, dimming, &paper::dry);
    }

    /// The sheet at `origin`, for a stage that draws the sheets itself: all
    /// its marks, as it may show more of the sheet than the window's clip.
    fn flat_sheet(
        &self,
        painter: &Painter,
        origin: Pos2,
        (page, dimming, wetness): (&Page, Dimming, paper::Wetness<'_>),
    ) -> FlatSheet {
        let size = self.metrics.paper_size;
        let mut quad = egui::Mesh::default();
        for corner in [
            pos2(0.0, 0.0),
            pos2(1.0, 0.0),
            pos2(1.0, 1.0),
            pos2(0.0, 1.0),
        ] {
            quad.vertices.push(Vertex {
                pos: origin + corner.to_vec2() * size,
                uv: corner,
                color: Color32::WHITE,
            });
        }
        quad.add_triangle(0, 1, 2);
        quad.add_triangle(0, 2, 3);
        let machine = &self.model.project.machine;
        let top_lines = machine.profile().margins.top_lines;
        let mut print = paper::margin_frame(&self.metrics, machine.carriage(), top_lines, origin);
        let ink_realism = self.model.settings.look.ink_realism;
        let everywhere = |_| true;
        let marks = paper::sheet_marks(
            &self.metrics,
            page,
            origin,
            ink_realism,
            dimming,
            wetness,
            everywhere,
        );
        print.extend(paper::shapes(painter, &self.metrics, marks));
        FlatSheet {
            paper: self.background.bent_sheet(size, quad),
            print,
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> PaperTable {
        PaperTable {
            wrap_mm: 50.8,
            seen_mm: 127.0,
        }
    }

    fn round_platen(by_hand: f32) -> WayIn {
        WayIn::round_platen(&table(), 460.0, 96.0, 364.0, by_hand)
    }

    #[test]
    fn from_below_rises_evenly_and_the_knob_winds_after_the_hand() {
        let way = WayIn::from_below(900.0, 300.0, 0.25);
        for p in [0.0, 0.1, 0.25, 0.6, 1.0] {
            assert!((way.at(p) - (900.0 - 600.0 * p)).abs() < 1e-3, "{p}");
        }
        assert_eq!(way.knob_done(0.2), 0.0);
        assert!((way.knob_travel() - 450.0).abs() < 1e-3);
        assert!((way.knob_done(1.0) - way.knob_travel()).abs() < 1e-3);
    }

    #[test]
    fn round_the_platen_the_hand_slides_it_down_the_table_from_over_its_top() {
        let way = round_platen(0.3);
        // Its top edge where it goes in once the hand is done.
        assert_eq!(way.at(0.3), 460.0 + 2.0 * 96.0);
        // Just out of sight over the table's top at first.
        assert_eq!(way.at(0.0) - way.knob_from, 5.0 * 96.0);
        assert_eq!(way.at(1.0), 364.0);
        assert_eq!(way.knob_travel(), way.knob_from - 364.0);
    }

    #[test]
    fn with_no_hand_the_knob_winds_it_all() {
        let way = round_platen(0.0);
        assert_eq!(way.at(0.0), way.knob_from);
        assert_eq!(way.knob_done(1.0), way.knob_travel());
        let all_hand = round_platen(1.0);
        assert_eq!(all_hand.at(1.0), 364.0);
    }
}
