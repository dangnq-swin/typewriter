//! The paper support: a tall chrome strip leaning back behind the platen,
//! in the carriage's middle, pulled up out of its sleeve. The sheet not yet
//! wound round leans on it. Its scale counts single lines to the page's
//! end, 1 at the top: the last mark above the sheet's top edge in front.
//!
//! Drawn across at the sheet's scale, as the carriage is: it travels with
//! the sheet.
//!
//! The sheet's way behind the platen is a line across the machine, `(y, z)`
//! inches, measured `along` it from where the paper leaves the platen.

use std::f32::consts::PI;

use eframe::egui::{Align2, Color32, Painter, Rect};

use super::carriage::{PLATEN_DIAMETER_INCHES, STRIKE_DEGREES, platen_axis};
use super::eye::{Eye, paint_flat_text};
use super::geometry::dot;
use super::light::{matte, toward_light};
use super::{CHROME, EDGE, ENGRAVED, IVORY_LIT, METAL};
use typewriter_app::draw::{Metrics, PaperTable, smoothstep};

/// Back from upright.
const LEAN_DEGREES: f32 = 15.0;
/// Across: the sleeve, the strip sliding in it, the scale's white panel.
const SLEEVE_HALF: f32 = 0.26;
const STRIP_HALF: f32 = 0.21;
const PANEL_HALF: f32 = 0.13;
/// Behind the sheet: the sleeve round the strip in front of it.
const SLEEVE_BEHIND: f32 = 0.03;
const STRIP_BEHIND: f32 = 0.05;
/// Its foot, hidden behind the platen, below where the paper leaves it.
const FOOT: f32 = -0.5;
/// Marks on the strip, the last just clear of the sleeve; one numbered each
/// five, and the first.
const MARKS: u16 = 20;
const NUMBERED: u16 = 5;
/// Above the first mark: the panel's end, then the strip's, bent into a tab.
const PANEL_OVER: f32 = 0.1;
const TAB_OVER: f32 = 0.3;
const TAB_HOLE: f32 = 0.09;
/// Tick lengths across, short and numbered; the ticks' width and the
/// numbers' height.
const TICK_HALF: f32 = 0.06;
const NUMBERED_TICK_HALF: f32 = 0.1;
const TICK_WIDTH: f32 = 0.018;
const NUMBER_HEIGHT: f32 = 0.09;
/// Half the gap in a numbered tick.
const NUMBER_GAP: f32 = 0.07;
/// Paper's back: lit over an ambient floor, never past white.
const PAPER_AMBIENT: f32 = 0.74;
const PAPER_LIT: f32 = 0.3;
/// Where it goes down past the platen: how dark, and from how far above.
const PLATEN_SHADE: f32 = 0.55;
const PLATEN_SHADE_INCHES: f32 = 0.4;
/// Past the window's top, where a sheet slid in by hand starts.
const OUT_OF_SIGHT: f32 = 30.0;
/// Searching along the way: how far at most, and how finely.
const WAY_LONGEST: f32 = 40.0;
const SEARCH_STEPS: u16 = 30;

/// Paper round the platen from where it leaves for the support, down
/// behind, under and up its front to the printing point.
fn wrap_inches() -> f32 {
    let turn = PI - LEAN_DEGREES.to_radians() + STRIKE_DEGREES.to_radians();
    PLATEN_DIAMETER_INCHES / 2.0 * turn
}

/// The paper's front's normal behind the platen, `(y, z)`.
fn facing() -> [f32; 2] {
    let (sin, cos) = LEAN_DEGREES.to_radians().sin_cos();
    [cos, sin]
}

/// The point `along` the way, `(y, z)`: it leaves the platen where a line
/// leaning back touches it.
fn way(along: f32) -> [f32; 2] {
    let (sin, cos) = LEAN_DEGREES.to_radians().sin_cos();
    let radius = PLATEN_DIAMETER_INCHES / 2.0;
    let normal = facing();
    let [_, axis_y, axis_z] = platen_axis();
    let leaves = [axis_y - radius * normal[0], axis_z - radius * normal[1]];
    [leaves[0] - along * sin, leaves[1] + along * cos]
}

/// `across` inches from `middle`, `along` the way, `behind` the paper.
fn on_way(middle: f32, across: f32, along: f32, behind: f32) -> [f32; 3] {
    let ([y, z], [ny, nz]) = (way(along), facing());
    [middle + across, y - behind * ny, z - behind * nz]
}

/// Along the way where it shows at screen height `y`, in the carriage's
/// middle: higher on screen further along.
fn along_at(eye: &Eye, y: f32) -> f32 {
    let (mut low, mut high) = (0.0, WAY_LONGEST);
    for _ in 0..SEARCH_STEPS {
        let mid = (low + high) / 2.0;
        if eye.at(on_way(0.0, 0.0, mid, 0.0)).y > y {
            low = mid;
        } else {
            high = mid;
        }
    }
    high
}

/// Where sheets go in, for the typing line at `typing_y` in `view`.
pub fn paper_table(view: Rect, metrics: &Metrics, typing_y: f32) -> PaperTable {
    let eye = Eye::new(view, metrics, typing_y).flat_across();
    let radius = PLATEN_DIAMETER_INCHES / 2.0 * metrics.points_per_inch;
    let [ny, nz] = facing();
    let lit = dot([0.0, ny, nz], toward_light()).max(0.0);
    let shade = (PAPER_AMBIENT + PAPER_LIT * lit).min(1.0);
    let axis_y = eye.at(platen_axis()).y;
    let platen_top = axis_y - radius;
    PaperTable {
        wrap_inches: wrap_inches(),
        seen_inches: along_at(&eye, view.top() - OUT_OF_SIGHT),
        platen_axis_y: axis_y,
        platen_radius: radius,
        place: Box::new(move |x, along| {
            let across = (x - eye.origin.x) / eye.ppi;
            let at = eye.at(on_way(across, 0.0, along, 0.0));
            // Darker going down past the platen, in its shade.
            let above = (platen_top - at.y) / (PLATEN_SHADE_INCHES * eye.ppi);
            let shaded = PLATEN_SHADE + (1.0 - PLATEN_SHADE) * smoothstep(above);
            (at, shade * shaded)
        }),
    }
}

/// The support centred `middle` inches across, its scale for a sheet
/// `metrics` long: the sleeve, the strip out of it, the scale and the tab.
pub(super) fn paint(painter: &Painter, eye: &Eye, metrics: &Metrics, middle: f32) {
    let eye = &eye.flat_across();
    let [ny, nz] = facing();
    let normal = [0.0, ny, nz];
    let ppi = metrics.points_per_inch;
    let length = metrics.paper_size.y / ppi;
    let line = (metrics.cell_offset(2, 0).y - metrics.cell_offset(0, 0).y) / ppi;
    // Mark `n`: where the sheet's top edge in front stands with `n` lines
    // left, seen past it.
    let mark = |n: u16| {
        let front = length - f32::from(n) * line;
        along_at(eye, eye.origin.y - front * ppi)
    };
    let (first, last) = (mark(1), mark(MARKS));
    let sleeve_top = last - line;
    let quad = |half: f32, from: f32, to: f32, behind: f32| {
        [
            on_way(middle, -half, from, behind),
            on_way(middle, half, from, behind),
            on_way(middle, half, to, behind),
            on_way(middle, -half, to, behind),
        ]
    };
    // The strip, then the sleeve round it, in front.
    let strip = quad(STRIP_HALF, sleeve_top, first + TAB_OVER, STRIP_BEHIND);
    eye.fill(painter, &strip, |_| matte(CHROME, normal));
    eye.outline(painter, &strip);
    let panel = quad(PANEL_HALF, sleeve_top, first + PANEL_OVER, STRIP_BEHIND);
    eye.fill(painter, &panel, |_| matte(IVORY_LIT, normal));
    let hole = quad(
        TAB_HOLE / 2.0,
        first + TAB_OVER - 1.5 * TAB_HOLE,
        first + TAB_OVER - 0.5 * TAB_HOLE,
        STRIP_BEHIND,
    );
    eye.fill(painter, &hole, |_| Color32::from_black_alpha(160));
    for n in 1..=MARKS {
        let along = mark(n);
        let numbered = n == 1 || n % NUMBERED == 0;
        let half = if numbered {
            NUMBERED_TICK_HALF
        } else {
            TICK_HALF
        };
        if !numbered {
            let tick = [-half, half].map(|a| on_way(middle, a, along, STRIP_BEHIND));
            eye.line(painter, &tick, TICK_WIDTH, ENGRAVED);
            continue;
        }
        // The number stands in a gap in its tick.
        for side in [-1.0, 1.0] {
            let piece = [half, NUMBER_GAP].map(|a| on_way(middle, side * a, along, STRIP_BEHIND));
            eye.line(painter, &piece, TICK_WIDTH, ENGRAVED);
        }
        paint_flat_text(
            painter,
            eye,
            &n.to_string(),
            NUMBER_HEIGHT,
            ENGRAVED,
            Align2::CENTER_CENTER,
            |across, down| on_way(middle, across, along - down, STRIP_BEHIND),
        );
    }
    let sleeve = quad(SLEEVE_HALF, FOOT, sleeve_top, SLEEVE_BEHIND);
    let brushed = matte(METAL.lerp_to_gamma(CHROME, 0.5), normal);
    eye.fill(painter, &sleeve, |_| brushed);
    eye.outline(painter, &sleeve);
    let lip = [
        on_way(middle, -SLEEVE_HALF, sleeve_top, SLEEVE_BEHIND),
        on_way(middle, SLEEVE_HALF, sleeve_top, SLEEVE_BEHIND),
    ];
    eye.line(painter, &lip, 0.02, EDGE);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_way_leaves_the_platen_behind_and_below_its_top() {
        let [y, z] = way(0.0);
        let radius = PLATEN_DIAMETER_INCHES / 2.0;
        let [_, axis_y, axis_z] = platen_axis();
        assert!(y < axis_y && z < axis_z, "{y} {z}");
        let from_axis = ((y - axis_y).powi(2) + (z - axis_z).powi(2)).sqrt();
        assert!((from_axis - radius).abs() < 1e-4);
    }

    #[test]
    fn the_scale_counts_down_toward_the_strip_s_top() {
        let eye = Eye::testing(500.0, 96.0);
        let at = |y| along_at(&eye, y);
        // Higher on screen: further along, up the strip.
        assert!(at(100.0) > at(300.0));
        let y = eye.at(on_way(0.0, 0.0, at(250.0), 0.0)).y;
        assert!((y - 250.0).abs() < 0.5, "{y}");
    }
}
