//! The scratchpad open: page 1 alone on its folded-back cover, then two
//! pages to a spread, stapled at the fold. Written in pencil; a full page
//! runs on to the next.

use eframe::egui::epaint::Shadow;
use eframe::egui::{
    Color32, CornerRadius, CursorIcon, FontFamily, FontId, Id, Key, Modifiers, Painter, Pos2, Rect,
    Stroke, StrokeKind, Ui, Vec2, pos2, vec2,
};
use typewriter_core::Scratchpad;
use typewriter_core::scratchpad::{self as book, SPREADS};

use super::note::{ASCENT_EM, GRAPHITE, LINE_EM, PENCIL_FAMILY, PencilField, SIZE_INCHES};
use super::scratchpad::{BOOK_INCHES, CORNER_WIDTHS, KRAFT, KRAFT_EDGE, STAPLE};
use super::{CLICK, HIGHLIGHT, smoothstep};

pub const SLIDE_SECONDS: f32 = 0.25;
/// Cover showing round the pages.
const RIM_INCHES: f32 = 0.06;
/// Page edge to text.
const INSET_INCHES: f32 = 0.22;
/// Turn buttons, in the bottom outer corners.
const TURN_INCHES: f32 = 0.16;
/// Book to window edge.
const GAP: f32 = 24.0;
const PAGE: Color32 = Color32::from_rgb(0xFC, 0xFB, 0xF8);
const PAGE_EDGE: Color32 = Color32::from_rgb(0xD8, 0xD4, 0xCA);
const DOT: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 0x40);
const TURN: Color32 = Color32::from_rgba_premultiplied(0x30, 0x30, 0x34, 0x90);

/// The open book on screen.
pub struct Pad {
    /// Left and right halves, each a closed book's size.
    halves: [Rect; 2],
    spread: usize,
    /// At the book's scale.
    points_per_inch: f32,
    font: FontId,
    /// Line to line, and the dot pitch.
    pitch: f32,
    lines: usize,
}

impl Pad {
    /// Open at `spread`, bottom right of `view`, `shown` of the way up (0 is
    /// below the window). True to size, smaller if two pages don't fit.
    pub fn rising(view: Rect, points_per_inch: f32, shown: f32, spread: usize) -> Self {
        let full = vec2(2.0 * BOOK_INCHES.x, BOOK_INCHES.y) * points_per_inch;
        let room = view.size() - Vec2::splat(2.0 * GAP);
        let scale = (room.x / full.x).min(room.y / full.y).clamp(0.0, 1.0);
        let points_per_inch = points_per_inch * scale;
        let half = BOOK_INCHES * points_per_inch;
        let up = view.bottom() - GAP - half.y;
        let down = view.bottom() + GAP;
        let top = down + (up - down) * smoothstep(shown);
        // The right half stays put as leaves turn.
        let right = Rect::from_min_size(pos2(view.right() - GAP - half.x, top), half);
        let font_size = SIZE_INCHES * points_per_inch;
        let pitch = LINE_EM * font_size;
        // A line on every row of dots: the first baseline sits an inset down,
        // the last half a dot pitch above the page's foot.
        let page_height = half.y - 2.0 * RIM_INCHES * points_per_inch;
        let first_baseline = INSET_INCHES * points_per_inch + ASCENT_EM * font_size;
        let below_first = page_height - 0.5 * pitch - first_baseline;
        Self {
            halves: [right.translate(vec2(-half.x, 0.0)), right],
            spread,
            points_per_inch,
            font: FontId::new(font_size, FontFamily::Name(PENCIL_FAMILY.into())),
            pitch,
            // Safe cast: a page's worth of lines.
            lines: (below_first / pitch).floor().max(0.0) as usize + 1,
        }
    }

    /// Page 1 lies alone, its cover folded back behind it.
    fn single(&self) -> bool {
        self.spread == 0
    }

    /// A page's paper on `side` (0 left): the rim shows round the book's
    /// outer edges, not at the fold.
    fn page_rect(&self, side: usize) -> Rect {
        let rim = RIM_INCHES * self.points_per_inch;
        let half = self.halves[side];
        match side {
            _ if self.single() => half.shrink(rim),
            0 => Rect::from_min_max(
                half.min + Vec2::splat(rim),
                half.right_bottom() - vec2(0.0, rim),
            ),
            _ => Rect::from_min_max(
                half.left_top() + vec2(0.0, rim),
                half.max - Vec2::splat(rim),
            ),
        }
    }

    fn text_rect(&self, side: usize) -> Rect {
        let inset = INSET_INCHES * self.points_per_inch;
        let page = self.page_rect(side);
        Rect::from_min_size(
            page.min + Vec2::splat(inset),
            vec2(page.width() - 2.0 * inset, self.pitch * self.lines as f32),
        )
    }

    /// Page `page`'s field, on `side`.
    fn field(&self, side: usize, page: usize) -> PencilField {
        PencilField {
            rect: self.text_rect(side),
            id: Id::new(("scratchpad-page", page)),
            font: self.font.clone(),
            rows: self.lines,
        }
    }

    /// A page on `side`: outer corners round, the fold square.
    fn corners(&self, side: usize) -> CornerRadius {
        let r = self.corner_radius();
        if side == 0 && !self.single() {
            CornerRadius {
                nw: r,
                sw: r,
                ne: 0,
                se: 0,
            }
        } else {
            CornerRadius {
                nw: 0,
                sw: 0,
                ne: r,
                se: r,
            }
        }
    }

    fn corner_radius(&self) -> u8 {
        // Safe cast: a few points.
        (CORNER_WIDTHS * BOOK_INCHES.x * self.points_per_inch) as u8
    }

    /// The buttons that turn a leaf back (-1) or on (1), where there are
    /// leaves to turn.
    fn turn_buttons(&self) -> Vec<(isize, Rect)> {
        let size = Vec2::splat(TURN_INCHES * self.points_per_inch);
        let inset = 0.5 * (INSET_INCHES * self.points_per_inch - size.x);
        let mut buttons = Vec::new();
        if self.spread > 0 {
            let page = self.page_rect(0);
            let at = page.left_bottom() + vec2(inset, -inset - size.y);
            buttons.push((-1, Rect::from_min_size(at, size)));
        }
        if self.spread + 1 < SPREADS {
            let page = self.page_rect(1);
            let at = page.right_bottom() - vec2(inset + size.x, inset + size.y);
            buttons.push((1, Rect::from_min_size(at, size)));
        }
        buttons
    }

    /// Page Up / Page Down or a turn button: back (-1) or on (1).
    pub fn turn_asked(&self, ui: &mut Ui) -> Option<isize> {
        let (back, on) = ui.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::PageUp),
                i.consume_key(Modifiers::NONE, Key::PageDown),
            )
        });
        let mut step = match (back, on) {
            (true, false) => Some(-1),
            (false, true) => Some(1),
            _ => None,
        };
        for (direction, rect) in self.turn_buttons() {
            let tip = if direction < 0 {
                "Previous page (Page Up)"
            } else {
                "Next page (Page Down)"
            };
            let response = ui
                .interact(rect, Id::new(("scratchpad-turn", direction)), CLICK)
                .on_hover_text(tip);
            if response.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
            if response.clicked() {
                step = Some(direction);
            }
        }
        step
    }

    /// The book: shadow, cover, pages and their dots, staples in the fold,
    /// turn buttons.
    pub fn paint(&self, painter: &Painter, pointer: Option<Pos2>) {
        let book = if self.single() {
            self.halves[1]
        } else {
            self.halves[0].union(self.halves[1])
        };
        let outer = if self.single() {
            self.corners(1)
        } else {
            CornerRadius::same(self.corner_radius())
        };
        let shadow = Shadow {
            offset: [0, 4],
            blur: 14,
            spread: 0,
            color: Color32::from_black_alpha(60),
        };
        painter.add(shadow.as_shape(book, outer));
        painter.rect(
            book,
            outer,
            KRAFT,
            Stroke::new(1.0, KRAFT_EDGE),
            StrokeKind::Inside,
        );
        for (side, page) in book::pages_of(self.spread).into_iter().enumerate() {
            if page.is_none() {
                continue; // The inside of the back cover.
            }
            let rect = self.page_rect(side);
            painter.rect(
                rect,
                self.corners(side),
                PAGE,
                Stroke::new(1.0, PAGE_EDGE),
                StrokeKind::Inside,
            );
            self.paint_dots(painter, side);
        }
        if !self.single() {
            let fold = self.halves[1].left();
            let (top, bottom) = (book.top(), book.bottom());
            painter.vline(fold, top..=bottom, Stroke::new(1.0, PAGE_EDGE));
            let staple = Stroke::new((0.012 * book.height()).max(1.0), STAPLE);
            for (from, to) in [(0.12, 0.2), (0.46, 0.54), (0.8, 0.88)] {
                let (from, to) = (top + from * book.height(), top + to * book.height());
                painter.line_segment([pos2(fold, from), pos2(fold, to)], staple);
            }
        }
        for (direction, rect) in self.turn_buttons() {
            let hovered = pointer.is_some_and(|p| rect.contains(p));
            let color = if hovered { HIGHLIGHT } else { TURN };
            let (tip, back) = if direction < 0 {
                (rect.left_center() + vec2(0.25 * rect.width(), 0.0), 0.35)
            } else {
                (rect.right_center() - vec2(0.25 * rect.width(), 0.0), -0.35)
            };
            let arm = vec2(back * rect.width(), 0.3 * rect.height());
            let stroke = Stroke::new(1.5, color);
            painter.line_segment([tip, tip + arm], stroke);
            painter.line_segment([tip, tip + vec2(arm.x, -arm.y)], stroke);
        }
    }

    /// A row of dots on each line's baseline: every row can be written on.
    fn paint_dots(&self, painter: &Painter, side: usize) {
        let page = self.page_rect(side);
        let text = self.text_rect(side);
        let dot = (0.035 * self.pitch).max(0.6);
        let edge = page.shrink(0.5 * self.pitch);
        let first = text.top() + ASCENT_EM * self.font.size;
        for line in 0..self.lines {
            let y = first + line as f32 * self.pitch;
            for x in grid_lines(text.left(), self.pitch, edge.left(), edge.right()) {
                painter.circle_filled(pos2(x, y), dot, DOT);
            }
        }
    }

    /// The spread's writing, not being edited.
    pub fn paint_text(&self, painter: &Painter, book: &Scratchpad) {
        for (side, page) in book::pages_of(self.spread).into_iter().enumerate() {
            if let Some(page) = page {
                let rect = self.text_rect(side);
                let galley = painter.layout(
                    book.page(page).to_owned(),
                    self.font.clone(),
                    GRAPHITE,
                    rect.width(),
                );
                painter.galley(rect.min, galley, GRAPHITE);
            }
        }
    }
}

/// Where the pencil is in the open book.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Writing {
    page: usize,
    /// Give the page's field focus: just opened, turned or run on.
    focus: bool,
    /// Where to put the cursor when it takes focus, in chars.
    cursor: Option<usize>,
}

impl Writing {
    /// At the end of the last written page where the book lies open.
    pub fn open(book: &Scratchpad) -> Self {
        let [left, right] = book::pages_of(book.spread());
        let page = match (left, right) {
            (_, Some(right)) if !book.page(right).is_empty() => right,
            (Some(left), _) => left,
            (None, right) => right.unwrap_or(0),
        };
        Self::at_end(book, page)
    }

    fn at_end(book: &Scratchpad, page: usize) -> Self {
        Self {
            page,
            focus: true,
            cursor: Some(book.page(page).chars().count()),
        }
    }

    /// Turns a leaf back (-1) or on (1). False if there is none.
    pub fn turn(&mut self, book: &mut Scratchpad, step: isize) -> bool {
        let Some(spread) = book.spread().checked_add_signed(step) else {
            return false;
        };
        if !book.open_at(spread) {
            return false;
        }
        let [left, right] = book::pages_of(book.spread());
        // Onwards to the left page, back to the right.
        let page = if step > 0 {
            left.or(right)
        } else {
            right.or(left)
        };
        *self = Self::at_end(book, page.unwrap_or(0));
        true
    }
}

/// What writing did this frame.
#[derive(Debug, Default)]
pub struct Outcome {
    pub changed: bool,
    /// Esc or a click away: close the book.
    pub done: bool,
}

/// One frame of writing on the open spread.
pub fn write(ui: &mut Ui, pad: &Pad, book: &mut Scratchpad, writing: &mut Writing) -> Outcome {
    let mut outcome = Outcome::default();
    let (mut focused, mut lost, mut arrived) = (false, false, false);
    let mut ran_on = None;
    for (side, page) in book::pages_of(pad.spread).into_iter().enumerate() {
        let Some(page) = page else { continue };
        let field = pad.field(side, page);
        let take_focus = writing.focus && page == writing.page;
        if take_focus && let Some(cursor) = writing.cursor.take() {
            field.place_cursor(ui.ctx(), cursor);
        }
        let mut text = book.page(page).to_owned();
        let written = field.show(ui, &mut text, take_focus);
        if written.response.has_focus() {
            focused = true;
            // Moving on: the page left behind may keep focus a frame.
            if !writing.focus {
                writing.page = page;
            } else if page == writing.page {
                arrived = true;
            }
        }
        lost |= written.response.lost_focus();
        if written.galley.rows.len() <= field.rows {
            outcome.changed |= book.write(page, &text);
            continue;
        }
        // Full: the rest runs on to the next page. Past page 48, refused.
        if page + 1 >= book::PAGES {
            continue;
        }
        let kept: usize = written.galley.rows[..field.rows]
            .iter()
            .map(|row| row.char_count_including_newline().0)
            .sum();
        let split = text.char_indices().nth(kept).map_or(text.len(), |(i, _)| i);
        let (head, tail) = text.split_at(split);
        // A newline at the cut is the page break itself.
        let (head, broke) = head
            .strip_suffix('\n')
            .map_or((head, false), |head| (head, true));
        let next = format!("{tail}{}", book.page(page + 1));
        outcome.changed |= book.write(page, head);
        outcome.changed |= book.write(page + 1, &next);
        let moves_on = |cursor: usize| cursor > kept || (broke && cursor == kept);
        if let Some(cursor) = written.cursor.filter(|&cursor| moves_on(cursor)) {
            writing.page = page + 1;
            writing.cursor = Some(cursor - kept);
            outcome.changed |= book.open_at(book::spread_of(page + 1));
            ran_on = Some(pad.field(side, page + 1).id);
        }
    }
    if let Some(id) = ran_on {
        // Hand focus over now, before the next frame's keys.
        ui.memory_mut(|memory| memory.request_focus(id));
        writing.focus = true;
    } else if writing.focus {
        writing.focus = !arrived;
    } else if !focused && !lost {
        // Focus went astray: take it back.
        writing.focus = true;
    }
    outcome.done = !focused && lost && ran_on.is_none();
    outcome
}

/// Every `pitch` through `through`, strictly between `min` and `max`.
fn grid_lines(through: f32, pitch: f32, min: f32, max: f32) -> impl Iterator<Item = f32> {
    let first = through - ((through - min) / pitch).floor() * pitch;
    (0..)
        .map(move |i| first + i as f32 * pitch)
        .skip_while(move |&v| v <= min)
        .take_while(move |&v| v < max)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view() -> Rect {
        Rect::from_min_size(pos2(0.0, 0.0), vec2(1200.0, 900.0))
    }

    #[test]
    fn the_book_hides_below_the_window_and_rises_into_it() {
        let hidden = Pad::rising(view(), 96.0, 0.0, 3);
        assert!(hidden.halves[1].top() > view().bottom());
        let shown = Pad::rising(view(), 96.0, 1.0, 3);
        assert!(view().contains_rect(shown.halves[0].union(shown.halves[1])));
        for side in 0..2 {
            assert!(shown.page_rect(side).contains_rect(shown.text_rect(side)));
        }
        assert!(shown.lines >= 15);
    }

    #[test]
    fn two_pages_fit_a_small_window() {
        let view = Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 300.0));
        let pad = Pad::rising(view, 192.0, 1.0, 5);
        assert!(view.contains_rect(pad.halves[0].union(pad.halves[1])));
    }

    #[test]
    fn the_pages_meet_at_the_fold() {
        let pad = Pad::rising(view(), 96.0, 1.0, 2);
        assert_eq!(pad.page_rect(0).right(), pad.page_rect(1).left());
    }

    #[test]
    fn no_turning_back_from_page_one_or_on_from_page_48() {
        let first = Pad::rising(view(), 96.0, 1.0, 0);
        assert_eq!(
            first.turn_buttons().iter().map(|b| b.0).collect::<Vec<_>>(),
            [1]
        );
        let last = Pad::rising(view(), 96.0, 1.0, SPREADS - 1);
        assert_eq!(
            last.turn_buttons().iter().map(|b| b.0).collect::<Vec<_>>(),
            [-1]
        );
    }

    #[test]
    fn turning_goes_on_to_the_left_page_and_back_to_the_right() {
        let mut book = Scratchpad::default();
        let mut writing = Writing::open(&book);
        assert_eq!(writing.page, 0);
        assert!(!writing.turn(&mut book, -1));
        assert!(writing.turn(&mut book, 1));
        assert_eq!((book.spread(), writing.page), (1, 1));
        assert!(writing.turn(&mut book, 1));
        assert!(writing.turn(&mut book, -1));
        assert_eq!((book.spread(), writing.page), (1, 2));
    }

    #[test]
    fn the_book_opens_at_the_last_written_page_of_its_spread() {
        let mut book = Scratchpad::default();
        book.write(3, "left");
        book.open_at(2);
        assert_eq!(Writing::open(&book).page, 3);
        assert_eq!(Writing::open(&book).cursor, Some(4));
        book.write(4, "right");
        assert_eq!(Writing::open(&book).page, 4);
    }

    /// A window-less egui with the pencil font, typing into an open book.
    struct Desk {
        ctx: eframe::egui::Context,
        book: Scratchpad,
        writing: Writing,
    }

    impl Desk {
        fn open_at(spread: usize) -> Self {
            use eframe::egui::{FontData, FontDefinitions};
            let ctx = eframe::egui::Context::default();
            let mut fonts = FontDefinitions::default();
            fonts.font_data.insert(
                PENCIL_FAMILY.into(),
                std::sync::Arc::new(FontData::from_static(super::super::note::CAVEAT)),
            );
            fonts.families.insert(
                FontFamily::Name(PENCIL_FAMILY.into()),
                vec![PENCIL_FAMILY.into()],
            );
            ctx.set_fonts(fonts);
            let mut book = Scratchpad::default();
            book.open_at(spread);
            let writing = Writing::open(&book);
            let mut desk = Self { ctx, book, writing };
            desk.frame(Vec::new()); // Fonts load.
            desk.frame(Vec::new()); // Focus arrives.
            desk
        }

        fn frame(&mut self, events: Vec<eframe::egui::Event>) {
            let input = eframe::egui::RawInput {
                screen_rect: Some(view()),
                events,
                ..Default::default()
            };
            let (book, writing) = (&mut self.book, &mut self.writing);
            let mut output = self.ctx.run_ui(input, |ui| {
                let pad = Pad::rising(view(), 96.0, 1.0, book.spread());
                let _ = write(ui, &pad, book, writing);
            });
            // No GPU here to take the font atlas.
            output.textures_delta.clear();
        }

        fn key(&mut self, key: Key) {
            self.frame(vec![eframe::egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }]);
        }

        fn text(&mut self, text: &str) {
            self.frame(vec![eframe::egui::Event::Text(text.into())]);
        }

        fn lines(&self) -> usize {
            Pad::rising(view(), 96.0, 1.0, self.book.spread()).lines
        }
    }

    #[test]
    fn every_dotted_line_takes_writing_and_a_full_left_page_runs_on_to_the_right() {
        let mut desk = Desk::open_at(1);
        assert_eq!(desk.writing.page, 1);
        let lines = desk.lines();
        for _ in 1..lines {
            desk.key(Key::Enter);
        }
        desk.text("last");
        assert_eq!(
            desk.writing.page, 1,
            "the last line is still the left page's"
        );
        assert_eq!(desk.book.page(1).lines().last(), Some("last"));
        desk.key(Key::Enter);
        assert_eq!(desk.writing.page, 2);
        assert!(!desk.book.page(1).ends_with('\n'));
        desk.frame(Vec::new());
        desk.text("next");
        assert_eq!(desk.book.page(2), "next");
        assert_eq!(desk.book.page(1).lines().count(), lines);
    }

    #[test]
    fn a_full_right_page_turns_the_leaf() {
        let mut desk = Desk::open_at(1);
        desk.writing = Writing::at_end(&desk.book, 2);
        desk.frame(Vec::new());
        for _ in 0..desk.lines() {
            desk.key(Key::Enter);
        }
        assert_eq!((desk.book.spread(), desk.writing.page), (2, 3));
        desk.frame(Vec::new());
        desk.text("over");
        assert_eq!(desk.book.page(3), "over");
    }

    #[test]
    fn grid_lines_run_through_the_baseline() {
        let lines: Vec<f32> = grid_lines(25.0, 10.0, 0.0, 50.0).collect();
        assert_eq!(lines, [5.0, 15.0, 25.0, 35.0, 45.0]);
        let edge: Vec<f32> = grid_lines(20.0, 10.0, 0.0, 30.0).collect();
        assert_eq!(edge, [10.0, 20.0]);
    }
}
