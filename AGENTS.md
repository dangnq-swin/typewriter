# AGENTS.md

Guidance for AI coding agents (and humans) working on this repository.

## Project overview

**typewriter** is a native desktop typewriter simulator for focused writing, in Rust. It emulates a
real mechanical machine: fixed pitch, fixed line width, a margin bell, a manual carriage return,
typewriter sounds, text on a textured sheet. Two editions on one library: `typewriter`, the plain
app, a focused writing tool; and `typewriter-desk`, the same machine seen from the chair. The
default profile is the **Olympia SM9** (1960s–70s West German portable); other machines come later
as profiles.

[`ROADMAP.md`](ROADMAP.md) lists what comes next and [`3D-FIX.md`](3D-FIX.md) the desk edition's
clean-up after its move to the depth pass. In either, a finished item is checked (`- [x]`) while its
section still has open items; once a section is done it leaves the file, as git and the release
notes keep it.

## Design principles

- **Only what a typewriter or a real desk can do.** Every feature needs a real-world counterpart:
  the machine, paper, a folder, a copy holder. No search across sheets, no ambient soundtracks. One
  exception: **Delete**, a traceless digital correction, kept at the maintainer's request and off the
  correction cycle unless the settings add it.
- **Game-like features go to the desk edition** — a moving camera, walking about an office. The
  plain app stays a focused writing tool.
- **The plain app's look is settled.** New parts drawn around the paper (levers, the margin rack) are
  the desk edition's only.

## Working agreement: ask before assuming

**Ask before proceeding** when a task involves a product or UX decision the code and this file do not
settle, a new dependency, asset or file format, a change to the document format or the
profile/config schema, or anything ambiguous where two readings lead to different code. Batch
questions, propose a recommended option, wait. If an answer sets a lasting rule, add it below. Small,
mechanical changes (a failing test, a rename, a typo) need no question round.

## Key binding rules

Check every new or changed binding against these:

- **No Ctrl** — a typewriter has none, and the keyboard should feel like one. Shift is fine. The one
  exception is **Ctrl+S** (save), kept for muscle memory at the maintainer's request.
- **Works on a compact (60 %) keyboard**, which lacks Insert and F1–F12: an action bound to one needs
  another way in. Return on the last line feeds a sheet as Insert does, the Spacing and Correct
  plates do what F1–F4 do, zoom is the plain wheel with a percentage plate that resets on
  double-click. F11 (fullscreen) is the exception: the desktop does it.
- **No auto-repeat unless the machine would repeat.** Space and Backspace ignore key repeat.
- **Controls never take keyboard focus.** Sense clicks with `render::CLICK`, not `Sense::click()`:
  egui moves focus with Tab and clicks a focused control on Enter, so the typewriter's own keys would
  press it.
- **Document it.** The key map is the module doc of `crates/typewriter-app/src/input.rs`; keep the
  README's *Controls* section in step.

## Repository layout

A Cargo workspace. Every module opens with a `//!` doc saying what it holds — read that rather than
look for a listing here.

- `crates/typewriter-core`: the machine as a library, unit-tested without a window.
- `crates/typewriter-app`: the app as a library, and `typewriter` — window, drawing (`render/`),
  input, audio, settings, filing, command line. `run()` opens it on a `Stage` (`stage.rs`), which
  `main.rs` hands it as `Plain`; `simulate.rs` is test-only.
- `crates/typewriter-desk`: the desk edition on the app's library — `room.rs`, `machine/`, everything
  in 3D. Not packaged yet: CI hands its binary out as a run artifact.
- `profiles/` machine profiles as data (schema: `docs/profiles.md`); `assets/` fonts, sounds, paper
  texture, icon, built into the binary; `packaging/linux/` the desktop entry and file type.
  `README.md` is the maintainer's: keep only its *Controls* section current.
- `scripts/`: `install.sh` builds and installs for the user, or in a tarball installs the program
  beside it; `package-linux.sh` the tarball and AppImage; `smoke-test.sh` checks a build opens a
  window; `prepare-sounds.sh` cuts the sounds from their CC0 sources; `convert-format-8.sh` converts
  pre-1.0 projects.

## Architecture rules

- `typewriter-core` must not depend on `egui`, `eframe`, audio crates, or the filesystem (except
  through `serde` types). The app crate handles all side effects.
- The core emits **events** (`Bell`, `CarriageReturn`, `KeyStrike`, `PageEnd`) that the app maps to
  sounds and animation. Never audio from the core.
- Machine characteristics belong in profile data (`profiles/*.toml`), not constants.
- The app decides in the **desk** (`app/desk/`), which knows no egui, sound or window: it takes
  `Intent`s and asks for `Effect`s. Views (`app/view/`) draw it and push intents; `app/mod.rs` does
  the effects. Test app behaviour on the desk (`desk/testing.rs`).
- The app's modules stay private unless `typewriter-import` needs them.
- The command line is Linux only, in `terminal.rs`. On Windows `typewriter` has no console, so
  anything it prints is lost and `typewriter-import` is the one console program. Commands are flags
  (`--import`), never bare words: a bare word is a project file.

### The two editions

- Everything 3D — the depth pass, its shaders, rasterizing it for snapshots, the eye, models,
  lighting — the room, the machine around the sheet, and whatever else only the desk uses, is code in
  `typewriter-desk`. `typewriter-app` holds the plain app and what both share, all of it flat.
- Where the desk needs the app, the app offers a hook on `Stage` whose default is the plain app's and
  hands over plain data (shapes, meshes, rects, the sheet's marks, window options); the desk does the
  3D with it. The typing view calls the hooks in its steps: behind the sheets, the knobs and the
  scale, over the sheets.
- The desk keeps no state: the app's desk holds what it draws from (the last return), and clicks on
  its controls come back as rects. It reaches the app's drawing only through `draw.rs`: share a
  helper by re-exporting it there, not by making a module public.
- The desk draws in depth — one wgpu callback a frame, a depth buffer, positions projected on the CPU
  (see `depth.rs` and `machine/canvas.rs`). A part moved into depth needs its real shape, not the
  order it was drawn in.

## Folder format

Projects are `*.typr` files (`typewriter-core/src/document.rs`), RON inside, MIME
`application/x-typewriter-folder`, versioned major.minor as `version: "1.0"`. Changing what they hold
needs the maintainer's yes.

- **Minor** (1.1, …): only what older files lack and can be read with defaults. The app opens every
  older minor of its major and writes only its own. Bump the minor in `FORMAT_VERSION`'s note, read
  the new field with `#[serde(default)]`, and test opening a file of the previous minor. A retired
  field: `#[serde(default, skip_serializing)]`, folded into its replacement.
- **Major** (2.0): anything older files can't default into. Refuse older majors and say so, and ship
  the conversion beside them, as `convert-format-8.sh` does for the pre-1.0 format.
- Newer files, minor or major, are refused with the reason: opening one would drop on the next save
  what this build doesn't know.
- Parse straight into the real types, never in a pass that ignores fields: ron skips ignored values in
  quadratic time, hours for a novel. The reopening-time test in `simulate.rs` guards this.

## Assets and licensing

- Every file in `assets/` needs a source and license recorded in `assets/LICENSES.md`.
- Only GPL-3.0-compatible licences (OFL, CC0, CC-BY, Apache-2.0 for fonts).
- Never download or invent a replacement for the maintainer-provided paper texture: a flat off-white
  fill is the fallback if it fails to load.

## Build, test, lint

```sh
cargo build --workspace
cargo run                     # typewriter
cargo run -p typewriter-desk  # the desk edition
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

A change is done when fmt, clippy (warnings denied) and tests all pass. CI
(`.github/workflows/`) runs those three on Linux for every push; keep the workflows in step with the
commands above. An ordinary push needs no report after it. Do check a run where CI is the only
witness — a `vX.Y.Z` tag, since a failed build leaves the Release an unpublished draft nobody looks
at; a hand-run Release dry run or Windows build, since the run and its downloads are the whole
result; or a change to `release.yml` or the packaging, which is done only when such a run passes.
Report a failure with its log, with whatever GitHub access there is (`gh`, a GitHub MCP server, or
the API by hand); where a run goes unchecked, say so rather than assume it passed. Where a build has
to open a window, `scripts/smoke-test.sh` proves that here too wherever `xvfb-run` is installed — it
hides `WAYLAND_DISPLAY` so the window goes to the display it starts, and under Xvfb the renderer is
Mesa's software one, so it shows the pipelines build and draw, nothing of how they look.

Measuring by hand, a novel-sized simulation (`TYPEWRITER_MANUSCRIPT=novel.odt` types a real
manuscript, not seeded prose):

```sh
cargo test -p typewriter-app --release -- --ignored --nocapture novel
```

Snapshots, to look at a drawing change where no display is: the typing view drawn to PNGs without a
window (`app/snapshot.rs`, the `snapshot` feature), the desk's depth pass filled on the CPU by
`depth::rasterize` through the `snapshot_callback` hook. The shots are in the desk's `stage.rs` test;
add one there for something new. Check a drawing change this way before calling it done.

```sh
TYPEWRITER_SNAPSHOT=<folder> cargo test -p typewriter-desk --release -- --ignored snapshot
```

### Releases

Semver on `version` in `Cargo.toml`, separate from the folder format's. To release: bump it, commit
and push, wait for Checks, then push an annotated tag `vX.Y.Z` whose message is the release notes
(`git tag -a v0.1.0 -F notes.md`).

`release.yml` builds Linux for glibc (on the oldest Ubuntu runner, for its older glibc) and for musl
(on Alpine, linked to its libraries: a static build can't open a window), and Windows, each Linux
build smoke-tested first. It attaches the plain app's tarballs, AppImage and zip and publishes the
Release once all are there; the desk's binaries go up as run artifacts on the same runs. The Windows
build alone is a workflow of its own (Actions → Windows build → Run workflow), leaving the programs as
a download on the run.

## Code style

- `rustfmt` defaults, idiomatic naming, edition 2024 idioms (let chains, `is_some_and`, …).
- Small, pure functions in the core. Unit tests sit next to the code; scenarios go in `tests/` ("type
  a full line and hear the bell at column N").
- Errors: `thiserror` in the core, `anyhow` at the app boundary.
- No `unwrap()`/`expect()` outside tests unless the invariant is stated at the call site.
- Reuse before adding: shared drawing and the palette in `render/mod.rs`, pencil fields in
  `render/note.rs` (`PencilField`), paths in `storage.rs`, and in the desk `machine/geometry.rs`,
  `machine/light.rs`, `machine/eye.rs`. Read them before writing a helper; extract one once the same
  logic appears twice.
- Name units: `_seconds`, `_mm`, `_percent`, `half_line`; or say them in the doc comment.
- XDG paths (`$XDG_CONFIG_HOME/typewriter`, `$XDG_DATA_HOME/typewriter`); on Windows both live in
  `%APPDATA%\typewriter`, and Windows shows paths in full, never as `~`.

## Comment style

Short, punchy guidance: say **why** or **what to watch out for**, never what the next line plainly
does.

- **Doc comments**: one line where possible, saying what the item does ("Moves finished sheet `from`
  to `to`."). Add a line only for a contract the signature can't show: units, ranges, edge cases,
  what a `bool` or `None` means.
- **Inline comments**: an instruction or a terse cause. "Keep equal to the crumple sound's length."
  "Safe cast: clamped to the zoom range." "No stop ahead: fly to the margin."
- Explain a mechanism only where the code can't show it (typewriter mechanics, perspective math, feed
  timing), in a few lines.
- Fragments are fine. Drop filler ("Note that", "We need to"), don't restate names or types, don't
  narrate history or cite tickets. Delete a comment rather than let it go stale.

```rust
// Too long:
/// The largest texture egui's GPU backends accept everywhere. The grain is
/// soft, so upscaling for large windows on 2x displays is not noticeable.
// Right:
/// egui's safe GPU texture limit. The grain is soft: upscaling won't show.
```

## UI text

**Labels, not explanations.** Controls carry short labels, with no notes explaining behaviour the
controls already show or that is documented elsewhere. Hover tooltips are fine. Problems (a broken
profile, a missing machine, a failed save) are always shown.

## Commits and branches

- Conventional Commits (`feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`), with `desk:` after
  the type for the desk edition (`feat: desk: …`).
- One logical change per commit. Do not commit or push unless asked.
- The plain app and shared code go straight to `main`. The desk edition is built on `desk-viewpoint`:
  a fix to shared code lands on `main` first and is merged into the branch (merged, not rebased: the
  branch is pushed).
