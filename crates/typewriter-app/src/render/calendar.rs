//! The writing log: a month tent calendar standing beside the folder, each
//! day's words pencilled in. Clicked, it comes up close.

use std::collections::BTreeMap;

use eframe::egui::epaint::Shadow;
use eframe::egui::{
    Align2, Color32, CursorIcon, FontFamily, FontId, Id, Painter, Pos2, Rect, Shape, Stroke,
    StrokeKind, Ui, Vec2, pos2, vec2,
};
use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{Timestamp, ToSpan};
use typewriter_core::WritingLog;

use super::note::{GRAPHITE, PENCIL_FAMILY};
use super::scratchpad::{COVER_FAMILY, STAPLE};
use super::{CLICK, HIGHLIGHT, SHEET, SHEET_EDGE, chevron, smoothstep};

/// A desk tent calendar's face, 6 × 4.5 in.
pub const CALENDAR_INCHES: Vec2 = vec2(6.0, 4.5);
pub const SLIDE_SECONDS: f32 = 0.25;
/// Wire loops along the top.
const LOOPS: usize = 24;
/// Rows of weeks: enough for any month.
const WEEKS: usize = 6;
const WEEKDAYS: [&str; 7] = ["M", "T", "W", "T", "F", "S", "S"];

const PRINT: Color32 = Color32::from_rgb(0x3A, 0x34, 0x2E);
const SUNDAY: Color32 = Color32::from_rgb(0xA8, 0x3A, 0x2A);
const RULE: Color32 = Color32::from_rgba_premultiplied(0x50, 0x48, 0x3C, 0x40);

/// The local day of Unix `seconds`.
pub fn local_day(seconds: u64) -> Date {
    day_in(seconds, &TimeZone::system())
}

fn day_in(seconds: u64, tz: &TimeZone) -> Date {
    let at = i64::try_from(seconds)
        .ok()
        .and_then(|s| Timestamp::from_second(s).ok())
        .unwrap_or(Timestamp::MAX);
    tz.to_datetime(at).date()
}

pub fn today() -> Date {
    jiff::Zoned::now().date()
}

/// A month of the log, as the calendar shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct Month {
    /// Its first day.
    pub first: Date,
    pub today: Date,
    /// This month's days written on, with their words.
    pub days: BTreeMap<Date, i64>,
    /// Turning back reaches the first day's month.
    pub earlier: bool,
    /// Turning on stops at today's month.
    pub later: bool,
}

impl Month {
    /// The month `back` months before today's.
    pub fn of(log: &WritingLog, back: u32, today: Date) -> Self {
        let this_month = today.first_of_month();
        let oldest = log
            .days()
            .keys()
            .next()
            .map_or(this_month, |oldest| oldest.first_of_month().min(this_month));
        let back = back.min(months_between(oldest, this_month));
        let first = this_month
            .checked_sub(i64::from(back).months())
            .unwrap_or(oldest);
        let days = log
            .days()
            .range(first..=first.last_of_month())
            .map(|(&day, &words)| (day, words))
            .collect();
        Self {
            first,
            today,
            days,
            earlier: first > oldest,
            later: back > 0,
        }
    }

    /// Day `date` as `(week, weekday)`, Monday first; `None` if not this month's.
    fn cell(&self, date: Date) -> Option<(usize, usize)> {
        (date.first_of_month() == self.first).then(|| {
            let offset = self.first.weekday().to_monday_zero_offset();
            // Safe cast: an offset and a day of the month, both small positives.
            let index = (offset + date.day() - 1) as usize;
            (index / 7, index % 7)
        })
    }

    fn dates(&self) -> impl Iterator<Item = Date> + '_ {
        (0..self.first.days_in_month())
            .filter_map(|d| self.first.checked_add(i64::from(d).days()).ok())
    }
}

fn months_between(from: Date, to: Date) -> u32 {
    let index = |d: Date| i32::from(d.year()) * 12 + i32::from(d.month());
    u32::try_from(index(to) - index(from)).unwrap_or(0)
}

/// What the pointer is over on the calendar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Spot {
    /// Back (-1) or on (1) a month.
    Turn(isize),
    Day(Date),
}

/// Where things lie on a calendar `size` points, from its top-left.
pub struct Layout {
    size: Vec2,
}

impl Layout {
    pub fn new(size: Vec2) -> Self {
        Self { size }
    }

    fn grid(&self) -> Rect {
        Rect::from_min_max(
            pos2(0.04 * self.size.x, 0.27 * self.size.y),
            pos2(0.96 * self.size.x, 0.96 * self.size.y),
        )
    }

    fn cell(&self, week: usize, weekday: usize) -> Rect {
        let grid = self.grid();
        let size = vec2(grid.width() / 7.0, grid.height() / WEEKS as f32);
        Rect::from_min_size(
            grid.min + vec2(weekday as f32 * size.x, week as f32 * size.y),
            size,
        )
    }

    fn turn_button(&self, direction: isize) -> Rect {
        let x = if direction < 0 { 0.08 } else { 0.92 };
        Rect::from_center_size(
            pos2(x * self.size.x, 0.13 * self.size.y),
            Vec2::splat(0.1 * self.size.y),
        )
    }

    /// Everything the pointer can be over, as rectangles on the calendar.
    pub fn spots(&self, month: &Month) -> Vec<(Spot, Rect)> {
        let mut spots = Vec::new();
        for (direction, shown) in [(-1, month.earlier), (1, month.later)] {
            if shown {
                spots.push((Spot::Turn(direction), self.turn_button(direction)));
            }
        }
        for date in month.dates() {
            if let Some((week, weekday)) = month.cell(date) {
                spots.push((Spot::Day(date), self.cell(week, weekday)));
            }
        }
        spots
    }

    /// The calendar, flat: card, wire binding, month, weekdays, grid, days
    /// with their words pencilled in, today ringed. `lit`: the card's edge.
    pub fn paint(
        &self,
        painter: &Painter,
        month: &Month,
        hovered: Option<Spot>,
        lit: bool,
    ) -> Vec<Shape> {
        let (w, h) = (self.size.x, self.size.y);
        let card = Rect::from_min_size(Pos2::ZERO, self.size);
        let edge = if lit { HIGHLIGHT } else { SHEET_EDGE };
        let mut shapes = vec![
            Shape::rect_filled(card, 0.0, SHEET),
            Shape::rect_stroke(
                card,
                0.0,
                Stroke::new((0.004 * w).clamp(0.5, 1.5), edge),
                StrokeKind::Inside,
            ),
        ];
        // Punched holes, each with a wire loop over the top edge.
        let pitch = w / LOOPS as f32;
        let wire = Stroke::new(0.008 * h, STAPLE);
        for i in 0..LOOPS {
            let x = (i as f32 + 0.5) * pitch;
            let hole = pos2(x, 0.03 * h);
            shapes.push(Shape::circle_filled(hole, 0.01 * h, STAPLE));
            for dx in [-0.12, 0.12] {
                let x = x + dx * pitch;
                shapes.push(Shape::line_segment(
                    [pos2(x, hole.y), pos2(x, -0.012 * h)],
                    wire,
                ));
            }
        }
        let print = |em: f32| FontId::new(em * h, FontFamily::Name(COVER_FAMILY.into()));
        let text = |at: Pos2, align: Align2, text: String, font: FontId, color: Color32| {
            let galley = painter.layout_no_wrap(text, font, color);
            let rect = align.anchor_size(at, galley.size());
            Shape::galley(rect.min, galley, color)
        };
        let title = month.first.strftime("%B %Y").to_string().to_uppercase();
        shapes.push(text(
            pos2(w / 2.0, 0.13 * h),
            Align2::CENTER_CENTER,
            title,
            print(0.075),
            PRINT,
        ));
        for (weekday, letter) in WEEKDAYS.iter().enumerate() {
            let color = if weekday == 6 { SUNDAY } else { PRINT };
            let at = pos2(self.cell(0, weekday).center().x, 0.235 * h);
            shapes.push(text(
                at,
                Align2::CENTER_CENTER,
                (*letter).to_owned(),
                print(0.04),
                color,
            ));
        }

        let hair = Stroke::new((0.003 * w).min(0.8), RULE);
        let grid = self.grid();
        for week in 0..=WEEKS {
            let y = grid.top() + week as f32 * grid.height() / WEEKS as f32;
            shapes.push(Shape::line_segment(
                [pos2(grid.left(), y), pos2(grid.right(), y)],
                hair,
            ));
        }
        for weekday in 1..7 {
            let x = grid.left() + weekday as f32 * grid.width() / 7.0;
            shapes.push(Shape::line_segment(
                [pos2(x, grid.top()), pos2(x, grid.bottom())],
                hair,
            ));
        }

        for (spot, rect) in self.spots(month) {
            let lit = hovered == Some(spot);
            match spot {
                Spot::Turn(direction) => {
                    let color = if lit { HIGHLIGHT } else { PRINT };
                    shapes.extend(chevron(rect, direction, Stroke::new(0.01 * h, color)));
                }
                Spot::Day(date) => {
                    if lit && month.days.contains_key(&date) {
                        shapes.push(Shape::rect_filled(
                            rect.shrink(0.5),
                            0.0,
                            HIGHLIGHT.gamma_multiply(0.08),
                        ));
                    }
                    let color = if date.weekday() == jiff::civil::Weekday::Sunday {
                        SUNDAY
                    } else {
                        PRINT
                    };
                    let number =
                        painter.layout_no_wrap(date.day().to_string(), print(0.032), color);
                    let at = Align2::LEFT_CENTER
                        .anchor_size(rect.min + vec2(0.12, 0.2) * rect.height(), number.size());
                    if date == month.today {
                        shapes.push(Shape::circle_stroke(
                            at.center(),
                            0.03 * h,
                            Stroke::new(0.005 * h, GRAPHITE),
                        ));
                    }
                    shapes.push(Shape::galley(at.min, number, color));
                    if let Some(&words) = month.days.get(&date) {
                        shapes.push(text(
                            rect.center() + vec2(0.0, 0.15 * rect.height()),
                            Align2::CENTER_CENTER,
                            thousands(words),
                            FontId::new(0.055 * h, FontFamily::Name(PENCIL_FAMILY.into())),
                            GRAPHITE,
                        ));
                    }
                }
            }
        }
        shapes
    }
}

/// The calendar up close: centred in `view`, `shown` of the way up from
/// below it.
pub fn up_close(view: Rect, shown: f32) -> Rect {
    let aspect = CALENDAR_INCHES.x / CALENDAR_INCHES.y;
    let height = (0.7 * view.height()).min(0.8 * view.width() / aspect);
    let size = vec2(height * aspect, height);
    let up = view.center().y - size.y / 2.0;
    let down = view.bottom() + 0.1 * height;
    let top = down + (up - down) * smoothstep(shown);
    Rect::from_min_size(pos2(view.center().x - size.x / 2.0, top), size)
}

/// What the calendar up close asks for.
#[derive(Debug, Default)]
pub struct UpClose {
    /// Back (-1) or on (1) a month.
    pub turn: Option<isize>,
    /// A click away.
    pub close: bool,
}

/// Draws the calendar up close, `shown` of the way up. Takes the clicks over
/// `view` while `open`.
pub fn show_up_close(ui: &mut Ui, view: Rect, shown: f32, month: &Month, open: bool) -> UpClose {
    let rect = up_close(view, shown);
    let layout = Layout::new(rect.size());
    let spots: Vec<(Spot, Rect)> = layout
        .spots(month)
        .into_iter()
        .map(|(spot, r)| (spot, r.translate(rect.min.to_vec2())))
        .collect();
    let mut asked = UpClose::default();
    let mut hovered = None;
    if open {
        asked.close = ui
            .interact(view, Id::new("writing-log-away"), CLICK)
            .clicked();
        ui.interact(rect, Id::new("writing-log-card"), CLICK);
        for &(spot, r) in &spots {
            let mut response = ui.interact(r, Id::new(("writing-log", spot)), CLICK);
            if response.hovered() {
                hovered = Some(spot);
            }
            if let Some(tip) = tooltip(spot, month) {
                response = response.on_hover_text(tip);
            }
            if let Spot::Turn(direction) = spot {
                if response.hovered() {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                }
                if response.clicked() {
                    asked.turn = Some(direction);
                }
            }
        }
    }
    let painter = ui.painter_at(view);
    painter.rect_filled(
        view,
        0.0,
        Color32::from_black_alpha(0x50).gamma_multiply(shown),
    );
    let shadow = Shadow {
        offset: [0, 6],
        blur: 24,
        spread: 0,
        color: Color32::from_black_alpha(80),
    };
    painter.add(shadow.as_shape(rect, 0));
    for mut shape in layout.paint(&painter, month, hovered, false) {
        shape.translate(rect.min.to_vec2());
        painter.add(shape);
    }
    asked
}

/// A hover text for `spot`, `None` for a day not written on.
pub fn tooltip(spot: Spot, month: &Month) -> Option<String> {
    match spot {
        Spot::Turn(-1) => Some("Previous month (Page Up)".to_owned()),
        Spot::Turn(_) => Some("Next month (Page Down)".to_owned()),
        Spot::Day(date) => month
            .days
            .get(&date)
            .map(|&words| format!("{}\n{}", date.strftime("%A %-d %B"), plural(words, "word"))),
    }
}

/// "12 days · 5,210 words", `""` before the first.
pub fn log_line(log: &WritingLog) -> String {
    let days = log.days().len();
    if days == 0 {
        return String::new();
    }
    // Safe cast: days written on, far below i64::MAX.
    let days = plural(days as i64, "day");
    format!("{days}  \u{b7}  {}", plural(log.words(), "word"))
}

/// "1 word", "1,840 words".
fn plural(n: i64, word: &str) -> String {
    let s = if n == 1 { "" } else { "s" };
    format!("{} {word}{s}", thousands(n))
}

/// 1840 as "1,840".
fn thousands(n: i64) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::date;

    fn log(days: &[(Date, i64)]) -> WritingLog {
        let mut log = WritingLog::default();
        for &(day, words) in days {
            log.add(day, words);
        }
        log
    }

    #[test]
    fn days_are_local() {
        let berlin = TimeZone::get("Europe/Berlin").unwrap();
        // 1 August 2026, 00:30 in Berlin: still 31 July in UTC.
        let seconds = date(2026, 8, 1)
            .at(0, 30, 0, 0)
            .to_zoned(berlin.clone())
            .unwrap()
            .timestamp()
            .as_second() as u64;
        assert_eq!(day_in(seconds, &berlin), date(2026, 8, 1));
        assert_eq!(day_in(seconds, &TimeZone::UTC), date(2026, 7, 31));
    }

    #[test]
    fn the_month_turns_back_to_the_first_day_written_on() {
        let log = log(&[(date(2026, 8, 1), 300), (date(2026, 9, 3), 150)]);
        let today = date(2026, 9, 29);

        let september = Month::of(&log, 0, today);
        assert_eq!(september.first, date(2026, 9, 1));
        assert_eq!(september.today, today);
        assert_eq!(september.days.len(), 1);
        assert_eq!(september.days[&date(2026, 9, 3)], 150);
        assert!(september.earlier && !september.later);

        let august = Month::of(&log, 1, today);
        assert_eq!(august.days[&date(2026, 8, 1)], 300);
        assert!(!august.earlier && august.later);
        // No further back than the first day.
        assert_eq!(Month::of(&log, 5, today), august);
    }

    #[test]
    fn an_empty_log_shows_this_month_alone() {
        let month = Month::of(&WritingLog::default(), 3, date(2026, 9, 29));
        assert_eq!(month.first, date(2026, 9, 1));
        assert!(!month.earlier && !month.later && month.days.is_empty());
    }

    #[test]
    fn weeks_start_on_monday() {
        let month = Month::of(&WritingLog::default(), 0, date(2026, 9, 29));
        // 1 September 2026 is a Tuesday.
        assert_eq!(month.cell(date(2026, 9, 1)), Some((0, 1)));
        assert_eq!(month.cell(date(2026, 9, 7)), Some((1, 0)));
        assert_eq!(month.cell(date(2026, 9, 30)), Some((4, 2)));
        assert_eq!(month.cell(date(2026, 10, 1)), None);
        assert_eq!(month.dates().count(), 30);
    }

    #[test]
    fn the_last_days_of_a_long_month_fit_the_grid() {
        let month = Month::of(&WritingLog::default(), 0, date(2026, 8, 15));
        // August 2026 starts on a Saturday: six rows.
        assert_eq!(month.cell(date(2026, 8, 31)), Some((5, 0)));
        let layout = Layout::new(vec2(500.0, 425.0));
        let spots = layout.spots(&month);
        assert_eq!(spots.len(), 31);
        assert!(spots.iter().all(|(_, rect)| rect.max.y <= 425.0));
    }

    #[test]
    fn up_close_it_rises_from_below_into_the_middle() {
        let view = Rect::from_min_size(Pos2::ZERO, vec2(1200.0, 800.0));
        assert!(up_close(view, 0.0).top() > view.bottom());
        let shown = up_close(view, 1.0);
        assert!(view.contains_rect(shown));
        assert!((shown.center() - view.center()).length() < 1.0);
        let narrow = Rect::from_min_size(Pos2::ZERO, vec2(500.0, 900.0));
        assert!(narrow.contains_rect(up_close(narrow, 1.0)));
    }

    #[test]
    fn the_log_adds_up_in_words() {
        assert_eq!(log_line(&WritingLog::default()), "");
        let two = log(&[(date(2026, 9, 1), 1_900), (date(2026, 9, 2), -60)]);
        assert_eq!(log_line(&two), "2 days  \u{b7}  1,840 words");
        let one = log(&[(date(2026, 9, 1), -1_234_567)]);
        assert_eq!(log_line(&one), "1 day  \u{b7}  -1,234,567 words");
    }

    #[test]
    fn a_day_tooltip_names_the_day() {
        let month = Month::of(&log(&[(date(2026, 9, 29), 1)]), 0, date(2026, 9, 30));
        let tip = tooltip(Spot::Day(date(2026, 9, 29)), &month).unwrap();
        assert_eq!(tip, "Tuesday 29 September\n1 word");
        assert_eq!(tooltip(Spot::Day(date(2026, 9, 28)), &month), None);
    }
}
