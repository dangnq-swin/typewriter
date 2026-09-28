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
| Platform | **Linux only** |
| License | **GPL-3.0-or-later** |
| Default profile | **Olympia SM9**, Pica type (10 cpi, 6 lines per inch) |
| Default paper | **A4** (210 × 297 mm) |
| Constraints | Fixed line width with margin bell and manual carriage return. Every constraint is configurable (strict ↔ relaxed) |
| Background | Configurable. The default is an off-white paper texture **supplied by the maintainer** (do not generate or download a substitute) |
| Calm mode | Distraction-free mode: lines are dimmed progressively the further they are from the line being typed |
| Sounds | Key strike, carriage return, margin bell |
| Focus goals | Word-count and timer targets per session |
| Font | **Courier Prime** (SIL OFL), vendored in `assets/fonts/`. Closest free match to the SM9's slab-serif Modern Pica; no libre font reproduces it exactly |
| CI/CD | None for now |
| Storage | Own native document format (keeps strikeouts, overtyping and session stats) with **export** to plain text and Markdown |
| Core dependencies | `serde` (derive), `thiserror`, `toml` in `typewriter-core`. Core parses TOML from a string; the app does the file reads |
| Vertical position | Tracked in **half-line** steps (1/12 in at 6 lpi), like the platen ratchet. Line spacing 1 / 1.5 / 2 = 2 / 3 / 4 half-lines |
| Default strictness | Authentic: Backspace moves the carriage back without erasing (overtyping), right margin locks until the one-shot margin release (cleared on return), free cursor movement off |
| Erasing | Separate **Erase** action, on by default. Modes: `digital` (default, removes the glyph), `white-out` and `correction-tape` (cover the glyphs, which stay in the cell's stack and can be typed over), `off` |
| SM9 defaults | A4 at Pica = 82 columns x 70 lines. Left margin col 10, right margin col 72 (locks before it), bell 8 columns before the right margin, top margin 6 lines |
| Page end | A return on the last line emits `PageEnd` and does not feed; the carriage stays on that line until a new sheet is fed |

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
- Do not download or invent a replacement for the maintainer-provided paper texture. Use a
  flat off-white fill as a fallback until the texture is supplied.

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
