# AGENTS.md

Guidance for AI coding agents (and humans) working on this repository.

## Project overview

**typewriter** is a native desktop typewriter simulator for focused, distraction-free
writing, written in Rust. It emulates the feel and constraints of a real mechanical
typewriter: fixed pitch, fixed line width, a margin bell, a manual carriage return,
typewriter sounds, and the text on a textured sheet of paper.

It comes in two editions on one library: `typewriter`, the plain app, a focused writing tool;
and `typewriter-desk`, the desk edition, the same machine seen from the chair at a desk.

The first and default machine profile is the **Olympia SM9** (1960s–70s West German
portable). Other machines come later as profiles.

[`ROADMAP.md`](ROADMAP.md) lists what comes next, in order and without numbers; finished items
leave it, as the git history and release notes keep them.

## Design principles

- **Only what a typewriter or a real desk can do.** Every feature needs a real-world
  counterpart: the machine, paper, a folder, a copy holder. No search across sheets, no ambient
  soundtracks. The one exception is **Delete**, a traceless digital correction method, kept at
  the maintainer's request and off the correction cycle unless the settings add it.
- **Game-like features go to the desk edition.** A scene with a moving camera, walking about an
  office and the like belong there, never in the plain app, which stays a focused writing tool.
- **The plain app's look is settled.** New parts of the machine drawn around the paper (levers,
  the margin rack) are the desk edition's only.

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
  line spacing but the Spacing plate is clickable too, F4 cycles the correction method as the
  Correct plate does, and zoom is the plain mouse wheel with a percentage plate that resets on
  double-click. The one exception is F11 (fullscreen), which the desktop can do on its own.
- **No auto-repeat unless the machine would repeat.** Keys that a real typewriter does not
  repeat (e.g. Space, Backspace) ignore key repeat.
- **Controls never take keyboard focus.** Sense clicks with `render::CLICK`, not
  `Sense::click()`: egui moves focus with Tab and clicks a focused control on Enter, so the
  typewriter's own keys would press it.
- **Document it.** The full key map is the module doc of `crates/typewriter-app/src/input.rs`.
  Keep the README's *Controls* section in step.

## Repository layout

A Cargo workspace. Every module opens with a `//!` doc saying what it holds: read that rather
than keep a listing here.

- `crates/typewriter-core`: the machine as a library (pages, carriage, profiles, sessions, the
  folder format), unit-tested without a window.
- `crates/typewriter-app`: the app as a library, and `typewriter`. `run()` in `lib.rs` opens it
  on a `Stage` (`stage.rs`); `main.rs` only calls `run(Plain)`. The window, drawing
  (`render/`), input, audio, settings, filing, the command line and `typewriter --import`.
  `simulate.rs` is test-only: a writer's months of work, seeded.
- `crates/typewriter-desk`: the desk edition, `run(stage::Desk)` on the app's library: its room
  (`room.rs`) and machine (`machine/`). Not packaged or released yet.
- `profiles/`: machine profiles as data; `docs/profiles.md` has the schema.
- `assets/`: fonts, sounds, the paper texture and the icon, all built into the binary.
- `packaging/linux/`: the desktop entry and the project file type, for `install.sh` and the
  AppImage.
- `scripts/`:
  - `install.sh`: builds and installs for the current user; in a release tarball, installs the
    program beside it.
  - `package-linux.sh`: the release tarball and AppImage.
  - `smoke-test.sh`: checks a build starts on a virtual display.
  - `prepare-sounds.sh`: cuts the sounds from their CC0 sources.
  - `convert-format-8.sh`: converts projects from before folder format 1.0.
- `README.md` is the maintainer's: keep only its *Controls* section current.

## Architecture rules

- `typewriter-core` must not depend on `egui`, `eframe`, audio crates, or the filesystem
  (except through `serde` types). The app crate handles all side effects.
- The core emits **events** (e.g. `Bell`, `CarriageReturn`, `KeyStrike`, `PageEnd`) that the
  app maps to sounds and animation. Do not call audio from the core.
- Machine characteristics belong in profile data (`profiles/*.toml`), not hard-coded constants.
- The app decides in the **desk** (`app/desk/`), which knows no egui, sound or window: it takes
  `Intent`s and asks for `Effect`s. Views (`app/view/`) draw it and push intents; `app/mod.rs`
  does the effects. Test app behaviour on the desk (`desk/testing.rs`).
- The app's modules stay private unless `typewriter-import` needs them.
- The command line (flags, `--import` among them) is Linux only, in `terminal.rs`. On Windows,
  `typewriter` has no console, so anything it prints is lost; `typewriter-import` is the one
  console program there. Commands are flags (`--import`), never bare words: a bare word is a
  project file.

### The two editions

- What only the desk edition draws lives in `typewriter-desk`, never in the app. Where it needs
  to draw, the app offers a hook on `Stage` whose default is the plain app's, and the typing
  view calls the hooks in its steps (behind the sheets, over them, around the knobs).
- The desk edition keeps no state: the app's desk keeps what it draws from (e.g. the last
  return), and clicks on its controls come back to the app as rects.
- It reaches the app's drawing only through `draw.rs`: share a helper by re-exporting it there,
  not by making a module public.
- The desk draws its body and sheets in depth (`draw::depth`, `render/depth.rs`): a wgpu paint
  callback with a depth buffer, positions still projected on the CPU by the desk's eye. Its
  parts draw on a `Canvas` (`machine/canvas.rs`), flat or in depth: a face hides what is
  behind; what lies on one (edges, marks, print, glass) is set a hair nearer and hides nothing.
  A part moved into depth needs its real shape, not the order it was drawn in.

## Folder format

Projects are `*.typr` files (`typewriter-core/src/document.rs`), RON inside, of MIME type
`application/x-typewriter-folder`. The format is versioned major.minor, written `version: "1.0"`
at the top of the file. Changing what the files hold needs the maintainer's yes (see above).

- **Minor** (1.1, 1.2, …): only what older files lack and can be read with defaults. The app
  opens every older minor of its major and writes only its own. Bump the minor in
  `FORMAT_VERSION`'s note, read the new field with `#[serde(default)]`, and add a test that opens
  a file of the previous minor. A retired field is read with `#[serde(default, skip_serializing)]`
  and folded into its replacement.
- **Major** (2.0): anything older files can't be read into with defaults. The app refuses older
  majors and says so; ship a conversion with it (a script or a command), as
  `scripts/convert-format-8.sh` converts the last format from before 1.0.
- Files from a newer version, minor or major, are refused with the reason: opening one would drop
  what this build doesn't know on the next save.
- Parse straight into the real types, never in a pass that ignores fields (such as reading
  `version` alone): ron skips ignored values in quadratic time, hours for a novel. The
  reopening-time test in `simulate.rs` guards this.

## Assets and licensing

- Every file in `assets/` needs a recorded source and license in `assets/LICENSES.md`.
- Only add assets that are GPL-3.0-compatible (e.g. OFL, CC0, CC-BY, Apache-2.0 for fonts).
- Do not download or invent a replacement for the maintainer-provided paper texture. A flat
  off-white fill is the fallback if it cannot be loaded.

## Build, test, lint

```sh
cargo build --workspace
cargo run                     # typewriter
cargo run -p typewriter-desk  # the desk edition
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

A change is done when fmt, clippy (warnings denied) and tests all pass. A change to `release.yml`
or the packaging is done when a by-hand run of the Release workflow passes for the builds it
touches.

CI (`.github/workflows/`) runs those three on Linux for every push; keep the workflows in step
with the commands above. The Windows build is started by hand (Actions → Windows build → Run
workflow) and leaves the programs as a download on the run.

After pushing, check that the push's Checks run passed, with whatever GitHub access you have (the
`gh` CLI, or a GitHub MCP server with its Actions tools), and report a failure with its log. With
none, say the run is unchecked rather than assume it passed.

A novel-sized simulation, for measuring by hand (`TYPEWRITER_MANUSCRIPT=novel.odt` types a real
manuscript instead of seeded prose):

```sh
cargo test -p typewriter-app --release -- --ignored --nocapture novel
```

Snapshots, to look at a drawing change where no display is: the typing view drawn to PNGs
without a window, the depth pass on the CPU (typewriter-app's `snapshot` feature,
`app/snapshot.rs`). The shots are in the
desk edition's `stage.rs` test; add one there to look at something new. Check a change to the
drawing this way before calling it done.

```sh
TYPEWRITER_SNAPSHOT=<folder> cargo test -p typewriter-desk --release -- --ignored snapshot
```

### Releases

Releases follow semver on `version` in `Cargo.toml`, which is separate from the folder format's
version. To release: bump it, commit and push, wait for Checks, then push an annotated tag
`vX.Y.Z` whose message is the release notes (`git tag -a v0.1.0 -F notes.md`).

`release.yml` builds Linux for glibc (on the oldest Ubuntu runner, for its older glibc) and for
musl (on Alpine, linked to its libraries: a static build can't open a window), and Windows. Each
Linux build must run on a virtual display (`scripts/smoke-test.sh`). It attaches the tarballs,
AppImage and zip, and publishes the Release once all are there; a failed build leaves it a
draft. Run by hand (Actions → Release → Run workflow), it is a dry run of one build or all, with
nothing released.

## Code style

- `rustfmt` defaults, idiomatic naming, edition 2024 idioms (let chains, `is_some_and`, …).
- Small, pure functions in the core. Unit tests sit next to the code (`#[cfg(test)] mod tests`);
  scenarios go in `tests/` (e.g. "type a full line and hear the bell at column N").
- Errors: `thiserror` in the core, `anyhow` at the app boundary.
- No `unwrap()`/`expect()` outside tests unless the invariant is stated at the call site.
- Reuse before adding. Shared drawing helpers and the palette live in `render/mod.rs`, pencil
  text fields in `render/note.rs` (`PencilField`), path and file helpers in `storage.rs`; the
  desk's machine shares `machine/geometry.rs`, `machine/light.rs` and its eye's drawing
  (`machine/eye.rs`). Read them before writing a helper. Extract one once the same logic
  appears twice.
- Name units: `_seconds`, `_mm`, `_percent`, `half_line`; or say them in the doc comment.
- Config and data paths follow XDG (`$XDG_CONFIG_HOME/typewriter`, `$XDG_DATA_HOME/typewriter`);
  on Windows both live in `%APPDATA%\typewriter`. Windows shows paths in full, never as `~`.

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

## Commits and branches

- Use Conventional Commits (`feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`), with
  `desk:` after the type for the desk edition (`feat: desk: …`).
- One logical change per commit. Do not commit or push unless asked.
- The plain app and shared code go straight to `main`. The desk edition is built on the
  `desk-viewpoint` branch: a fix to shared code lands on `main` first and is merged into the
  branch (merged, not rebased: the branch is pushed).
