//! The bundled typefaces, each a named family with egui's fonts behind it.

use std::sync::Arc;

use eframe::egui::{self, FontData, FontDefinitions, FontFamily};

use crate::render::{COURIER_PRIME, FONT_FAMILY, note, notebook};

pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    add_family(
        &mut fonts,
        FONT_FAMILY,
        COURIER_PRIME,
        FontFamily::Monospace,
    );
    add_family(
        &mut fonts,
        note::PENCIL_FAMILY,
        note::CAVEAT,
        FontFamily::Proportional,
    );
    add_family(
        &mut fonts,
        notebook::COVER_FAMILY,
        notebook::JOST,
        FontFamily::Proportional,
    );
    ctx.set_fonts(fonts);
}

/// A named family: `font` first, `fallback`'s fonts behind it for glyphs it
/// lacks.
fn add_family(fonts: &mut FontDefinitions, name: &str, font: &'static [u8], fallback: FontFamily) {
    fonts
        .font_data
        .insert(name.into(), Arc::new(FontData::from_static(font)));
    let mut family = vec![name.to_owned()];
    family.extend(fonts.families.get(&fallback).cloned().unwrap_or_default());
    fonts.families.insert(FontFamily::Name(name.into()), family);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fonts as the app sets them up, loaded.
    fn fonts() -> egui::Context {
        let ctx = egui::Context::default();
        install(&ctx);
        let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
        output.textures_delta.clear();
        ctx
    }

    #[test]
    fn the_interface_font_has_every_symbol_the_interface_writes() {
        let ctx = fonts();
        let font = egui::FontId::proportional(12.0);
        // Symbols in labels, tooltips and notes. ✔ ✕ ↑ ↓ are missing: draw
        // them, or write words.
        let missing: String = ctx.fonts_mut(|fonts| {
            "\u{2026}\u{201c}\u{201d}\u{2013}\u{b7}%"
                .chars()
                .filter(|&c| !fonts.has_glyph(&font, c))
                .collect()
        });
        assert_eq!(missing, "");
        let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
        output.textures_delta.clear();
    }
}
