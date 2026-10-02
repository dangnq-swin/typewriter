//! Keys, the wheel and the knobs at the machine: routing a frame's keys by
//! what is open, type jams, commands in and their events out.

use typewriter_core::{BlockReason, Command, Direction, EraseMode, Event};

use super::{Model, View};
use crate::app::intent::{Effect, Sound};
use crate::input::Action;
use crate::render::folder::FolderAction;
use crate::stage::Return;

/// Scroll per zoom step or knob notch: about one wheel notch.
const SCROLL_POINTS_PER_STEP: f32 = 40.0;
/// Strikes closer than this tangle, with type jams on. Frame-timed: keys
/// in one frame count as together.
const JAM_SECONDS: f64 = 0.03;
const JAMMED: &str = "Jammed: Backspace frees the typebars";
/// The most notches one frame's knob turn rolls.
const KNOB_NOTCHES_PER_FRAME: f32 = 3.0;
/// Platen guides stay this long after the knob stops, then fade.
const GUIDES_HOLD_SECONDS: f64 = 1.0;
const GUIDES_FADE_SECONDS: f64 = 0.4;

impl Model {
    /// One frame's keys and wheel, routed by what is open.
    pub(super) fn input(&mut self, keys: Vec<Action>, wheel: f32, over_knob: bool, now: f64) {
        if self.keys_to_fields() {
            return;
        }
        if self.overlays.log_open {
            for action in keys {
                match action {
                    Action::Fullscreen => self.toggle_fullscreen(),
                    Action::Save => self.save_now(now),
                    Action::PageUp => self.turn_log(-1),
                    Action::PageDown => self.turn_log(1),
                    Action::Escape => self.overlays.log_open = false,
                    _ => {}
                }
            }
            return;
        }
        if self.view == View::Settings {
            // Keys belong to the card.
            for action in keys {
                match action {
                    Action::Fullscreen => self.toggle_fullscreen(),
                    Action::Escape => self.escape(),
                    Action::Save => self.save_now(now),
                    _ => {}
                }
            }
            return;
        }
        let busy = self.is_busy(now);
        for action in keys {
            match action {
                // Not the machine's: allowed while feeding.
                Action::Fullscreen => self.toggle_fullscreen(),
                Action::Save => self.save_now(now),
                Action::Notebook => self.open_notebook(),
                _ if busy => {}
                // No such key on the machine: the typeface can't print it.
                Action::Machine(Command::Type(c))
                    if !c.is_whitespace() && !self.typeface.contains(&c) => {}
                Action::Machine(command) => self.key(command, now),
                Action::PageUp => self.page_up(),
                Action::PageDown => self.page_down(),
                Action::Escape => self.escape(),
                Action::NextCorrection => self.next_correction(now),
                Action::Delete if self.view == View::Folder => {
                    self.folder_action(FolderAction::Scrunch, now);
                }
                Action::Delete => self.key(Command::Erase, now),
                Action::ShiftArrow(direction) if self.view == View::Folder => {
                    self.move_chosen(direction, now);
                }
                Action::ShiftArrow(direction) => self.key(Command::Move(direction), now),
            }
        }
        if !busy {
            self.wheel(wheel, over_knob, now);
        }
    }

    /// Over a knob the wheel rolls the paper; else, the paper never
    /// scrolling, it zooms: up is closer.
    fn wheel(&mut self, points: f32, over_knob: bool, now: f64) {
        if over_knob && self.view == View::Typing {
            // Wheel up rolls on, as the knob's front turns up on a platen.
            self.turn_knob(points, SCROLL_POINTS_PER_STEP, now);
            return;
        }
        self.scroll_zoom += points;
        if self.scroll_zoom.abs() >= SCROLL_POINTS_PER_STEP {
            self.zoom(if self.scroll_zoom > 0.0 { 1 } else { -1 });
            self.scroll_zoom = 0.0;
        }
    }

    /// Adds `points` of turn (up positive, rolling on); each `per_notch`
    /// rolls a half-line.
    pub(super) fn turn_knob(&mut self, points: f32, per_notch: f32, now: f64) {
        // A flick drops what's past a few notches: each clicks, and clicks
        // struck together get loud.
        let most = KNOB_NOTCHES_PER_FRAME * per_notch;
        self.knob_turn = (self.knob_turn + points).clamp(-most, most);
        while self.knob_turn.abs() >= per_notch {
            let direction = if self.knob_turn > 0.0 {
                Direction::Down
            } else {
                Direction::Up
            };
            self.knob_turn -= per_notch.copysign(self.knob_turn);
            self.apply(Command::Move(direction), now);
        }
    }

    /// A key for the machine: browses in the folder views, else types and
    /// counts towards the session.
    fn key(&mut self, command: Command, now: f64) {
        if self.browse_command(command) {
            return;
        }
        let command = self.jams(command, now);
        self.apply(command, now);
        self.typed(now);
    }

    /// A strike too soon after the last tangles with it, if type jams are on.
    fn jams(&mut self, command: Command, now: f64) -> Command {
        let Command::Type(c) = command else {
            return command;
        };
        if c.is_whitespace() {
            return command;
        }
        let machine = &self.project.machine;
        let too_soon = now - std::mem::replace(&mut self.last_strike, now) < JAM_SECONDS;
        if too_soon && machine.constraints.type_jams && !machine.is_jammed() {
            Command::Jam
        } else {
            command
        }
    }

    /// Applies a command. Any machine command returns to the typing view.
    pub(super) fn apply(&mut self, command: Command, now: f64) {
        self.view = View::Typing;
        let knob = matches!(command, Command::Move(Direction::Up | Direction::Down));
        // Keep a copy: the filed sheet is still seen rolling out.
        let changing = matches!(command, Command::FeedSheet | Command::RollIn { .. });
        let machine = &self.project.machine;
        let outgoing = changing.then(|| (machine.page().clone(), machine.carriage().half_line));
        let column = machine.carriage().column;
        let mut page_end = false;
        self.project.filing.changed(now);
        for event in self.project.machine.apply(command) {
            match event {
                // Feed on a return at the page end: many keyboards lack
                // Insert.
                Event::PageEnd => page_end = true,
                // Only a roll that happened shows the guides.
                Event::LineFeed if knob => self.knob_turned = now,
                Event::CarriageReturn => {
                    let machine = &self.project.machine;
                    let columns = column.abs_diff(machine.carriage().column);
                    // The pitch counts columns an inch: the glide, millimetres.
                    let mm = f64::from(columns) * 25.4 / f64::from(machine.profile().pitch_cpi);
                    self.last_return = Return { at: now, mm };
                }
                Event::Blocked(reason) => {
                    self.effects.push(Effect::Jolt);
                    // Shown again at each blocked key: it stays while tried.
                    if reason == BlockReason::Jammed {
                        self.notice.show(JAMMED, now);
                    }
                }
                Event::Freed => self.notice.withdraw(JAMMED),
                Event::Erase(EraseMode::Fluid) => self.project.dab(now),
                Event::SheetFed => {
                    self.project.sheet_changed();
                    self.keep(now);
                    self.start_feed(outgoing.clone(), now);
                }
                _ => {}
            }
            self.effects.push(Effect::Sound(Sound::Machine(event)));
        }
        if page_end {
            self.apply(Command::FeedSheet, now);
        }
    }

    /// Counts a key towards the session; rings the bell on reaching the goal.
    fn typed(&mut self, now: f64) {
        let project = &mut self.project;
        if project.session.typed(project.machine.document(), now) {
            self.effects
                .push(Effect::Sound(Sound::Machine(Event::Bell)));
        }
        self.project.record_words();
    }

    pub(super) fn next_spacing(&mut self, now: f64) {
        let next = self.project.machine.carriage().line_spacing.next();
        self.apply(Command::SetLineSpacing(next), now);
    }

    pub(super) fn next_correction(&mut self, now: f64) {
        let with_delete = self.settings.machine.rules.delete_in_cycle;
        let next = self.project.machine.constraints.erase.next(with_delete);
        self.apply(Command::SetEraseMode(next), now);
    }

    /// 1 while the knob turns, fading out once it rests. 0 if switched off.
    pub fn guides_opacity(&self, now: f64) -> f32 {
        if !self.settings.look.platen_guides {
            return 0.0;
        }
        let fading = now - self.knob_turned - GUIDES_HOLD_SECONDS;
        (1.0 - fading / GUIDES_FADE_SECONDS).clamp(0.0, 1.0) as f32
    }
}

#[cfg(test)]
mod tests {
    use typewriter_core::{Command, EraseMode, Event};

    use super::super::testing::{model, model_with, press, type_text};
    use super::*;
    use crate::app::intent::Intent;
    use crate::picker::Dialog;
    use crate::settings::Settings;

    fn column(model: &Model) -> u16 {
        model.project.machine.carriage().column
    }

    #[test]
    fn a_return_is_kept_with_how_far_the_carriage_glides_home() {
        let mut model = model();
        assert_eq!(model.last_return, Return::NONE);
        let ret = Action::Machine(Command::Return);
        press(&mut model, &[ret], 10.0);
        assert_eq!(
            model.last_return,
            Return { at: 10.0, mm: 0.0 },
            "at the margin"
        );
        type_text(&mut model, &"x".repeat(60), 12.0);
        press(&mut model, &[ret], 20.0);
        // Pica: 60 columns, 152.4 mm.
        assert_eq!(
            model.last_return,
            Return {
                at: 20.0,
                mm: 152.4
            }
        );
    }

    #[test]
    fn a_letter_the_typeface_lacks_does_not_type() {
        let mut model = model();
        let start = column(&model);
        type_text(&mut model, "\u{416}", 10.0);
        assert_eq!(column(&model), start);
        type_text(&mut model, "a", 11.0);
        assert_eq!(column(&model), start + 1);
    }

    #[test]
    fn strikes_too_close_tangle_only_with_type_jams_on() {
        let mut model = model();
        press(&mut model, &[Action::Machine(Command::Type('a'))], 10.0);
        press(&mut model, &[Action::Machine(Command::Type('b'))], 10.01);
        assert!(!model.project.machine.is_jammed(), "off by default");

        let mut settings = Settings::default();
        settings.machine.rules.type_jams = true;
        let mut model = model_with(settings);
        press(&mut model, &[Action::Machine(Command::Type('a'))], 10.0);
        press(&mut model, &[Action::Machine(Command::Type('b'))], 10.01);
        assert!(model.project.machine.is_jammed());
        assert!(model.take_effects().contains(&Effect::Jolt));
        assert!(model.notice.is_animating(10.1));
        press(&mut model, &[Action::Machine(Command::Backspace)], 11.0);
        assert!(!model.project.machine.is_jammed());
        assert!(!model.notice.is_animating(11.0), "freed: the notice goes");
    }

    #[test]
    fn the_wheel_rolls_the_paper_over_a_knob_and_zooms_elsewhere() {
        let mut model = model();
        let half_line = model.project.machine.carriage().half_line;
        let wheel = |model: &mut Model, points, over_knob, now| {
            let keys = Vec::new();
            model.update(
                Intent::Input {
                    keys,
                    wheel: points,
                    over_knob,
                },
                now,
            );
        };
        wheel(&mut model, SCROLL_POINTS_PER_STEP, true, 10.0);
        assert_eq!(model.project.machine.carriage().half_line, half_line + 1);
        assert_eq!(model.zoom_percent, 100);
        wheel(&mut model, 25.0, false, 10.1);
        assert_eq!(model.zoom_percent, 100, "not a whole notch yet");
        wheel(&mut model, 25.0, false, 10.2);
        assert_eq!(model.zoom_percent, 110);
        assert!(model.take_effects().contains(&Effect::Zoomed));
    }

    #[test]
    fn a_flicked_knob_rolls_at_most_three_notches_a_frame() {
        let mut model = model();
        let half_line = model.project.machine.carriage().half_line;
        model.update(
            Intent::DragKnob {
                points: 100.0,
                per_notch: 10.0,
            },
            10.0,
        );
        assert_eq!(model.project.machine.carriage().half_line, half_line + 3);
        assert!(model.guides_opacity(10.5) > 0.0);
        assert_eq!(model.guides_opacity(20.0), 0.0);
    }

    #[test]
    fn a_return_on_the_last_line_feeds_a_sheet_and_keys_wait_for_it() {
        let mut model = model();
        type_text(&mut model, "first", 10.0);
        let mut now = 11.0;
        while model.project.machine.document().finished().is_empty() {
            press(&mut model, &[Action::Machine(Command::Return)], now);
            now += 0.1;
        }
        assert!(model.is_busy(now));
        let start = column(&model);
        type_text(&mut model, "x", now);
        assert_eq!(column(&model), start, "locked while the sheet feeds");
        let later = now + 60.0;
        model.tick(later);
        type_text(&mut model, "x", later);
        assert_eq!(column(&model), start + 1);
    }

    #[test]
    fn delete_joins_the_correction_cycle_only_by_the_rule() {
        let cycle = |settings: Settings| {
            let mut model = model_with(settings);
            (0..5)
                .map(|i| {
                    press(&mut model, &[Action::NextCorrection], 10.0 + f64::from(i));
                    model.project.machine.constraints.erase
                })
                .collect::<Vec<_>>()
        };
        assert!(!cycle(Settings::default()).contains(&EraseMode::Delete));
        let mut settings = Settings::default();
        settings.machine.rules.delete_in_cycle = true;
        assert!(cycle(settings).contains(&EraseMode::Delete));
    }

    #[test]
    fn fluid_dries_a_few_seconds_after_the_dab() {
        let mut model = model();
        type_text(&mut model, "x", 10.0);
        model.project.machine.constraints.erase = EraseMode::Fluid;
        press(&mut model, &[Action::Delete], 11.0);
        let effects = model.take_effects();
        assert!(effects.contains(&Effect::Sound(Sound::Machine(Event::Erase(
            EraseMode::Fluid
        )))));
        let at = column(&model);
        let half_line = model.project.machine.carriage().half_line;
        assert!(model.project.wetness(11.5, half_line, at) > 0.0);
        model.tick(12.0);
        assert!(model.project.is_drying());
        model.tick(20.0);
        assert!(!model.project.is_drying());
    }

    #[test]
    fn keys_to_a_field_never_reach_the_machine() {
        let mut model = model();
        model.overlays.renaming = Some("novel".into());
        assert!(model.keys_to_fields());
        type_text(&mut model, "gone", 10.0);
        assert!(model.project.machine.page().is_blank());
    }

    #[test]
    fn keys_belong_to_the_log_while_it_is_open() {
        let mut model = model();
        model.update(Intent::OpenLog, 10.0);
        model.take_effects();
        type_text(&mut model, "x", 11.0);
        assert!(
            model.project.machine.page().is_blank(),
            "the log reads, the machine waits"
        );
        press(&mut model, &[Action::PageUp, Action::PageDown], 11.1);
        press(&mut model, &[Action::Save], 11.2);
        assert!(
            model.take_effects().contains(&Effect::Ask(Dialog::SaveAs)),
            "Save still saves, even from the log"
        );
        let before = model.settings.look.fullscreen;
        press(&mut model, &[Action::Fullscreen], 11.3);
        assert!(
            model.take_effects().contains(&Effect::Fullscreen(!before)),
            "and the desktop still hears about fullscreen"
        );
        press(&mut model, &[Action::Escape], 11.4);
        assert!(!model.overlays.log_open);
        type_text(&mut model, "x", 11.5);
        assert!(!model.project.machine.page().is_blank(), "keys come back");
    }

    #[test]
    fn the_settings_card_keeps_the_keys_save_escape_and_fullscreen() {
        let mut model = model();
        model.update(Intent::OpenSettings, 10.0);
        model.take_effects();
        type_text(&mut model, "x", 11.0);
        assert!(
            model.project.machine.page().is_blank(),
            "keys belong to the card"
        );
        press(&mut model, &[Action::Save], 11.1);
        assert!(model.take_effects().contains(&Effect::Ask(Dialog::SaveAs)));
        let before = model.settings.look.fullscreen;
        press(&mut model, &[Action::Fullscreen], 11.2);
        assert!(model.take_effects().contains(&Effect::Fullscreen(!before)));
        press(&mut model, &[Action::Escape], 11.3);
        assert_eq!(model.view, View::Typing, "Escape shuts the card");
    }

    #[test]
    fn save_and_shift_arrows_answer_at_the_machine() {
        let mut model = model();
        type_text(&mut model, "ab", 10.0);
        model.take_effects();
        press(&mut model, &[Action::Save], 11.0);
        assert!(model.take_effects().contains(&Effect::Ask(Dialog::SaveAs)));
        let at = column(&model);
        press(&mut model, &[Action::ShiftArrow(Direction::Left)], 11.1);
        assert_eq!(column(&model), at, "free movement is off by default");
        assert!(
            model.take_effects().contains(&Effect::Jolt),
            "the key still reaches the machine, and it says so"
        );
    }

    #[test]
    fn the_guides_can_be_switched_off() {
        let mut settings = Settings::default();
        settings.look.platen_guides = false;
        let mut model = model_with(settings);
        model.update(
            Intent::DragKnob {
                points: 40.0,
                per_notch: 10.0,
            },
            10.0,
        );
        assert_eq!(model.guides_opacity(10.1), 0.0);
    }
}
