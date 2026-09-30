//! The settings in effect: fullscreen, zoom, goals, and edits on the card.

use typewriter_core::Goal;

use super::{Desk, View};
use crate::app::intent::Effect;
use crate::render;
use crate::settings::{self, Settings, ZOOM_MAX, ZOOM_MIN, ZOOM_STEP};

impl Desk {
    pub(super) fn toggle_fullscreen(&mut self) {
        let look = &mut self.settings.look;
        look.fullscreen = !look.fullscreen;
        self.effects.push(Effect::Fullscreen(look.fullscreen));
    }

    /// Zooms `steps` in (positive) or out (negative).
    pub(super) fn zoom(&mut self, steps: i32) {
        self.set_zoom(zoomed(self.zoom_percent, steps));
    }

    pub(super) fn set_zoom(&mut self, percent: u16) {
        self.settings.look.zoom_percent = percent;
        if percent != self.zoom_percent {
            self.zoom_percent = percent;
            self.effects.push(Effect::Zoomed);
        }
    }

    /// Screen points per inch of paper in the typing view.
    pub fn points_per_inch(&self) -> f32 {
        render::points_per_inch(self.zoom_percent)
    }

    pub(super) fn next_goal(&mut self) {
        let session = &mut self.project.session;
        let goal = Goal::next(session.goal(), &self.settings.goals.cycle());
        session.set_goal(goal);
        self.settings.goals.goal = goal;
    }

    pub(super) fn open_settings(&mut self) {
        // Reload: list profiles added since start.
        self.effects.push(Effect::ReloadMachines);
        self.overlays.notebook = None;
        self.view = View::Settings;
    }

    /// Puts edits made on the card since `before` into effect.
    pub(super) fn settings_edited(&mut self, before: &Settings, now: f64) {
        if self.settings == *before {
            return;
        }
        self.follow_custom_goal(&before.goals);
        let rules = &self.settings.machine.rules;
        let constraints = &mut self.project.machine.constraints;
        if rules.apply_changes(&before.machine.rules, constraints) {
            self.project.filing.changed(now);
        }
        self.set_zoom(self.settings.look.zoom_percent);
        let goal = self.settings.goals.goal;
        if self.project.session.goal() != goal {
            self.project.session.set_goal(goal);
        }
        self.effects.push(Effect::SettingsChanged);
    }

    /// Editing the chosen custom goal changes the chosen goal with it.
    fn follow_custom_goal(&mut self, before: &settings::Goals) {
        let goals = &mut self.settings.goals;
        let (old, new) = (before.custom(), goals.custom());
        let is_preset = old.is_some_and(|old| Goal::cycle(None).contains(&old));
        if old != new && goals.goal == old && !is_preset {
            goals.goal = new;
        }
    }
}

fn zoomed(percent: u16, steps: i32) -> u16 {
    let target = i32::from(percent) + steps * i32::from(ZOOM_STEP);
    // Safe cast: clamped to the zoom range.
    target.clamp(i32::from(ZOOM_MIN), i32::from(ZOOM_MAX)) as u16
}

#[cfg(test)]
mod tests {
    use super::super::testing::{desk, press};
    use super::*;
    use crate::app::intent::Intent;
    use crate::input::Action;

    #[test]
    fn zoom_steps_within_limits() {
        assert_eq!(zoomed(100, 1), 110);
        assert_eq!(zoomed(100, -1), 90);
        assert_eq!(zoomed(200, 1), 200);
        assert_eq!(zoomed(50, -1), 50);
    }

    #[test]
    fn f11_switches_fullscreen_in_the_settings_and_the_window() {
        let mut desk = desk();
        let was = desk.settings.look.fullscreen;
        press(&mut desk, &[Action::Fullscreen], 10.0);
        assert_eq!(desk.settings.look.fullscreen, !was);
        assert_eq!(desk.take_effects(), [Effect::Fullscreen(!was)]);
    }

    #[test]
    fn an_edited_card_puts_its_rules_and_zoom_into_effect() {
        let mut desk = desk();
        desk.update(Intent::OpenSettings, 10.0);
        let before = desk.settings.clone();
        desk.settings.machine.rules.type_jams = true;
        desk.settings.look.zoom_percent = 150;
        desk.update(Intent::SettingsEdited(Box::new(before)), 10.0);
        assert!(desk.project.machine.constraints.type_jams);
        assert_eq!(desk.zoom_percent, 150);
        let effects = desk.take_effects();
        assert!(effects.contains(&Effect::Zoomed));
        assert!(effects.contains(&Effect::SettingsChanged));
        press(&mut desk, &[Action::Escape], 11.0);
        assert_eq!(desk.view, View::Typing);
    }

    #[test]
    fn the_goal_plate_cycles_the_session_and_the_setting_together() {
        let mut desk = desk();
        desk.update(Intent::NextGoal, 10.0);
        let goal = desk.project.session.goal();
        assert!(goal.is_some());
        assert_eq!(desk.settings.goals.goal, goal);
    }
}
