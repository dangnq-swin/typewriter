# AGENTS.md

Guidance for AI coding agents (and humans) working on this repository.

## Project overview

**typewriter** is a native desktop typewriter simulator for focused, distraction-free
writing, written in Rust. It emulates the feel and constraints of a real mechanical
typewriter: fixed pitch, fixed line width, a margin bell, a manual carriage return,
and typewriter sounds. It renders the text onto a textured sheet of paper.

The first and default machine profile is the **Olympia SM9** (1960s–70s West German
portable). Other machines are added later as additional profiles.

See [`ROADMAP.md`](ROADMAP.md) for the planned milestones.

## Design principles

- **Only what a typewriter or a real desk can do.** Every feature needs a real-world
  counterpart: the machine, paper, a folder, a copy holder. No search across sheets, no ambient
  soundtracks.

## Working agreement: ask before assuming

**Ask the maintainer questions before proceeding** whenever a task involves:

- a product or UX decision the code and this file do not already settle,
- adding a new dependency, asset, or file format,
- changing the on-disk document format or the profile/config schema,
- anything ambiguous, where two reasonable readings lead to different code.

Batch questions together, propose a recommended option, and wait for an answer
instead of guessing. If an answer sets a lasting rule, add it to the matching
section below.

Small, mechanical, clearly-scoped changes (fixing a failing test, renaming, docs
typos) do not need a question round.

## Key binding rules

Check every new or changed binding against these:

- **No Ctrl.** A typewriter has no Control key, and the keyboard should feel like one. Shift is
  fine (typewriters have it). The one exception is **Ctrl+S** (save), kept for muscle memory at
  the maintainer's request.
- **Works on a compact (60 %) keyboard.** Keys such as Insert and F1–F12 are missing there, so an
  action bound to one also needs another way in: an on-screen control or a typewriter-like
  behaviour. For example, Insert feeds a sheet but so does Return on the last line, F1/F2/F3 set
  line spacing but the Spacing plate is clickable too, F4 cycles the correction method as the Correct plate does, and zoom is the plain mouse wheel with a
  percentage plate that resets on double-click. The one exception is F11 (fullscreen), which the desktop can do on its own.
- **No auto-repeat unless the machine would repeat.** Keys that a real typewriter does not
  repeat (e.g. Space, Backspace) ignore key repeat.
- **Document it.** The full key map is the module doc of `crates/typewriter-app/src/input.rs`.
  Keep the README's *Controls* section in step; the rest of the README is the maintainer's.

## Repository layout

Cargo workspace, with the pure logic separated from the GUI so it can be unit-tested
without a window:

```
typewriter/
├── Cargo.toml                  # workspace manifest (shared deps, lints, profiles)
├── rust-toolchain.toml         # rustup toolchain: stable + rustfmt, clippy
├── AGENTS.md
├── README.md                   # the maintainer's; keep its Controls section current
├── ROADMAP.md
├── LICENSE                     # GPL-3.0
├── crates/
│   ├── typewriter-core/        # library: no GUI, no audio, no I/O side effects
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── page.rs         # page grid, cells, overtyped glyph stacks
│   │       ├── carriage.rs     # carriage position, margins, bell zone, line feed
│   │       ├── profile.rs      # machine profiles (SM9 first), pitch, paper size
│   │       ├── constraints.rs  # strictness settings (backspace, margins, ...)
│   │       ├── machine.rs      # Typewriter: commands in, events out
│   │       ├── document.rs     # multi-page document + native format (serde)
│   │       ├── export.rs       # plain text / Markdown export
│   │       └── session.rs      # focus goals, word count, timers, stats
│   └── typewriter-app/         # binary: eframe app, rendering, input, audio, settings
│       ├── build.rs            # re-encodes the paper texture for bundling
│       └── src/
│           ├── main.rs
│           ├── app.rs          # eframe::App impl, top-level state
│           ├── render/         # paper, glyphs, calm-mode dimming, platen view, settings card, notes
│           ├── input.rs        # key events -> core commands
│           ├── filing.rs       # projects: autosave, Save / Save As / Rename / Open (rfd), export
│           ├── machines.rs     # built-in and user profiles
│           ├── storage.rs      # XDG paths, drafts, crash-safe writes
│           ├── audio.rs        # sound playback
│           └── settings.rs     # user settings, config.toml (XDG paths)
├── assets/
│   ├── paper/                  # paper textures (default provided by maintainer)
│   ├── fonts/                  # typewriter fonts (license must be GPL-compatible)
│   ├── icons/                  # the app icon (SVG)
│   └── sounds/                 # key, return, bell samples (license must be recorded)
├── profiles/
│   └── olympia-sm9.toml        # data-driven machine profile
├── scripts/
│   ├── install.sh              # builds and installs the command, desktop entry and icon
│   └── prepare-sounds.sh       # cuts assets/sounds/ from their CC0 sources (curl, ffmpeg)
└── docs/
    └── profiles.md             # machine profile schema, adding a machine
```

Rules:

- `typewriter-core` must not depend on `egui`, `eframe`, audio crates, or the filesystem
  (except through `serde` types). The app crate handles all side effects.
- The core emits **events** (e.g. `Bell`, `CarriageReturn`, `KeyStrike`, `LineEnd`) that the
  app maps to sounds and animation. Do not call audio from the core.
- Machine characteristics belong in profile data (`profiles/*.toml`), not hard-coded constants.

## Assets and licensing

- Every file in `assets/` needs a recorded source and license in `assets/LICENSES.md`.
- Only add assets that are GPL-3.0-compatible (e.g. OFL, CC0, CC-BY, Apache-2.0 for fonts).
- Do not download or invent a replacement for the maintainer-provided paper texture. A flat
  off-white fill is the fallback if it cannot be loaded.

## Build, test, lint

```sh
cargo build --workspace
cargo run -p typewriter-app
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

A change is done when fmt, clippy (warnings denied) and tests all pass.

## Code style

- `rustfmt` defaults, idiomatic naming, edition 2024 idioms (let chains, `is_some_and`, …).
- Small, pure functions in the core. Unit tests sit next to the code (`#[cfg(test)] mod tests`);
  scenarios go in `tests/` (e.g. "type a full line and hear the bell at column N").
- Errors: `thiserror` in the core, `anyhow` at the app boundary.
- No `unwrap()`/`expect()` outside tests unless the invariant is stated at the call site.
- Reuse before adding. Shared drawing helpers and the palette live in `render/mod.rs`
  (`smoothstep`, `unit`, `splitmix64`, `SHEET`, `SHEET_EDGE`, `HIGHLIGHT`); shared path and
  file helpers in `storage.rs`. Extract a helper once the same logic appears twice.
- Name units: `_seconds`, `_mm`, `_percent`, `half_line`; or say them in the doc comment.
- Config and data paths follow XDG (`$XDG_CONFIG_HOME/typewriter`, `$XDG_DATA_HOME/typewriter`).

## Comment style

Comments are short, punchy guidance: they say **why**, or **what to watch out for**, never
what the next line plainly does.

- **Doc comments** (`///`, `//!`): one line where possible, saying what the item is or does
  ("Moves finished sheet `from` to `to`."). Add a line only for a contract the signature
  cannot show: units, ranges, edge cases, what a `bool` or `None` means ("False if out of
  range.").
- **Inline comments** (`//`): an instruction or a terse cause. "Keep equal to the crumple
  sound's length." "Safe cast: clamped to the zoom range." "No stop ahead: fly to the margin."
- Explain a mechanism only when the code cannot show it (typewriter mechanics, perspective
  math, feed timing), and then in a few lines.
- Fragments are fine. Drop filler ("Note that", "This is so that", "We need to"). Do not
  restate names or types, narrate history, or cite tickets and conversations.
- Delete a comment rather than let it go stale.

```rust
// Too long:
/// The largest texture egui's GPU backends accept everywhere. The grain is
/// soft, so upscaling for large windows on 2x displays is not noticeable.
// Right:
/// egui's safe GPU texture limit. The grain is soft: upscaling won't show.
```

## UI text

**Labels, not explanations.** Controls carry short labels, with no notes explaining behaviour
the controls already show or that is documented elsewhere. Hover tooltips are fine. Problems
(a broken profile, a missing machine, a failed save) are always shown.

## Commits

- Use Conventional Commits (`feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`).
- One logical change per commit. Do not commit or push unless asked.
