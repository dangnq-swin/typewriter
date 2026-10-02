//! The machines: built-in profiles, then the user's own in
//! `$XDG_DATA_HOME/typewriter/profiles/*.toml` (`%APPDATA%\typewriter\profiles` on Windows).

use std::fs;

use typewriter_core::Profile;

use crate::storage;

/// New projects' machine unless the settings choose another.
pub const DEFAULT: &str = "Olympia SM9";

const BUILT_IN: [(&str, &str); 1] = [(
    "olympia-sm9.toml",
    include_str!("../../../profiles/olympia-sm9.toml"),
)];

pub struct Machines {
    profiles: Vec<Profile>,
    /// Unusable user profiles, and why.
    pub problems: Vec<String>,
}

impl Machines {
    /// Built-in machines, then the user's in file name order. Taken names
    /// are refused.
    pub fn load() -> anyhow::Result<Self> {
        let mut machines = Self::built_in()?;
        let Some(dir) = storage::profiles_dir() else {
            return Ok(machines);
        };
        let mut files: Vec<_> = match fs::read_dir(&dir) {
            Ok(entries) => entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|e| e == "toml"))
                .collect(),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(err) => {
                let dir = storage::home_relative(&dir);
                machines.problems.push(format!("{dir}: {err}"));
                Vec::new()
            }
        };
        files.sort();
        for path in files {
            let file = storage::file_name(&path);
            let loaded = fs::read_to_string(&path)
                .map_err(|err| err.to_string())
                .and_then(|text| Profile::from_toml_str(&text).map_err(|err| err.to_string()));
            match loaded {
                Ok(profile) if machines.find(&profile.name).is_some() => machines
                    .problems
                    .push(format!("{file}: the name \"{}\" is taken", profile.name)),
                Ok(profile) => machines.profiles.push(profile),
                Err(err) => machines.problems.push(format!("{file}: {err}")),
            }
        }
        Ok(machines)
    }

    /// The built-in machines alone.
    pub fn built_in() -> anyhow::Result<Self> {
        let profiles = BUILT_IN
            .iter()
            .map(|(file, text)| {
                Profile::from_toml_str(text)
                    .map_err(|err| anyhow::anyhow!("built-in profile {file}: {err}"))
            })
            .collect::<anyhow::Result<_>>()?;
        Ok(Self {
            profiles,
            problems: Vec::new(),
        })
    }

    pub fn all(&self) -> &[Profile] {
        &self.profiles
    }

    pub fn find(&self, name: &str) -> Option<Profile> {
        self.profiles.iter().find(|p| p.name == name).cloned()
    }

    /// A new project's machine: `name`, or the default if it is gone.
    pub fn for_new(&self, name: &str) -> Profile {
        self.find(name)
            .or_else(|| self.find(DEFAULT))
            .or_else(|| self.profiles.first().cloned())
            // Can't happen: `load` always adds the built-ins.
            .unwrap_or_else(|| unreachable!("no machines"))
    }
}

/// "10 cpi · 6 lpi · 210 × 297 mm · 82 columns × 70 lines".
pub fn describe(profile: &Profile) -> String {
    format!(
        "{} cpi \u{b7} {} lpi \u{b7} {} \u{d7} {} mm \u{b7} {} columns \u{d7} {} lines",
        profile.pitch_cpi,
        profile.lines_per_inch,
        profile.paper.width_mm,
        profile.paper.height_mm,
        profile.columns(),
        profile.half_lines() / 2
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_documented_example_is_a_valid_profile() {
        let docs = include_str!("../../../docs/profiles.md");
        let example = docs
            .split("```toml\n")
            .nth(1)
            .and_then(|rest| rest.split("```").next())
            .unwrap();
        let profile = Profile::from_toml_str(example).unwrap();
        assert_eq!((profile.columns(), profile.half_lines() / 2), (102, 66));
    }

    #[test]
    fn the_sm9_is_described_by_its_type_and_paper() {
        let sm9 = Profile::from_toml_str(BUILT_IN[0].1).unwrap();
        assert_eq!(sm9.name, DEFAULT);
        assert_eq!(
            describe(&sm9),
            "10 cpi \u{b7} 6 lpi \u{b7} 210 \u{d7} 297 mm \u{b7} 82 columns \u{d7} 70 lines"
        );
    }
}
