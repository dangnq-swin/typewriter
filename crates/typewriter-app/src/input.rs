//! Keyboard events → actions. The full key map.
//!
//! No Ctrl bindings (a typewriter has none), except Ctrl+S. Keys don't repeat
//! unless a real machine would: only letters, arrows and Enter do.
//!
//! - printable keys: type, except 1 and ! (no such keys)
//! - 1 / !: the scratchpad; Esc or a click away puts it back. While it is
//!   open, Page Up / Page Down turn its leaves (`render/pad.rs`)
//! - Enter: return; held, rolls the paper a line per key repeat
//! - Insert: feed a new sheet
//! - Backspace: carriage back (no erase)
//! - Shift+Backspace, Delete: erase (in the folder, Delete scrunches up)
//! - Tab: tabulate
//! - hold Shift, tap Tab: 1× set a stop, 2× clear the nearest, 3× clear all;
//!   acts on Shift release
//! - F1 / F2 / F3: line spacing 1 / 1.5 / 2
//! - F4: next correction method
//! - arrows: free movement (if allowed)
//! - Page Up / Page Down, arrows, Shift+arrows, Enter, Esc: the folder;
//!   `app.rs` redirects them there
//! - Esc: calm mode (typing view)
//! - F11: fullscreen
//! - Ctrl+S: save (Save As for a draft)

use eframe::egui::{Event, Key, Modifiers};
use typewriter_core::{Command, Direction, LineSpacing};

/// No such keys: type 1 as l, and ! as ' Backspace . (overstruck). The key
/// opens the scratchpad instead.
const SCRATCHPAD_KEYS: [char; 2] = ['1', '!'];

/// A machine command, or something for the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Machine(Command),
    PageUp,
    PageDown,
    Escape,
    Fullscreen,
    NextCorrection,
    Save,
    Scratchpad,
    /// Erase; in the folder, scrunch up the chosen sheet.
    Delete,
    /// In the folder, moves the chosen sheet; elsewhere a plain arrow.
    ShiftArrow(Direction),
}

#[derive(Debug, Default)]
pub struct Input {
    shift_tab_taps: u8,
}

impl Input {
    /// Translates one frame of events. `shift_down`: Shift at frame end
    /// (releasing it completes a Shift+Tab gesture).
    pub fn actions(&mut self, events: &[Event], shift_down: bool) -> Vec<Action> {
        let mut actions = Vec::new();
        // Space arrives as text too: drop it when its key is repeating.
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
                // Finish the gesture first: the stop goes where Tab was tapped.
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
            .map(|c| {
                if SCRATCHPAD_KEYS.contains(&c) {
                    Action::Scratchpad
                } else {
                    Action::Machine(Command::Type(c))
                }
            })
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
    let once = |action| (!repeat).then_some(action);
    if m.shift
        && let Some(direction) = arrow(key)
    {
        // No repeat: held, it would shuffle a sheet through the whole folder.
        return once(Action::ShiftArrow(direction));
    }
    match key {
        Key::PageUp => Some(Action::PageUp),
        Key::PageDown => Some(Action::PageDown),
        Key::Escape => once(Action::Escape),
        Key::F11 => once(Action::Fullscreen),
        Key::F4 => once(Action::NextCorrection),
        Key::Delete => once(Action::Delete),
        Key::S if m.command => once(Action::Save),
        _ => key_command(key, m, repeat).map(Action::Machine),
    }
}

fn key_command(key: Key, m: Modifiers, repeat: bool) -> Option<Command> {
    if let Some(direction) = arrow(key) {
        return Some(Command::Move(direction));
    }
    if repeat {
        // Held Return winds the platen knob.
        return (key == Key::Enter).then_some(Command::LineFeed);
    }
    let command = match key {
        Key::Insert => Command::FeedSheet,
        Key::Enter => Command::Return,
        Key::Backspace if m.shift => Command::Erase,
        Key::Backspace => Command::Backspace,
        Key::Tab if !m.any() => Command::Tab,
        Key::F1 => Command::SetLineSpacing(LineSpacing::Single),
        Key::F2 => Command::SetLineSpacing(LineSpacing::OneAndHalf),
        Key::F3 => Command::SetLineSpacing(LineSpacing::Double),
        _ => return None,
    };
    Some(command)
}

fn arrow(key: Key) -> Option<Direction> {
    Some(match key {
        Key::ArrowLeft => Direction::Left,
        Key::ArrowRight => Direction::Right,
        Key::ArrowUp => Direction::Up,
        Key::ArrowDown => Direction::Down,
        _ => return None,
    })
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
    fn ctrl_s_saves() {
        let actions = |k, m| Input::default().actions(&[key(k, m)], false);
        assert_eq!(actions(Key::S, Modifiers::COMMAND), [Action::Save]);
        assert_eq!(actions(Key::S, Modifiers::NONE), []);
    }

    #[test]
    fn text_types_each_printable_char() {
        assert_eq!(
            one_frame(&[Event::Text("a b\r".into())]),
            [Command::Type('a'), Command::Type(' '), Command::Type('b')]
        );
    }

    #[test]
    fn one_and_exclamation_open_the_scratchpad() {
        assert_eq!(
            Input::default().actions(&[Event::Text("1!l".into())], false),
            [
                Action::Scratchpad,
                Action::Scratchpad,
                Action::Machine(Command::Type('l'))
            ]
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
            Input::default().actions(&[key(Key::Delete, Modifiers::NONE)], false),
            [Action::Delete]
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
                key(Key::F4, Modifiers::NONE),
            ],
            false,
        );
        assert_eq!(
            actions,
            [
                Action::PageUp,
                Action::PageDown,
                Action::Escape,
                Action::Fullscreen,
                Action::NextCorrection,
            ]
        );
    }

    #[test]
    fn held_escape_f4_and_f11_act_once() {
        let actions = Input::default().actions(
            &[
                held(Key::Escape, Modifiers::NONE),
                held(Key::F11, Modifiers::NONE),
                held(Key::F4, Modifiers::NONE),
            ],
            false,
        );
        assert!(actions.is_empty());
    }
}
