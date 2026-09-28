//! Carriage position, margin and tab stops, and platen line spacing.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineSpacing {
    #[default]
    #[serde(rename = "1")]
    Single,
    #[serde(rename = "1.5")]
    OneAndHalf,
    #[serde(rename = "2")]
    Double,
}

impl LineSpacing {
    pub fn half_lines(self) -> u16 {
        match self {
            Self::Single => 2,
            Self::OneAndHalf => 3,
            Self::Double => 4,
        }
    }

    /// The next notch of the line-space lever, wrapping from 2 back to 1.
    pub fn next(self) -> Self {
        match self {
            Self::Single => Self::OneAndHalf,
            Self::OneAndHalf => Self::Double,
            Self::Double => Self::Single,
        }
    }
}

/// How far from the carriage, in columns, clearing a tab stop reaches.
pub const TAB_CLEAR_VICINITY: u16 = 3;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Carriage {
    /// Column the next strike lands on. May equal the page width, which means
    /// the carriage has run off the right edge of the paper.
    pub column: u16,
    pub half_line: u16,
    pub left_margin: u16,
    /// First column the carriage locks at.
    pub right_margin: u16,
    /// One-shot: lets the carriage pass both margins until the next return.
    pub margin_released: bool,
    pub line_spacing: LineSpacing,
    tab_stops: BTreeSet<u16>,
}

impl Carriage {
    /// Fits a sheet `columns` wide and `half_lines` tall, e.g. after loading.
    pub fn fits(&self, columns: u16, half_lines: u16) -> bool {
        self.column <= columns
            && self.half_line < half_lines
            && self.left_margin < self.right_margin
            && self.right_margin <= columns
            && self.tab_stops.iter().all(|&stop| stop < columns)
    }

    pub fn new(left_margin: u16, right_margin: u16, top_half_line: u16) -> Self {
        Self {
            column: left_margin,
            half_line: top_half_line,
            left_margin,
            right_margin,
            margin_released: false,
            line_spacing: LineSpacing::default(),
            tab_stops: BTreeSet::new(),
        }
    }

    pub fn tab_stops(&self) -> impl Iterator<Item = u16> + '_ {
        self.tab_stops.iter().copied()
    }

    pub fn set_tab_stop(&mut self, column: u16) {
        self.tab_stops.insert(column);
    }

    /// Removes the stop closest to the carriage, if one lies within
    /// [`TAB_CLEAR_VICINITY`] columns. Returns the column that was cleared.
    pub fn clear_nearest_tab_stop(&mut self) -> Option<u16> {
        let low = self.column.saturating_sub(TAB_CLEAR_VICINITY);
        let high = self.column.saturating_add(TAB_CLEAR_VICINITY);
        let nearest = self
            .tab_stops
            .range(low..=high)
            .min_by_key(|&&stop| stop.abs_diff(self.column))
            .copied()?;
        self.tab_stops.remove(&nearest);
        Some(nearest)
    }

    pub fn clear_all_tab_stops(&mut self) {
        self.tab_stops.clear();
    }

    pub fn next_tab_stop(&self) -> Option<u16> {
        self.tab_stops
            .range(self.column.saturating_add(1)..)
            .next()
            .copied()
    }

    /// The bell is tripped by the carriage passing a fixed point, so it rings
    /// whenever a forward move crosses it, not once per line.
    pub fn crosses_bell(&self, from: u16, to: u16, bell_columns_before_margin: u16) -> bool {
        let bell = self.right_margin.saturating_sub(bell_columns_before_margin);
        from < bell && to >= bell
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spacing_in_half_lines() {
        assert_eq!(LineSpacing::Single.half_lines(), 2);
        assert_eq!(LineSpacing::OneAndHalf.half_lines(), 3);
        assert_eq!(LineSpacing::Double.half_lines(), 4);
    }

    #[test]
    fn line_spacing_lever_cycles() {
        let mut spacing = LineSpacing::Single;
        let mut seen = vec![];
        for _ in 0..4 {
            spacing = spacing.next();
            seen.push(spacing.half_lines());
        }
        assert_eq!(seen, [3, 4, 2, 3]);
    }

    #[test]
    fn next_tab_stop_is_strictly_after_the_carriage() {
        let mut c = Carriage::new(10, 72, 12);
        c.set_tab_stop(10);
        c.set_tab_stop(20);
        c.set_tab_stop(30);
        assert_eq!(c.next_tab_stop(), Some(20));
        c.column = 20;
        assert_eq!(c.next_tab_stop(), Some(30));
    }

    #[test]
    fn clearing_removes_the_nearest_stop_within_reach() {
        let mut c = Carriage::new(10, 72, 12);
        c.set_tab_stop(20);
        c.set_tab_stop(26);
        c.column = 24;
        assert_eq!(c.clear_nearest_tab_stop(), Some(26));
        assert_eq!(c.clear_nearest_tab_stop(), None);
        c.column = 22;
        assert_eq!(c.clear_nearest_tab_stop(), Some(20));
    }

    #[test]
    fn clearing_prefers_the_stop_behind_on_a_tie() {
        let mut c = Carriage::new(10, 72, 12);
        c.set_tab_stop(20);
        c.set_tab_stop(24);
        c.column = 22;
        assert_eq!(c.clear_nearest_tab_stop(), Some(20));
    }

    #[test]
    fn bell_trips_only_when_crossing_forward() {
        let c = Carriage::new(10, 72, 12);
        assert!(c.crosses_bell(63, 64, 8));
        assert!(!c.crosses_bell(64, 65, 8));
        assert!(!c.crosses_bell(62, 63, 8));
        assert!(c.crosses_bell(20, 72, 8));
    }
}
