# AGENTS.md

Guidance for AI coding agents (and humans) working on this repository.

## Project overview

**typewriter** is a native desktop typewriter simulator in Rust: fixed pitch, one line width, a margin bell,
a manual carriage return, typewriter sounds, text on a textured sheet. Two editions on one library:
`typewriter`, the plain app, a focused writing tool; and `typewriter-desk`, the same machine seen from the
chair. The default profile is the **Olympia SM9**; other machines come later as profiles.
[`ROADMAP.md`](ROADMAP.md) lists what comes next, [`3D-FIX.md`](3D-FIX.md) the desk's clean-up: a done item
gets `- [x]`; a finished section leaves the file. `README.md` is the maintainer's: keep only its *Controls*
section current.

## Design principles

- **Only what a typewriter or a real desk can do.** Every feature needs a real-world counterpart: the
  machine, paper, a folder, a copy holder. No search across sheets, no ambient soundtracks. One exception:
  **Delete**, a traceless digital correction, kept at the maintainer's request.
- **Game-like features go to the desk edition** — a moving camera, walking about an office. The plain app
  stays a focused writing tool.
- **The plain app's look is settled.** New parts drawn around the paper (levers, the margin rack) are the
  desk's only.

## Working agreement: ask before assuming

**Ask before proceeding** when a task involves a product or UX decision the code and this file do not settle,
a new dependency, asset or file format, or a change to the document format or the profile/config schema, or
anything ambiguous where two readings lead to different code. Batch questions, propose a recommended option,
wait; answers that set a lasting rule go here. Small, mechanical changes need no question round.

## Key binding rules

The full key map is the module doc of `crates/typewriter-app/src/input.rs`; keep the README's *Controls* in
step. Check every binding against these:

- **No Ctrl** — the keyboard should feel like a typewriter's. Shift is fine; the one exception is **Ctrl+S**
  (save), kept for muscle memory at the maintainer's request.
- **No auto-repeat unless the machine would repeat.** Space and Backspace ignore key repeat.

## Compact keyboard

The app must work on a 60 % keyboard, which lacks Insert and F1–F12: an action bound to one needs another
way in — the alternatives are listed in `input.rs`'s module doc and the README. F11 (fullscreen) is the
exception: the desktop does it.

## Mouse

Controls never take keyboard focus: sense clicks with `render::CLICK`, not `Sense::click()` — egui moves
focus with Tab and clicks a focused control on Enter, so the typewriter's own keys would press it.

## Repository layout

A Cargo workspace. Every module opens with a `//!` doc saying what it holds — read that rather than look for
a listing here.

- `crates/typewriter-core`: the machine as a library, unit-tested without a window.
- `crates/typewriter-app`: the app as a library, and the `typewriter` and `typewriter-import` binaries —
  window, drawing (`render/`), input, audio, settings, filing, command line. `run()` opens it on a `Stage`
  (`stage.rs`), which `main.rs` hands it as `Plain`; `simulate.rs` is test-only.
- `crates/typewriter-desk`: the desk edition on the app's library — `room.rs`, `machine/`, everything in 3D.
  Not packaged yet: CI hands its binary out as a run artifact.
- `profiles/` machine profiles as data (schema: `docs/profiles.md`); `assets/` fonts, sounds, paper texture,
  icon, built into the binary; `packaging/linux/` the desktop entry and file type; `scripts/` install,
  packaging, smoke test, sound cutting, format conversion.

## Architecture rules

- `typewriter-core` must not depend on `egui`, `eframe`, audio crates, or the filesystem (except through
  `serde` types); the app handles all side effects. The core emits **events** (`Bell`, `CarriageReturn`,
  `KeyStrike`, `PageEnd`) for the app to turn into sounds — never audio from the core.
- Machine characteristics belong in profile data (`profiles/*.toml`), not constants.
- The app decides in the **desk** (`app/desk/`), which knows no egui, sound or window: views push `Intent`s,
  `app/mod.rs` does the `Effect`s. Test app behaviour on the desk (`desk/testing.rs`).
- The app's modules stay private unless `typewriter-import` needs them.
- The command line is Linux only, in `terminal.rs`; commands are flags (`--import`), never bare words: a
  bare word is a project file. On Windows `typewriter` has no console, so `typewriter-import` is the one
  console program.

### The two editions

- Everything 3D — depth pass, shaders, the eye, models, lighting, the room, the machine around the sheet —
  is code in `typewriter-desk`; `typewriter-app` holds the plain app and what both share, all of it flat.
- The app meets the desk through hooks on `Stage` handing over plain data; the desk keeps no state and
  reaches the app's drawing only through `draw.rs` (share a helper by re-exporting there, never by making
  a module public). The contracts are in the desk's `stage.rs` doc.

## Folder format

`*.typr` projects are RON inside `typewriter-core/src/document.rs`, versioned major.minor; changing what
they hold needs the maintainer's yes. The versioning rules — minor vs major, defaults, conversions, and
why a parse must not ignore fields — are `document.rs`'s module doc; read it before touching the format.

## Assets and licensing

- Every file in `assets/` needs a source and license recorded in `assets/LICENSES.md`; only
  GPL-3.0-compatible ones (OFL, CC0, CC-BY, Apache-2.0 for fonts).
- Never download or invent a replacement for the maintainer-provided paper texture: a flat off-white fill
  is the fallback if it fails to load.

## Build and test

```sh
cargo build --workspace
cargo run                     # typewriter
cargo run -p typewriter-desk  # the desk edition
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

A change is done when fmt, clippy and tests pass; the workspace lints plus `-D warnings` deny
`unwrap`/`expect` outside tests. Look at a drawing change by snapshot, not by guessing: the typing view
draws to PNGs through the `snapshot` feature, and

```sh
TYPEWRITER_SNAPSHOT=<folder> cargo test -p typewriter-desk --release -- --ignored snapshot
```

fills the desk's depth pass on the CPU; the desk's `stage.rs` test holds the shots — add one there for
something new.

### CI

`checks.yml` runs fmt, clippy and tests on every push — keep it in step with the commands above. An
ordinary push needs no report; do check a run where CI is the only witness (a `vX.Y.Z` tag, a hand-run
dry run or Windows build, a change to `release.yml` or the packaging) and report a failure with its log
(`gh`, the GitHub MCP server, or the API). `scripts/smoke-test.sh` proves a build opens a window wherever
`xvfb-run` is installed.

### Releases

Semver on `version` in `Cargo.toml`, separate from the folder format's. To release: bump it, commit and
push, wait for Checks, then push an annotated tag `vX.Y.Z` whose message is the release notes.
`release.yml` builds Linux (glibc and musl) and Windows, each Linux build smoke-tested first, and
publishes once every artifact is attached; the desk's binaries go up as run artifacts. `release.yml` and
`windows.yml` can be run by hand for dry runs.

## Code style

- `rustfmt` defaults, idiomatic naming, edition 2024 idioms.
- Small, pure functions in the core. Unit tests sit next to the code; scenarios go in `tests/`.
- Errors: `thiserror` in the core, `anyhow` at the app boundary.
- No `unwrap()`/`expect()` outside tests unless the invariant is stated at the call site.
- Reuse before adding: shared drawing and the palette in `render/mod.rs`, paths in `storage.rs`, and in
  the desk `machine/geometry.rs`, `machine/light.rs`, `machine/eye.rs`. Read them before writing a
  helper; extract one once the same logic appears twice.
- Name units: `_seconds`, `_mm`, `_percent`, `half_line`; or say them in the doc comment.
- XDG paths (`$XDG_CONFIG_HOME/typewriter`, `$XDG_DATA_HOME/typewriter`); on Windows both live in
  `%APPDATA%\typewriter`, and Windows shows paths in full, never as `~`.

## Comment style

Say **why** or **what to watch out for**, never what the next line plainly does. Fragments are fine; drop
filler ("Note that"); delete a comment rather than let it go stale.

- **Doc comments**: one line where possible; more only for a contract the signature can't show — units,
  ranges, edge cases, what a `bool` or `None` means.
- **Inline comments**: an instruction or a terse cause.
- Explain a mechanism only where the code can't show it (typewriter mechanics, perspective math, feed
  timing), in a few lines.

## UI text

**Labels, not explanations.** No notes explaining behaviour the controls already show or that is
documented elsewhere; hover tooltips are fine. Problems (a broken profile, a missing machine, a failed
save) are always shown.

## Commits and branches

- Conventional Commits (`feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`), with `desk:` after the
  type for the desk edition (`feat: desk: …`).
- One logical change per commit. Do not commit or push unless asked.
- The plain app and shared code go straight to `main`. The desk edition is built on `desk-viewpoint`: a
  fix to shared code lands on `main` first and is merged into the branch (merged, not rebased: the
  branch is pushed).
