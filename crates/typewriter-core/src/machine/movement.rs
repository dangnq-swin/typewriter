//! Moving the carriage and the paper: return, the platen, tabs, margins and
//! free movement.

use super::{BlockReason, Direction, Event, Side, Typewriter};
use crate::carriage::LineSpacing;

impl Typewriter {
    pub(super) fn carriage_return(&mut self) -> Vec<Event> {
        let c = &mut self.carriage;
        c.column = c.left_margin;
        c.margin_released = false;
        if self.roll_one_line() {
            vec![Event::CarriageReturn]
        } else {
            vec![Event::CarriageReturn, Event::PageEnd]
        }
    }

    pub(super) fn line_feed(&mut self) -> Vec<Event> {
        if self.roll_one_line() {
            vec![Event::LineFeed]
        } else {
            vec![Event::PageEnd]
        }
    }

    /// Rolls on by the line spacing. False, not moving, past the bottom.
    fn roll_one_line(&mut self) -> bool {
        let c = &mut self.carriage;
        let next = c.half_line + c.line_spacing.half_lines();
        let fits = next < self.document.current().half_lines();
        if fits {
            c.half_line = next;
        }
        fits
    }

    pub(super) fn platen_notch(&mut self) -> Vec<Event> {
        let c = &mut self.carriage;
        if c.half_line + 1 < self.document.current().half_lines() {
            c.half_line += 1;
            vec![Event::LineFeed]
        } else {
            vec![Event::PageEnd]
        }
    }

    pub(super) fn set_line_spacing(&mut self, spacing: LineSpacing) -> Vec<Event> {
        self.carriage.line_spacing = spacing;
        vec![]
    }

    pub(super) fn tab(&mut self) -> Vec<Event> {
        let (limit, reason) = self.right_limit();
        let column = self.carriage.column;
        if column >= limit {
            return vec![Event::Blocked(reason)];
        }
        // No stop ahead: fly to the margin.
        let target = self
            .carriage
            .next_tab_stop()
            .map_or(self.carriage.right_margin, |stop| stop.min(limit));
        if target <= column {
            return vec![Event::Blocked(reason)];
        }
        let mut events = vec![Event::Tab];
        self.advance_to(target, &mut events);
        events
    }

    pub(super) fn set_tab_stop(&mut self) -> Vec<Event> {
        if self.carriage.column >= self.document.current().columns() {
            return vec![Event::Blocked(BlockReason::InvalidStop)];
        }
        self.carriage.set_tab_stop(self.carriage.column);
        vec![]
    }

    pub(super) fn clear_tab_stop(&mut self) -> Vec<Event> {
        match self.carriage.clear_nearest_tab_stop() {
            Some(_) => vec![],
            None => vec![Event::Blocked(BlockReason::InvalidStop)],
        }
    }

    pub(super) fn clear_all_tab_stops(&mut self) -> Vec<Event> {
        self.carriage.clear_all_tab_stops();
        vec![]
    }

    /// Stops keep at least a column between them, the right one on the paper.
    pub(super) fn move_margin(&mut self, side: Side, column: u16) -> Vec<Event> {
        let columns = self.document.current().columns();
        let c = &mut self.carriage;
        let fits = match side {
            Side::Left => column < c.right_margin,
            Side::Right => column > c.left_margin && column <= columns,
        };
        if !fits {
            return vec![Event::Blocked(BlockReason::InvalidStop)];
        }
        match side {
            Side::Left => c.left_margin = column,
            Side::Right => c.right_margin = column,
        }
        vec![]
    }

    /// Until the next return.
    pub(super) fn release_margins(&mut self) -> Vec<Event> {
        self.carriage.margin_released = true;
        vec![]
    }

    pub(super) fn move_freely(&mut self, direction: Direction) -> Vec<Event> {
        let knob = matches!(direction, Direction::Up | Direction::Down);
        if !knob && !self.constraints.free_movement {
            return vec![Event::Blocked(BlockReason::NotAllowed)];
        }
        let columns = self.document.current().columns();
        let half_lines = self.document.current().half_lines();
        let c = &mut self.carriage;
        let (position, max) = match direction {
            Direction::Left | Direction::Right => (&mut c.column, columns),
            Direction::Up | Direction::Down => (&mut c.half_line, half_lines.saturating_sub(1)),
        };
        let target = match direction {
            Direction::Left | Direction::Up => position.checked_sub(1),
            Direction::Right | Direction::Down => position.checked_add(1).filter(|&p| p <= max),
        };
        match target {
            Some(p) => {
                *position = p;
                // The knob's ratchet clicks.
                if knob { vec![Event::LineFeed] } else { vec![] }
            }
            None => vec![Event::Blocked(BlockReason::PaperEdge)],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::{sm9, type_str};
    use super::super::{BlockReason, Command, Direction, Event, Side, Typewriter};
    use crate::carriage::LineSpacing;

    #[test]
    fn return_goes_to_left_margin_and_feeds_by_spacing() {
        let mut tw = sm9();
        type_str(&mut tw, "abc");
        assert_eq!(tw.apply(Command::Return), [Event::CarriageReturn]);
        assert_eq!((tw.carriage().column, tw.carriage().half_line), (10, 14));
        tw.apply(Command::SetLineSpacing(LineSpacing::OneAndHalf));
        tw.apply(Command::Return);
        assert_eq!(tw.carriage().half_line, 17);
        tw.apply(Command::SetLineSpacing(LineSpacing::Double));
        tw.apply(Command::Return);
        assert_eq!(tw.carriage().half_line, 21);
    }

    #[test]
    fn line_feed_rolls_the_paper_but_not_the_carriage() {
        let mut tw = sm9();
        type_str(&mut tw, "abc");
        assert_eq!(tw.apply(Command::LineFeed), [Event::LineFeed]);
        assert_eq!((tw.carriage().column, tw.carriage().half_line), (13, 14));
        tw.apply(Command::SetLineSpacing(LineSpacing::Double));
        tw.apply(Command::LineFeed);
        assert_eq!(tw.carriage().half_line, 18);
    }

    #[test]
    fn line_feed_stops_at_the_bottom_of_the_sheet() {
        let mut tw = sm9();
        while tw.apply(Command::LineFeed) == [Event::LineFeed] {}
        let last = tw.carriage().half_line;
        assert!(last + 2 >= tw.page().half_lines());
        assert_eq!(tw.apply(Command::LineFeed), [Event::PageEnd]);
        assert_eq!(tw.carriage().half_line, last);
    }

    #[test]
    fn the_platen_stops_at_the_bottom_of_the_sheet() {
        let mut tw = sm9();
        for _ in 0..200 {
            tw.apply(Command::PlatenNotch);
        }
        assert_eq!(tw.carriage().half_line, 139);
        assert_eq!(tw.apply(Command::PlatenNotch), [Event::PageEnd]);
    }

    #[test]
    fn return_resets_margin_release() {
        let mut tw = sm9();
        tw.apply(Command::MarginRelease);
        tw.apply(Command::Return);
        assert!(!tw.carriage().margin_released);
    }

    #[test]
    fn tab_jumps_to_next_stop_or_margin() {
        let mut tw = sm9();
        tw.carriage.set_tab_stop(20);
        assert_eq!(tw.apply(Command::Tab), [Event::Tab]);
        assert_eq!(tw.carriage().column, 20);
        assert_eq!(tw.apply(Command::Tab), [Event::Tab, Event::Bell]);
        assert_eq!(tw.carriage().column, 72);
        assert_eq!(
            tw.apply(Command::Tab),
            [Event::Blocked(BlockReason::RightMargin)]
        );
    }

    #[test]
    fn tab_stops_are_set_and_cleared_at_the_carriage() {
        let mut tw = sm9();
        type_str(&mut tw, "     ");
        tw.apply(Command::SetTabStop);
        assert_eq!(tw.carriage().tab_stops().collect::<Vec<_>>(), [15]);
        type_str(&mut tw, "  ");
        assert_eq!(tw.apply(Command::ClearTabStop), []);
        assert_eq!(tw.carriage().tab_stops().count(), 0);
        assert_eq!(
            tw.apply(Command::ClearTabStop),
            [Event::Blocked(BlockReason::InvalidStop)]
        );
        tw.apply(Command::SetTabStop);
        tw.apply(Command::ClearAllTabStops);
        assert_eq!(tw.carriage().tab_stops().count(), 0);
    }

    #[test]
    fn margins_are_set_at_the_carriage() {
        let mut tw = sm9();
        type_str(&mut tw, "     ");
        tw.apply(Command::SetLeftMargin);
        tw.apply(Command::Return);
        assert_eq!(tw.carriage().column, 15);
        type_str(&mut tw, "     ");
        tw.apply(Command::SetRightMargin);
        assert_eq!(tw.carriage().right_margin, 20);
        assert_eq!(
            type_str(&mut tw, "x"),
            [Event::Blocked(BlockReason::RightMargin)]
        );
    }

    #[test]
    fn margin_cannot_cross_the_other() {
        let mut tw = sm9();
        assert_eq!(
            tw.apply(Command::SetRightMargin),
            [Event::Blocked(BlockReason::InvalidStop)]
        );
    }

    #[test]
    fn stops_slide_along_the_scale_but_never_cross_or_leave_the_paper() {
        let mut tw = sm9();
        let slide =
            |tw: &mut Typewriter, side, column| tw.apply(Command::MoveMargin { side, column });
        assert_eq!(slide(&mut tw, Side::Left, 5), []);
        assert_eq!(slide(&mut tw, Side::Right, 80), []);
        assert_eq!(
            (tw.carriage().left_margin, tw.carriage().right_margin),
            (5, 80)
        );
        let columns = tw.page().columns();
        for (side, column) in [
            (Side::Left, 80),
            (Side::Right, 5),
            (Side::Right, columns + 1),
        ] {
            assert_eq!(
                slide(&mut tw, side, column),
                [Event::Blocked(BlockReason::InvalidStop)]
            );
        }
        assert_eq!(
            (tw.carriage().left_margin, tw.carriage().right_margin),
            (5, 80)
        );
    }

    #[test]
    fn free_movement_is_off_by_default_but_the_platen_knob_always_turns() {
        let mut tw = sm9();
        assert_eq!(
            tw.apply(Command::Move(Direction::Left)),
            [Event::Blocked(BlockReason::NotAllowed)]
        );
        assert_eq!(tw.apply(Command::Move(Direction::Up)), [Event::LineFeed]);
        assert_eq!(tw.apply(Command::Move(Direction::Down)), [Event::LineFeed]);
        assert_eq!(tw.carriage().half_line, 12);
    }

    #[test]
    fn a_half_line_up_types_a_superscript() {
        let mut tw = sm9();
        type_str(&mut tw, "x");
        tw.apply(Command::Move(Direction::Up));
        type_str(&mut tw, "2");
        assert_eq!(tw.page().cell(11, 11).unwrap().top_glyph(), Some('2'));
    }

    #[test]
    fn free_movement_moves_in_columns_and_half_lines() {
        let mut tw = sm9();
        tw.constraints.free_movement = true;
        tw.apply(Command::Move(Direction::Up));
        tw.apply(Command::Move(Direction::Left));
        assert_eq!((tw.carriage().column, tw.carriage().half_line), (9, 11));
        tw.apply(Command::Move(Direction::Down));
        tw.apply(Command::Move(Direction::Right));
        assert_eq!((tw.carriage().column, tw.carriage().half_line), (10, 12));
    }

    #[test]
    fn the_platen_knob_stops_at_the_paper_edge() {
        let mut tw = sm9();
        for _ in 0..12 {
            tw.apply(Command::Move(Direction::Up));
        }
        assert_eq!(
            tw.apply(Command::Move(Direction::Up)),
            [Event::Blocked(BlockReason::PaperEdge)]
        );
    }
}
