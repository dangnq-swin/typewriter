//! Printing: sheets as a PDF in the app's print folder, opened in the
//! desktop's PDF viewer, whose print dialog does the rest.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};

use anyhow::Context as _;
use eframe::egui::Context;
use typewriter_core::Profile;
use typewriter_core::page::Page;

use crate::render::pdf;
use crate::storage;

/// Openings that failed, told once each.
pub struct Printing {
    failed: Sender<String>,
    failures: Receiver<String>,
}

impl Default for Printing {
    fn default() -> Self {
        let (failed, failures) = mpsc::channel();
        Self { failed, failures }
    }
}

impl Printing {
    /// Opens `sheets` (each with its place in the folder) for printing,
    /// titled `title`. What to tell.
    pub fn print(
        &self,
        ctx: &Context,
        profile: &Profile,
        sheets: &[(usize, &Page)],
        title: &str,
        ink_realism: bool,
    ) -> String {
        let opened = pdf::render_sheets(profile, sheets, title, ink_realism).and_then(|bytes| {
            let path = fresh_path(title)?;
            storage::write_atomic(&path, &bytes).context("writing the PDF")?;
            self.open(ctx, &path)
        });
        match opened {
            Ok(()) => "Opened in the PDF viewer, to print".to_owned(),
            Err(err) => format!("Could not print: {err:#}"),
        }
    }

    /// A viewer that could not be started for the last printout, if one
    /// failed.
    pub fn failure(&self) -> Option<String> {
        self.failures.try_recv().ok()
    }

    fn open(&self, ctx: &Context, path: &Path) -> anyhow::Result<()> {
        // Ask the desktop, off the UI thread: the opener can take a moment
        // to find the viewer.
        let (failed, ctx, path) = (self.failed.clone(), ctx.clone(), path.to_path_buf());
        std::thread::spawn(move || {
            if let Err(err) = open::that(&path) {
                let _ = failed.send(format!("Could not print: no PDF viewer opened it ({err})"));
                ctx.request_repaint();
            }
        });
        Ok(())
    }
}

/// `title`.pdf in the print folder, emptied of earlier printouts first:
/// their viewers have them open already.
fn fresh_path(title: &str) -> anyhow::Result<PathBuf> {
    anyhow::ensure!(
        storage::is_file_name(title),
        "the project's name is not a file name"
    );
    let dir = storage::data_dir()
        .context("no data folder to put the PDF in")?
        .join("print");
    if let Ok(earlier) = fs::read_dir(&dir) {
        for file in earlier.flatten() {
            let _ = fs::remove_file(file.path());
        }
    }
    Ok(dir.join(format!("{title}.pdf")))
}
