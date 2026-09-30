//! The desktop's file dialogs, on a thread so the window keeps drawing.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use eframe::egui;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialog {
    SaveAs,
    Open,
}

/// A dialog's answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Picked {
    SaveAs(PathBuf),
    Open(PathBuf),
    Cancelled,
}

/// The dialog open, if one is.
#[derive(Default)]
pub struct Picker {
    open: Option<(Dialog, Receiver<Option<PathBuf>>)>,
}

impl Picker {
    /// Opens `dialog` at `directory`, suggesting `file_name` to save as. One
    /// at a time: asked again while open, nothing happens.
    pub fn ask(
        &mut self,
        dialog: Dialog,
        directory: Option<PathBuf>,
        file_name: String,
        ctx: &egui::Context,
    ) {
        if self.open.is_some() {
            return;
        }
        let (sender, receiver) = mpsc::channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let mut picker = rfd::FileDialog::new().add_filter("Typewriter project", &["typr"]);
            if let Some(directory) = directory {
                picker = picker.set_directory(directory);
            }
            let picked = match dialog {
                Dialog::SaveAs => picker
                    .set_title("Save the project as")
                    .set_file_name(file_name)
                    .save_file(),
                Dialog::Open => picker.set_title("Open a project").pick_file(),
            };
            let _ = sender.send(picked);
            ctx.request_repaint();
        });
        self.open = Some((dialog, receiver));
    }

    /// The dialog's answer, once it has closed.
    pub fn picked(&mut self) -> Option<Picked> {
        let (dialog, receiver) = self.open.as_ref()?;
        let picked = match receiver.try_recv() {
            Err(TryRecvError::Empty) => return None,
            Ok(picked) => picked,
            Err(TryRecvError::Disconnected) => None,
        };
        let dialog = *dialog;
        self.open = None;
        Some(match (picked, dialog) {
            (Some(path), Dialog::SaveAs) => Picked::SaveAs(path),
            (Some(path), Dialog::Open) => Picked::Open(path),
            (None, _) => Picked::Cancelled,
        })
    }

    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }
}
