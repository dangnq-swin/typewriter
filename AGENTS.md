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

## Working agreement: ask before assuming

**Ask the maintainer questions before proceeding** whenever a task involves:

- a product or UX decision not already recorded in the *Decision log* below,
- adding a new dependency, asset, or file format,
- changing the on-disk document format or the profile/config schema,
- anything ambiguous, where two reasonable readings lead to different code.

Batch questions together, propose a recommended option, and wait for an answer
instead of guessing. Once a question is answered, record the outcome in the
*Decision log* so it is not asked again.

Small, mechanical, clearly-scoped changes (fixing a failing test, renaming, docs
typos) do not need a question round.

## Decision log

| Topic | Decision |
|---|---|
| Language | Rust (stable, edition 2024). Toolchain managed with **rustup**, pinned to the `stable` channel (with `rustfmt` and `clippy`) in `rust-toolchain.toml` |
| UI | Native GUI using **egui / eframe** |
| Platform | **Linux only**, Wayland first. X11 is not a target yet (low priority) |
| License | **GPL-3.0-or-later** |
| Default profile | **Olympia SM9**, Pica type (10 cpi, 6 lines per inch) |
| Default paper | **A4** (210 × 297 mm) |
| Constraints | Fixed line width with margin bell and manual carriage return. Every constraint is configurable (strict ↔ relaxed) |
| Background | The off-white paper photo **supplied by the maintainer** (do not generate or download a substitute). It fills the whole window, fixed (cover-fit, cropped equally), and does not scroll with the sheet. Its tone is even: the photo's vignetting is removed at build time, keeping the grain. Making it configurable is on the backburner |
| Calm mode | Distraction-free mode: lines are dimmed progressively the further they are from the line being typed, **strongly**: from full ink on the typing line down to 10 % about 4 lines away (a smooth curve, by distance on the sheet). It hides the carriage scale, the Spacing and Zoom plates and the folder icon; the margin frame, the typing pointer and the calm mode icon stay. Chrome and dimming fade in and out together over 250 ms. **Esc** turns it on and off in the typing view (in the folder, Esc still goes back); a small icon right of the folder icon also turns it on and off, and stays shown in calm mode (it does not fade). Falloff and minimum are constants until settings (M8) |
| Fullscreen | **F11** toggles fullscreen, on its own (calm mode does not change it). Keyboards without F11 use the desktop's own shortcut. F11 works while a sheet is being fed |
| Sounds | Key strike, carriage return, margin bell, plus blocked input, space bar, backspace, tab, sheet feed, erase, and a platen ratchet click per line when the paper is rolled (four variants). Played with **rodio**. Keys have several variants (never the same twice in a row); the bell picks one of its samples at random |
| Silent mechanisms | A profile's `[sounds]` table can silence a mechanism: `carriage_return` (Enter pressed) and `line_feed` (Enter held, rolling the paper), both on by default. The **SM9's carriage return is silent**, so its return sound is bundled but not played; its platen ratchet is audible |
| Sound source | **CC0 samples** (BigSoundBank, Freesound), proposed by the agent and approved by the maintainer. `scripts/prepare-sounds.sh` downloads them to `target/` and cuts the WAV clips in `assets/sounds/`, which are embedded in the binary. Freesound clips are from its 128 kbps previews (originals need a login) |
| Volume / mute | No volume or mute controls until settings (M8) |
| Focus goals | Word-count and timer targets per session |
| Font | **Courier Prime** (SIL OFL), vendored in `assets/fonts/`. Closest free match to the SM9's slab-serif Modern Pica; no libre font reproduces it exactly |
| CI/CD | None for now |
| Storage | Own native document format (keeps strikeouts, overtyping and session stats) with **export** to plain text and Markdown |
| Core dependencies | `serde` (derive), `thiserror`, `toml` in `typewriter-core`. Core parses TOML from a string; the app does the file reads |
| Vertical position | Tracked in **half-line** steps (1/12 in at 6 lpi), like the platen ratchet. Line spacing 1 / 1.5 / 2 = 2 / 3 / 4 half-lines |
| Default strictness | Authentic: Backspace moves the carriage back without erasing (overtyping), right margin locks until the one-shot margin release (cleared on return), free cursor movement off |
| Erasing | Separate **Erase** action, on by default. Modes: `digital` (default, removes the glyph), `white-out` and `correction-tape` (cover the glyphs, which stay in the cell's stack and can be typed over), `off` |
| SM9 defaults | A4 at Pica = 82 columns x 70 lines. Left margin col 10, right margin col 72 (locks before it), bell 8 columns before the right margin, top margin 6 lines |
| Page end | A return or line feed on the last line emits `PageEnd` (the core does not feed). The app answers it by **feeding a new sheet**, as Insert does, since many compact (e.g. 60 %) keyboards have no Insert key |
| Keys | **No Ctrl bindings** (a typewriter has no Control key); Shift is fine. Insert = feed a new sheet. F1 / F2 / F3 = line spacing 1 / 1.5 / 2. Esc = calm mode on/off (typing view) or back (folder). F11 = fullscreen. Backspace = carriage back (no erase). Shift+Backspace or Delete = Erase. Enter = return; **holding Enter rolls the paper** on a line per key repeat (`LineFeed`: platen knob, carriage stays put), into a new sheet at the page end. Space, Backspace, Shift+Backspace, Delete, Insert, Tab, Esc and F11 ignore key repeat. Hold Shift and tap Tab: 1× sets a tab stop, 2× clears the nearest stop within 3 columns, 3× clears all (acts on Shift release). Full list in `crates/typewriter-app/src/input.rs`; rebinding comes with settings (M8) |
| Platen view | The typing point stays fixed on screen. The paper scrolls up and, by default, slides sideways like the carriage (can be turned off). A single pointer just below the typing line marks the typing point |
| Blocked input | A short horizontal jolt of the sheet (plus a sound from M4) |
| Carriage scale | A ruler below the typing line, travelling with the carriage like the SM9 scale: column ticks and numbers, margin brackets, tab stop pointers |
| Spacing indicator | Plate labelled "Spacing:" right below the scale, flush with its left end (paper edge, travels with the carriage). Two circles: first filled; second empty (1), left half filled (1.5) or filled (2). **Clicking it** moves the lever one notch: 1 → 1.5 → 2 → 1 |
| Sheet | No fill of its own. A thin frame around the writing area, 1 mm outside the margins (just clear of the type), with its four sides extended to the paper's edges to show the sheet's width and height. It follows the carriage margins; its bottom edge is the bottom of the sheet, since type can go down to the last line (the profile has no bottom margin) |
| Texture bundling | `image` crate (JPEG only), as a build dependency and at runtime. `build.rs` evens out the original in `assets/paper/` and re-encodes it at JPEG quality 70, at most 2048 px per side (the GPU texture limit), into the binary; the original file stays as supplied |
| New sheet | **Insert** feeds a fresh sheet at any time; so does Return on the last line. Margins, tab stops and spacing carry over; the carriage starts at the top margin. A blank sheet is not filed, it just stays in the machine. A feed takes about 8.5 s, as two clips played back to back: `feed-out.wav` (steady platen ratchet clicks, one every 60 ms, 1.6 s) and `feed-in.wav` (Gate13 1:49–1:57, with the pauses between its parts — paper in, page turn, ratchet run — tightened to 0.15 s and 0.25 s). The script makes the clicks as long as the time the tightened pauses gave back, stretched by 40 % so the old sheet does not rush out. **All input is ignored until the feed has finished** (until the new sheet has settled, timed from the clips, with or without a sound device) |
| Sheet feed animation | During the clicks, the finished sheet winds up and out **at a steady speed** (its whole travel over the clicks' duration). Then the new sheet rises from the bottom of the window into place **in step with the knob turns** (driven by `feed-in.wav`'s loudness, so it moves only while the knob is heard). While moving, the sheet shows a soft shadow and a curled top edge that flattens as it lands. Once it lands (always at least 250 ms before its sound ends), its shadow fades out while the typing pointer fades in, **together over 250 ms**, and the feed is over: input unlocks then, without waiting for the quiet end of the sound (about 7.3 s after the key press with the bundled clips). Ruler and spacing plate stay put |
| First sheet | A **new document** starts by winding its first sheet in: the feed animation and `feed-in.wav` without the wind-out (there is no finished sheet), input locked until it settles. Reopening saved documents (M6) is still to be decided |
| Finished sheets | Kept in the document and browsable in a **2.5D folder**: a manila folder in perspective on a dimmed desk, sheets fanned so each older one peeks out behind the newer, words shown as faint ink bars (egui cannot draw text in perspective). The chosen sheet is lifted and outlined; the arrow keys (up/left = older) or Page Up / Page Down choose it, as does moving the pointer onto one. Enter or a click opens it read-only at full size, where the same keys flip sheets. Opened with Page Up or the folder icon at the bottom left, with the newest sheet chosen; Esc goes back one level; typing returns to the machine and types |
| Zoom | The **plain mouse wheel** (no keys) steps 50–200 % in 10 % steps, one step per notch; 100 % = 96 points per inch. The level shows as a percentage on a plate right of the Spacing plate; **double-clicking** it resets to 100 %. egui's own UI zoom keys are off. Not remembered between runs until settings (M8) |
| Ink realism | On by default, very subtle: each strike lands up to 0.4 pt off and prints up to 10 % lighter, derived from its position so it never changes on redraw. Toggle comes with settings (M8) |
| Margin controls | On the backburner: no key bindings for margin release or setting margins yet (the core supports them) |

## Key binding rules

Check every new or changed binding against these:

- **No Ctrl.** A typewriter has no Control key, and the keyboard should feel like one. Shift is
  fine (typewriters have it).
- **Works on a compact (60 %) keyboard.** Keys such as Insert and F1–F12 are missing there, so an
  action bound to one also needs another way in: an on-screen control or a typewriter-like
  behaviour. For example, Insert feeds a sheet but so does Return on the last line, F1/F2/F3 set
  line spacing but the Spacing plate is clickable too, and zoom is the plain mouse wheel with a
  percentage plate that resets on double-click. The one exception is F11 (fullscreen), which the desktop can do on its own.
- **No auto-repeat unless the machine would repeat.** Keys that a real typewriter does not
  repeat (e.g. Space, Backspace) ignore key repeat.

## Repository layout

Cargo workspace, with the pure logic separated from the GUI so it can be unit-tested
without a window:

```
typewriter/
├── Cargo.toml                  # workspace manifest (shared deps, lints, profiles)
├── rust-toolchain.toml         # rustup toolchain: stable + rustfmt, clippy
├── AGENTS.md
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
│           ├── render/         # paper, glyphs, calm-mode dimming, platen view
│           ├── input.rs        # key events -> core commands
│           ├── audio.rs        # sound playback
│           └── settings.rs     # user config (XDG paths)
├── assets/
│   ├── paper/                  # paper textures (default provided by maintainer)
│   ├── fonts/                  # typewriter fonts (license must be GPL-compatible)
│   └── sounds/                 # key, return, bell samples (license must be recorded)
├── profiles/
│   └── olympia-sm9.toml        # data-driven machine profile
├── scripts/
│   └── prepare-sounds.sh       # cuts assets/sounds/ from their CC0 sources (curl, ffmpeg)
└── docs/
    └── adr/                    # architecture decision records for larger decisions
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

## Coding conventions

- Follow `rustfmt` defaults and idiomatic Rust naming.
- Prefer small, pure functions in the core. Test them with unit tests next to the code
  (`#[cfg(test)] mod tests`) and use `tests/` for integration-style scenarios
  (e.g. "type a full line and hear the bell at column N").
- Errors: `thiserror` in the core, `anyhow` at the app boundary.
- No `unwrap()`/`expect()` in non-test code unless the invariant is documented at the call site.
- Keep comments sparse. Explain *why* (e.g. typewriter mechanics), not *what*.
- Config and data paths follow XDG (`$XDG_CONFIG_HOME/typewriter`, `$XDG_DATA_HOME/typewriter`).

## Commits

- Use Conventional Commits (`feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`).
- One logical change per commit. Do not commit or push unless asked.
