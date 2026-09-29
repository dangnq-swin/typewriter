//! The machine in a folder file, and back where typing stopped.

use super::Typewriter;
use crate::document::{FORMAT_VERSION, FolderError, FolderFile};
use crate::profile::Profile;
use crate::session::WritingLog;

impl Typewriter {
    /// The project as folder-file RON.
    pub fn to_folder_ron(&self) -> Result<String, FolderError> {
        FolderFile {
            version: FORMAT_VERSION,
            profile: self.profile.name.clone(),
            constraints: self.constraints.clone(),
            carriage: self.carriage.clone(),
            document: self.document.clone(),
            sessions: Vec::new(),
            log: self.log.clone(),
        }
        .to_ron()
    }

    /// Loads a project into its own machine (found by name via `machine`),
    /// where typing stopped. The hand-held slip starts out. `day`: Unix
    /// seconds to their day, for sessions from before the writing log.
    pub fn from_folder_ron(
        text: &str,
        machine: impl FnOnce(&str) -> Option<Profile>,
        day: impl Fn(u64) -> jiff::civil::Date,
    ) -> Result<Self, FolderError> {
        let file = FolderFile::from_ron(text)?;
        let Some(profile) = machine(&file.profile) else {
            return Err(FolderError::UnknownMachine(file.profile));
        };
        let mut machine = Self::new(profile, file.constraints)
            .map_err(|err| FolderError::DoesNotFit(err.to_string()))?;
        let (columns, half_lines) = (machine.profile.columns(), machine.profile.half_lines());
        let document = &file.document;
        let fits = document.current().fits(columns, half_lines)
            && document
                .finished()
                .iter()
                .all(|p| p.fits(columns, half_lines))
            && file.carriage.fits(columns, half_lines);
        if !fits {
            return Err(FolderError::DoesNotFit(machine.profile.name));
        }
        machine.carriage = file.carriage;
        machine.document = file.document;
        machine.log = file.log;
        for (day, words) in WritingLog::from_sessions(&file.sessions, day).days() {
            machine.log.add(*day, *words);
        }
        Ok(machine)
    }

    /// Reinserts the sheet as a typist would: carriage at the left margin,
    /// paper at the top margin. Returns where typing stopped: wind down to it.
    pub fn reinsert(&mut self) -> u16 {
        let c = &mut self.carriage;
        let stopped_at = c.half_line;
        c.half_line = stopped_at.min(self.profile.margins.top_lines * 2);
        c.column = c.left_margin;
        c.margin_released = false;
        stopped_at
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::{sm9, type_str};
    use super::super::{Command, Event, Typewriter};
    use crate::carriage::LineSpacing;
    use crate::constraints::EraseMode;
    use crate::document::{FORMAT_VERSION, FolderError};
    use crate::profile::Profile;

    /// Finds only `profile`, as if it were the one machine there is.
    fn by_name(profile: &Profile) -> impl FnOnce(&str) -> Option<Profile> + '_ {
        move |name| (name == profile.name).then(|| profile.clone())
    }

    fn utc_day(seconds: u64) -> jiff::civil::Date {
        jiff::Timestamp::from_second(seconds as i64)
            .unwrap()
            .to_zoned(jiff::tz::TimeZone::UTC)
            .date()
    }

    #[test]
    fn a_folder_file_puts_everything_back_where_it_was() {
        let mut tw = sm9();
        tw.constraints.erase = EraseMode::Fluid;
        type_str(&mut tw, "first sheet");
        tw.apply(Command::FeedSheet);
        tw.apply(Command::SetLineSpacing(LineSpacing::Double));
        type_str(&mut tw, "ab");
        tw.apply(Command::Erase);
        tw.apply(Command::Tab);
        let day = jiff::civil::date(2026, 9, 30);
        tw.log_words(day, 7);
        assert!(tw.annotate(0, "tighten\nthe opening"));
        assert!(!tw.annotate(1, "the sheet in the machine"));
        assert!(tw.scratchpad_mut().write(3, "call the printer"));
        assert!(tw.scratchpad_mut().open_at(2));
        let text = tw.to_folder_ron().unwrap();
        let back = Typewriter::from_folder_ron(&text, by_name(tw.profile()), utc_day).unwrap();
        assert_eq!(back.log(), tw.log());
        assert!(!text.contains("sessions"), "{text}");
        assert_eq!(back.document(), tw.document());
        assert_eq!(back.document().scratchpad().page(3), "call the printer");
        assert_eq!(back.document().scratchpad().spread(), 2);
        assert_eq!(back.carriage(), tw.carriage());
        assert_eq!(back.constraints, tw.constraints);
    }

    #[test]
    fn a_reinserted_sheet_winds_back_notch_by_notch() {
        let mut tw = sm9();
        tw.apply(Command::SetLineSpacing(LineSpacing::OneAndHalf));
        type_str(&mut tw, "one");
        tw.apply(Command::Return);
        type_str(&mut tw, "two");
        let stopped_at = tw.reinsert();
        assert_eq!(stopped_at, 15);
        assert_eq!((tw.carriage().half_line, tw.carriage().column), (12, 10));
        let notches: Vec<Event> = (12..stopped_at)
            .flat_map(|_| tw.apply(Command::PlatenNotch))
            .collect();
        assert_eq!(notches, [Event::LineFeed; 3]);
        assert_eq!((tw.carriage().half_line, tw.carriage().column), (15, 10));
    }

    #[test]
    fn a_folder_file_from_before_session_stats_opens() {
        let mut tw = sm9();
        type_str(&mut tw, "old");
        let text = tw.to_folder_ron().unwrap();
        let start = text.find("log:").unwrap();
        let old = text[..start].trim_end().replacen(
            &format!("version: {FORMAT_VERSION}"),
            "version: 1",
            1,
        ) + "\n)";
        let back = Typewriter::from_folder_ron(&old, by_name(tw.profile()), utc_day).unwrap();
        assert_eq!(back.document(), tw.document());
        assert!(back.log().days().is_empty());
    }

    #[test]
    fn sessions_in_an_older_folder_file_fold_into_days() {
        let tw = sm9();
        let text = tw.to_folder_ron().unwrap();
        let start = text.find("log:").unwrap();
        // 29 September 2026, two sessions; then the 30th.
        let old = text[..start].replacen(&format!("version: {FORMAT_VERSION}"), "version: 6", 1)
            + "sessions: [(started: 1790700000, seconds: 60, words: 40), \
               (started: 1790710000, seconds: 60, words: -5), \
               (started: 1790790000, seconds: 60, words: 12)],\n)";
        let back = Typewriter::from_folder_ron(&old, by_name(tw.profile()), utc_day).unwrap();
        let days: Vec<_> = back
            .log()
            .days()
            .iter()
            .map(|(d, &w)| (d.day(), w))
            .collect();
        assert_eq!(days, [(29, 35), (30, 12)]);
        let saved = back.to_folder_ron().unwrap();
        assert!(
            !saved.contains("sessions") && saved.contains("2026-09-29"),
            "{saved}"
        );
    }

    #[test]
    fn a_folder_file_from_before_the_scratchpad_opens() {
        let mut tw = sm9();
        type_str(&mut tw, "old");
        let text = tw.to_folder_ron().unwrap();
        // Drop the scratchpad's lines, from its field to its closing one.
        let mut skipping = false;
        let old: Vec<&str> = text
            .lines()
            .filter(|line| {
                if line.trim_start().starts_with("scratchpad:") {
                    skipping = true;
                }
                let keep = !skipping;
                if skipping && line.trim() == ")," {
                    skipping = false;
                }
                keep
            })
            .collect();
        let old = old
            .join("\n")
            .replacen(&format!("version: {FORMAT_VERSION}"), "version: 3", 1);
        assert!(!old.contains("pages:"));
        let back = Typewriter::from_folder_ron(&old, by_name(tw.profile()), utc_day).unwrap();
        assert_eq!(back.document(), tw.document());
        assert!(back.document().scratchpad().is_fresh());
    }

    #[test]
    fn a_folder_file_from_before_type_jams_opens_with_them_off() {
        let tw = sm9();
        let text = tw.to_folder_ron().unwrap();
        let old: Vec<&str> = text.lines().filter(|l| !l.contains("type_jams")).collect();
        let old = old
            .join("\n")
            .replacen(&format!("version: {FORMAT_VERSION}"), "version: 4", 1);
        let back = Typewriter::from_folder_ron(&old, by_name(tw.profile()), utc_day).unwrap();
        assert!(!back.constraints.type_jams);
    }

    #[test]
    fn folder_files_from_elsewhere_are_refused() {
        let tw = sm9();
        let text = tw.to_folder_ron().unwrap();
        let profile = tw.profile().clone();
        let newer = text.replacen(&format!("version: {FORMAT_VERSION}"), "version: 99", 1);
        assert!(matches!(
            Typewriter::from_folder_ron(&newer, by_name(&profile), utc_day),
            Err(FolderError::NewerVersion(99))
        ));
        let other = text.replacen("Olympia SM9", "Hermes 3000", 1);
        assert!(matches!(
            Typewriter::from_folder_ron(&other, by_name(&profile), utc_day),
            Err(FolderError::UnknownMachine(_))
        ));
        let off_the_sheet = text.replacen("column: 10", "column: 900", 1);
        assert!(matches!(
            Typewriter::from_folder_ron(&off_the_sheet, by_name(&profile), utc_day),
            Err(FolderError::DoesNotFit(_))
        ));
        assert!(matches!(
            Typewriter::from_folder_ron("a shopping list", by_name(&profile), utc_day),
            Err(FolderError::Unreadable(_))
        ));
    }
}
