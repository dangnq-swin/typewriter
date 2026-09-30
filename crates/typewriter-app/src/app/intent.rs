//! What the user asks of the desk, and what the desk asks of the window.

use std::path::PathBuf;

use typewriter_core::{Event, Side};

use super::desk::Answer;
use crate::input::Action;
use crate::picker::{Dialog, Picked};
use crate::render::folder::FolderAction;
use crate::settings::Settings;

/// Everything the user can ask, by key or pointer.
#[derive(Debug, Clone, PartialEq)]
pub enum Intent {
    /// One frame's keys, and the wheel turned `wheel` points (up positive):
    /// over a platen knob it rolls the paper, else it zooms.
    Input {
        keys: Vec<Action>,
        wheel: f32,
        over_knob: bool,
    },
    /// A platen knob dragged `points`, down positive; `per_notch` rolls a
    /// half-line.
    DragKnob {
        points: f32,
        per_notch: f32,
    },
    ReleaseKnob,
    OpenFolder,
    OpenScratchpad,
    CloseScratchpad,
    /// Writing in the scratchpad, or turning its leaves, changed the project.
    ScratchpadWritten,
    ToggleCalm,
    OpenSettings,
    CloseSettings,
    /// The settings card was edited, from these.
    SettingsEdited(Box<Settings>),
    NextSpacing,
    ResetZoom,
    NextCorrection,
    NextGoal,
    Save,
    ReleaseMargins,
    MoveMargin {
        side: Side,
        column: u16,
    },
    TakeHolderDown,
    Folder(FolderAction),
    /// A finished sheet clicked open.
    OpenSheet(usize),
    OpenLog,
    CloseLog,
    /// The writing log turned back (-1) or on (1) a month.
    TurnLog(isize),
    /// The open sheet's top margin clicked.
    StartNote,
    NoteWritten {
        sheet: usize,
        note: String,
    },
    ScrunchUp(usize),
    KeepSheet,
    Leave(Answer),
    Picked(Picked),
    /// A project from outside: the file manager, through another launch.
    OpenFile(PathBuf),
    /// The window's close button.
    CloseWindow,
    /// The desktop switched the window in or out of fullscreen.
    WindowFullscreen(bool),
}

/// What the desk needs the window to do.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    Sound(Sound),
    /// Something blocked: the platen jolts.
    Jolt,
    /// Finished sheet `index` is gone: crumple it where it lay.
    Scrunched(usize),
    Ask(Dialog),
    Fullscreen(bool),
    /// Hold the window open: a question comes first.
    CancelClose,
    Close,
    /// The zoom changed, and the page's size on screen with it.
    Zoomed,
    /// Another project is in: perhaps another machine, pitch and paper.
    MachineChanged,
    /// Sound, look or fullscreen settings may have changed.
    SettingsChanged,
    /// List the machine profiles again: some may have been added.
    ReloadMachines,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sound {
    Machine(Event),
    Crumple,
    WindIn,
    WindBackClick,
}
