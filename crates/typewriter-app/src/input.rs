//! Keyboard events to typewriter commands.
//!
//! Default bindings:
//! - printable keys: type
//! - Enter: carriage return
//! - Backspace: carriage back one column (no erase)
//! - Shift+Backspace, Delete: erase
//! - Tab: tabulate
//! - hold Shift and tap Tab: once sets a tab stop at the carriage, twice clears
//!   the nearest stop, three times clears all stops. Acts when Shift is released.
//! - Ctrl+1 / Ctrl+2 / Ctrl+3: line spacing 1 / 1.5 / 2
//! - arrows: move the carriage and platen (only if free movement is allowed)

use eframe::egui::{Event, Key, Modifiers};
use typewriter_core::{Command, Direction, LineSpacing};

#[derive(Debug, Default)]
pub struct Input {
    shift_tab_taps: u8,
}

impl Input {
    /// Translates one frame of events. `shift_down` is the Shift state at the
    /// end of the frame; releasing it completes a Shift+Tab gesture.
    pub fn commands(&mut self, events: &[Event], shift_down: bool) -> Vec<Command> {
        let mut commands = Vec::new();
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
                    commands.extend(self.finish_gesture());
                }
                _ => {}
            }
            commands.extend(command_for(event));
        }
        if !shift_down {
            commands.extend(self.finish_gesture());
        }
        commands
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

fn command_for(event: &Event) -> Vec<Command> {
    match event {
        Event::Text(text) => text
            .chars()
            .filter(|c| !c.is_control())
            .map(Command::Type)
            .collect(),
        Event::Key {
            key,
            pressed: true,
            modifiers,
            ..
        } => key_command(*key, *modifiers).into_iter().collect(),
        _ => vec![],
    }
}

fn key_command(key: Key, m: Modifiers) -> Option<Command> {
    let command = match key {
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
        Input::default().commands(events, false)
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
                assert!(input.commands(&[shift_tab()], true).is_empty());
            }
            assert_eq!(input.commands(&[], false), [expected], "{taps} taps");
            assert!(input.commands(&[], false).is_empty());
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
        input.commands(&[shift_tab(), repeat], true);
        assert_eq!(input.commands(&[], false), [Command::SetTabStop]);
    }

    #[test]
    fn another_key_finishes_the_gesture_first() {
        let mut input = Input::default();
        let commands = input.commands(&[shift_tab(), Event::Text("A".into())], true);
        assert_eq!(commands, [Command::SetTabStop, Command::Type('A')]);
    }
}
