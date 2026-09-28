//! Keyboard events to typewriter commands.
//!
//! Default bindings:
//! - printable keys: type
//! - Enter: carriage return
//! - Ctrl+Enter: feed a new sheet
//! - Backspace: carriage back one column (no erase)
//! - Shift+Backspace, Delete: erase
//! - Tab: tabulate
//! - hold Shift and tap Tab: once sets a tab stop at the carriage, twice clears
//!   the nearest stop, three times clears all stops. Acts when Shift is released.
//! - Ctrl+1 / Ctrl+2 / Ctrl+3: line spacing 1 / 1.5 / 2
//! - arrows: move the carriage and platen (only if free movement is allowed)
//! - Ctrl+Plus / Ctrl+Minus / Ctrl+0: zoom in / out / reset
//! - Page Up: open the folder of finished sheets; there, arrows or Page Up /
//!   Page Down choose or flip sheets (up/left = older), Enter opens the chosen
//!   one and Esc goes back. The app redirects these; see `app.rs`.

use eframe::egui::{Event, Key, Modifiers};
use typewriter_core::{Command, Direction, LineSpacing};

/// What a key asks for: a machine command, or something for the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Machine(Command),
    Zoom(Zoom),
    PageUp,
    PageDown,
    Escape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zoom {
    In,
    Out,
    Reset,
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
            actions.extend(action_for(event));
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
            modifiers,
            ..
        } => key_action(*key, *modifiers).into_iter().collect(),
        _ => vec![],
    }
}

fn key_action(key: Key, m: Modifiers) -> Option<Action> {
    let app = match key {
        Key::Plus | Key::Equals if m.command => Some(Action::Zoom(Zoom::In)),
        Key::Minus if m.command => Some(Action::Zoom(Zoom::Out)),
        Key::Num0 if m.command => Some(Action::Zoom(Zoom::Reset)),
        Key::PageUp => Some(Action::PageUp),
        Key::PageDown => Some(Action::PageDown),
        Key::Escape => Some(Action::Escape),
        _ => None,
    };
    app.or_else(|| key_command(key, m).map(Action::Machine))
}

fn key_command(key: Key, m: Modifiers) -> Option<Command> {
    let command = match key {
        Key::Enter if m.command => Command::FeedSheet,
        Key::Enter => Command::Return,
        Key::Backspace if m.shift => Command::Erase,
        Key::Backspace => Command::Backspace,
        Key::Delete => Command::Erase,
        Key::Tab if !m.any() => Command::Tab,
        Key::Num1 if m.command => Command::SetLineSpacing(LineSpacing::Single),
        Key::Num2 if m.command => Command::SetLineSpacing(LineSpacing::OneAndHalf),
        Key::Num3 if m.command => Command::SetLineSpacing(LineSpacing::Double),
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
    fn ctrl_enter_feeds_a_sheet() {
        assert_eq!(
            one_frame(&[key(Key::Enter, Modifiers::COMMAND)]),
            [Command::FeedSheet]
        );
    }

    #[test]
    fn app_keys() {
        let actions = Input::default().actions(
            &[
                key(Key::Equals, Modifiers::COMMAND),
                key(Key::Minus, Modifiers::COMMAND),
                key(Key::Num0, Modifiers::COMMAND),
                key(Key::PageUp, Modifiers::NONE),
                key(Key::PageDown, Modifiers::NONE),
                key(Key::Escape, Modifiers::NONE),
            ],
            false,
        );
        assert_eq!(
            actions,
            [
                Action::Zoom(Zoom::In),
                Action::Zoom(Zoom::Out),
                Action::Zoom(Zoom::Reset),
                Action::PageUp,
                Action::PageDown,
                Action::Escape,
            ]
        );
    }
}
