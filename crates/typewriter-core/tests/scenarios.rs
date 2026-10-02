//! Whole-line and whole-page scenarios on the Olympia SM9 profile.

#![allow(clippy::unwrap_used)]

use typewriter_core::{BlockReason, Command, Constraints, Event, Profile, Typewriter};

fn sm9() -> Typewriter {
    let profile =
        Profile::from_toml_str(include_str!("../../../profiles/olympia-sm9.toml")).unwrap();
    Typewriter::new(profile, Constraints::default()).unwrap()
}

fn type_str(tw: &mut Typewriter, s: &str) -> Vec<Event> {
    s.chars().flat_map(|c| tw.apply(Command::Type(c))).collect()
}

#[test]
fn full_line_rings_the_bell_eight_before_the_margin_then_locks() {
    let mut tw = sm9();
    let line = "x".repeat(62);
    let events = type_str(&mut tw, &line);

    let bell_at = events.iter().position(|e| *e == Event::Bell).unwrap();
    // The bell sounds right after the strike that moves the carriage onto
    // column 64: the 54th character of a line starting at column 10.
    assert_eq!(events[..bell_at].len(), 54);
    assert_eq!(events.iter().filter(|e| **e == Event::Bell).count(), 1);

    assert_eq!(tw.carriage().column, 72);
    assert_eq!(
        type_str(&mut tw, "y"),
        [Event::Blocked(BlockReason::RightMargin)]
    );
}

#[test]
fn margin_release_lets_you_type_to_the_paper_edge() {
    let mut tw = sm9();
    type_str(&mut tw, &"x".repeat(62));
    tw.apply(Command::MarginRelease);
    let events = type_str(&mut tw, &"y".repeat(10));
    assert!(events.iter().all(|e| matches!(e, Event::KeyStrike('y'))));
    assert_eq!(
        type_str(&mut tw, "z"),
        [Event::Blocked(BlockReason::PaperEdge)]
    );
}

#[test]
fn relaxed_margin_allows_typing_past_it() {
    let mut tw = sm9();
    tw.constraints.lock_at_right_margin = false;
    let events = type_str(&mut tw, &"x".repeat(72));
    assert!(!events.iter().any(|e| matches!(e, Event::Blocked(_))));
    assert_eq!(
        type_str(&mut tw, "x"),
        [Event::Blocked(BlockReason::PaperEdge)]
    );
}

#[test]
fn backspacing_and_retyping_rings_the_bell_again() {
    let mut tw = sm9();
    type_str(&mut tw, &"x".repeat(54));
    tw.apply(Command::Backspace);
    assert_eq!(type_str(&mut tw, "x"), [Event::KeyStrike('x'), Event::Bell]);
}

#[test]
fn page_ends_after_the_last_line() {
    let mut tw = sm9();
    // Single spacing from half-line 12 on a 140 half-line page: 63 returns fit.
    for _ in 0..63 {
        assert_eq!(tw.apply(Command::Return), [Event::CarriageReturn]);
    }
    assert_eq!(tw.carriage().half_line, 138);
    assert_eq!(
        tw.apply(Command::Return),
        [Event::CarriageReturn, Event::PageEnd]
    );
    assert_eq!(tw.carriage().half_line, 138);
}
