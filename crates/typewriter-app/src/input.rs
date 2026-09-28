//! Keyboard events to typewriter commands.
//!
//! A typewriter has no Control key, so no binding uses one. Default bindings:
//! - printable keys: type
//! - Enter: carriage return; held, it rolls the paper on line by line
//!   (at the keyboard's repeat rate)
//! - Insert: feed a new sheet
//! - Backspace: carriage back one column (no erase)
//! - Shift+Backspace, Delete: erase
//! - Tab: tabulate
//! - hold Shift and tap Tab: once sets a tab stop at the carriage, twice clears
//!   the nearest stop, three times clears all stops. Acts when Shift is released.
//! - F1 / F2 / F3: line spacing 1 / 1.5 / 2
//! - arrows: move the carriage and platen (only if free movement is allowed)
//! - Page Up: open the folder of finished sheets; there, arrows or Page Up /
//!   Page Down choose or flip sheets (up/left = older), Enter opens the chosen
//!   one and Esc goes back. The app redirects these; see `app.rs`.
//! - Esc (typing view): calm mode on / off
//! - F11: fullscreen
//!
//! A typewriter's keys do not repeat, so held keys that act on the machine
//! only act once, except Enter (above), the arrows and the letters.

use eframe::egui::{Event, Key, Modifiers};
use typewriter_core::{Command, Direction, LineSpacing};

/// What a key asks for: a machine command, or something for the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Machine(Command),
    PageUp,
    PageDown,
    Escape,
    Fullscreen,
}

#[derive(Debug, Default)]
pub struct Input {
    shift_tab_taps: u8,
}

impl Input {
    /// Translates one frame of events. `shift_down` is the Shift state at the
    /// end of the frame; releasing it completes a Shift+Tab gesture.
    pub fn actions(&mut self, events: &[Event], shift_down: bool) -> Vec<Action> {
        let mut actions = Vec::new();
        // The space bar arrives as text, alongside its key event.
        let space_held = events.iter().any(|e| {
            matches!(
                e,
                Event::Key {
                    key: Key::Space,
                    pressed: true,
                    repeat: true,
                    ..
                }
            )
        });
        for event in events {
            match event {
                Event::Key {
                    key: Key::Tab,
                    pressed: true,
                    repeat,
                    modifiers,
                    ..
                } if modifiers.shift => {
                    if !repeat {
                        self.shift_tab_taps = (self.shift_tab_taps + 1).min(3);
                    }
                    continue;
                }
                // Any other key finishes the gesture first, so the stop is set
                // where the carriage was when Tab was tapped.
                Event::Key { pressed: true, .. } | Event::Text(_) => {
                    actions.extend(self.finish_gesture().map(Action::Machine));
                }
                _ => {}
            }
            actions.extend(
                action_for(event)
                    .into_iter()
                    .filter(|a| !(space_held && *a == Action::Machine(Command::Type(' ')))),
            );
        }
        if !shift_down {
            actions.extend(self.finish_gesture().map(Action::Machine));
        }
        actions
    }

    fn finish_gesture(&mut self) -> Option<Command> {
        let command = match self.shift_tab_taps {
            0 => None,
            1 => Some(Command::SetTabStop),
            2 => Some(Command::ClearTabStop),
            _ => Some(Command::ClearAllTabStops),
        };
        self.shift_tab_taps = 0;
        command
    }
}

fn action_for(event: &Event) -> Vec<Action> {
    match event {
        Event::Text(text) => text
            .chars()
            .filter(|c| !c.is_control())
            .map(|c| Action::Machine(Command::Type(c)))
            .collect(),
        Event::Key {
            key,
            pressed: true,
            repeat,
            modifiers,
            ..
        } => key_action(*key, *modifiers, *repeat).into_iter().collect(),
        _ => vec![],
    }
}

fn key_action(key: Key, m: Modifiers, repeat: bool) -> Option<Action> {
    let app = match key {
        Key::PageUp => Some(Action::PageUp),
        Key::PageDown => Some(Action::PageDown),
        Key::Escape if !repeat => Some(Action::Escape),
        Key::F11 if !repeat => Some(Action::Fullscreen),
        Key::Escape | Key::F11 => return None,
        _ => None,
    };
    app.or_else(|| key_command(key, m, repeat).map(Action::Machine))
}

fn key_command(key: Key, m: Modifiers, repeat: bool) -> Option<Command> {
    if repeat {
        return match key {
            // Holding Return is like winding the platen knob.
            Key::Enter => Some(Command::LineFeed),
            Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown => {
                key_command(key, m, false)
            }
            _ => None,
        };
    }
    let command = match key {
        Key::Insert => Command::FeedSheet,
        Key::Enter => Command::Return,
        Key::Backspace if m.shift => Command::Erase,
        Key::Backspace => Command::Backspace,
        Key::Delete => Command::Erase,
        Key::Tab if !m.any() => Command::Tab,
        Key::F1 => Command::SetLineSpacing(LineSpacing::Single),
        Key::F2 => Command::SetLineSpacing(LineSpacing::OneAndHalf),
        Key::F3 => Command::SetLineSpacing(LineSpacing::Double),
        Key::ArrowLeft => Command::Move(Direction::Left),
        Key::ArrowRight => Command::Move(Direction::Right),
        Key::ArrowUp => Command::Move(Direction::Up),
        Key::ArrowDown => Command::Move(Direction::Down),
        _ => return None,
    };
    Some(command)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(key: Key, modifiers: Modifiers) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    fn shift_tab() -> Event {
        key(Key::Tab, Modifiers::SHIFT)
    }

    fn one_frame(events: &[Event]) -> Vec<Command> {
        Input::default()
            .actions(events, false)
            .into_iter()
            .map(|a| match a {
                Action::Machine(c) => c,
                other => panic!("not a machine command: {other:?}"),
            })
            .collect()
    }

    fn machine(input: &mut Input, events: &[Event], shift_down: bool) -> Vec<Command> {
        input
            .actions(events, shift_down)
            .into_iter()
            .filter_map(|a| match a {
                Action::Machine(c) => Some(c),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn text_types_each_printable_char() {
        assert_eq!(
            one_frame(&[Event::Text("a b\r".into())]),
            [Command::Type('a'), Command::Type(' '), Command::Type('b')]
        );
    }

    #[test]
    fn backspace_moves_back_and_shift_backspace_or_delete_erases() {
        assert_eq!(
            one_frame(&[key(Key::Backspace, Modifiers::NONE)]),
            [Command::Backspace]
        );
        assert_eq!(
            one_frame(&[key(Key::Backspace, Modifiers::SHIFT)]),
            [Command::Erase]
        );
        assert_eq!(
            one_frame(&[key(Key::Delete, Modifiers::NONE)]),
            [Command::Erase]
        );
    }

    #[test]
    fn enter_returns_and_plain_tab_tabulates() {
        assert_eq!(
            one_frame(&[key(Key::Enter, Modifiers::NONE)]),
            [Command::Return]
        );
        assert_eq!(one_frame(&[key(Key::Tab, Modifiers::NONE)]), [Command::Tab]);
    }

    fn held(key: Key, modifiers: Modifiers) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: true,
            modifiers,
        }
    }

    #[test]
    fn held_return_rolls_the_paper() {
        assert_eq!(
            one_frame(&[
                key(Key::Enter, Modifiers::NONE),
                held(Key::Enter, Modifiers::NONE),
                held(Key::Enter, Modifiers::NONE),
            ]),
            [Command::Return, Command::LineFeed, Command::LineFeed]
        );
    }

    #[test]
    fn space_backspace_and_erase_do_not_repeat() {
        assert_eq!(
            one_frame(&[key(Key::Space, Modifiers::NONE), Event::Text(" ".into())]),
            [Command::Type(' ')]
        );
        assert!(
            one_frame(&[held(Key::Space, Modifiers::NONE), Event::Text(" ".into())]).is_empty()
        );
        for (k, m) in [
            (Key::Backspace, Modifiers::NONE),
            (Key::Backspace, Modifiers::SHIFT),
            (Key::Delete, Modifiers::NONE),
            (Key::Insert, Modifiers::NONE),
            (Key::Tab, Modifiers::NONE),
        ] {
            assert!(one_frame(&[held(k, m)]).is_empty(), "{k:?}");
        }
        assert_eq!(
            one_frame(&[held(Key::ArrowLeft, Modifiers::NONE)]),
            [Command::Move(Direction::Left)]
        );
    }

    #[test]
    fn key_release_does_nothing() {
        let released = Event::Key {
            key: Key::Enter,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        assert!(one_frame(&[released]).is_empty());
    }

    #[test]
    fn shift_tab_taps_act_when_shift_is_released() {
        for (taps, expected) in [
            (1, Command::SetTabStop),
            (2, Command::ClearTabStop),
            (3, Command::ClearAllTabStops),
            (5, Command::ClearAllTabStops),
        ] {
            let mut input = Input::default();
            for _ in 0..taps {
                assert!(machine(&mut input, &[shift_tab()], true).is_empty());
            }
            assert_eq!(machine(&mut input, &[], false), [expected], "{taps} taps");
            assert!(machine(&mut input, &[], false).is_empty());
        }
    }

    #[test]
    fn held_tab_repeat_is_not_another_tap() {
        let mut input = Input::default();
        let repeat = Event::Key {
            key: Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: true,
            modifiers: Modifiers::SHIFT,
        };
        machine(&mut input, &[shift_tab(), repeat], true);
        assert_eq!(machine(&mut input, &[], false), [Command::SetTabStop]);
    }

    #[test]
    fn another_key_finishes_the_gesture_first() {
        let mut input = Input::default();
        let commands = machine(&mut input, &[shift_tab(), Event::Text("A".into())], true);
        assert_eq!(commands, [Command::SetTabStop, Command::Type('A')]);
    }

    #[test]
    fn insert_feeds_a_sheet_and_ctrl_does_nothing_special() {
        assert_eq!(
            one_frame(&[key(Key::Insert, Modifiers::NONE)]),
            [Command::FeedSheet]
        );
        assert_eq!(
            one_frame(&[key(Key::Enter, Modifiers::COMMAND)]),
            [Command::Return]
        );
        assert!(one_frame(&[key(Key::Num1, Modifiers::COMMAND)]).is_empty());
    }

    #[test]
    fn function_keys_set_line_spacing() {
        assert_eq!(
            one_frame(&[
                key(Key::F1, Modifiers::NONE),
                key(Key::F2, Modifiers::NONE),
                key(Key::F3, Modifiers::NONE),
            ]),
            [
                Command::SetLineSpacing(LineSpacing::Single),
                Command::SetLineSpacing(LineSpacing::OneAndHalf),
                Command::SetLineSpacing(LineSpacing::Double),
            ]
        );
    }

    #[test]
    fn app_keys() {
        let actions = Input::default().actions(
            &[
                key(Key::PageUp, Modifiers::NONE),
                key(Key::PageDown, Modifiers::NONE),
                key(Key::Escape, Modifiers::NONE),
                key(Key::F11, Modifiers::NONE),
            ],
            false,
        );
        assert_eq!(
            actions,
            [
                Action::PageUp,
                Action::PageDown,
                Action::Escape,
                Action::Fullscreen
            ]
        );
    }

    #[test]
    fn held_escape_and_f11_toggle_once() {
        let actions = Input::default().actions(
            &[
                held(Key::Escape, Modifiers::NONE),
                held(Key::F11, Modifiers::NONE),
            ],
            false,
        );
        assert!(actions.is_empty());
    }
}
