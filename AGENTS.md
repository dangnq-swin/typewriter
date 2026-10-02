# AGENTS.md

Guidance for AI coding agents (and humans) working on this repository.

## Project overview

**typewriter** is a native desktop typewriter simulator in Rust: fixed pitch, one line width, a margin bell,
a manual carriage return, typewriter sounds, text on a textured sheet. Two editions on one library:
`typewriter`, the desk edition, the default; and `typewriter-plain`, a focused writing tool, the same
machine without the room. The default profile is the **Olympia SM9**; other machines come later as profiles.
[`ROADMAP.md`](ROADMAP.md) lists what comes next: a done item gets `- [x]`; a finished section leaves the
file. `README.md` is the maintainer's: keep only its *Controls* section current.

## Design principles

- **Only what a typewriter or a real desk can do.** Every feature needs a real-world counterpart: the
  machine, paper, a folder, a copy holder. No search across sheets, no ambient soundtracks.
- **The plain app's look is settled.** New parts drawn around the paper (levers, the margin rack) are the
  desk's only.

## Working agreement: ask before assuming

**Ask before proceeding** when a task involves a product or UX decision the code and this file do not settle,
a new dependency, asset or file format, or a change to the document format or the profile/config schema, or
anything ambiguous where two readings lead to different code. Batch questions, propose a recommended option,
wait; answers that set a lasting rule go here. Small, mechanical changes need no question round.

## Key binding rules

The full key map is the module doc of `crates/typewriter-ui/src/input.rs`; keep the README's *Controls* in
step.

## Mouse

Controls never take keyboard focus: sense clicks with `render::CLICK`, not `Sense::click()` — egui moves
focus with Tab and clicks a focused control on Enter, so the typewriter's own keys would press it.

## Architecture rules

- `typewriter-core` must not depend on `egui`, `eframe`, audio crates, or the filesystem (except through
  `serde` types); the app handles all side effects. The core emits **events** (`Bell`, `CarriageReturn`,
  `KeyStrike`, `PageEnd`) for the app to turn into sounds — never audio from the core.
- Machine characteristics belong in profile data (`profiles/*.toml`), not constants.
- The app decides in its model, the flat desk of folders and notes (`app/desk/` — not the desk
  edition), which knows no egui, sound or window: views push `Intent`s, `app/mod.rs` does the
  `Effect`s. Test app behaviour there (`desk/testing.rs`).
- The app's modules stay private unless an edition or `typewriter-import` needs them.
- The command line is Linux only, in `terminal.rs`; commands are flags (`--import`), never bare words: a
  bare word is a project file. On Windows `typewriter` has no console, so `typewriter-import` is the one
  console program.

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
cargo run                     # typewriter, the desk edition
cargo run --bin typewriter-plain   # the plain app
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

A change is done when fmt, clippy and tests pass; the workspace lints plus `-D warnings` deny
`unwrap`/`expect` outside tests. Look at a drawing change by snapshot, not by guessing. The CPU twin —
the `snapshot` feature, which fills the desk's depth pass without a display — draws the typing views of
**both editions** to PNGs, scene after scene; the desk's `stage.rs` test holds the shots and is where a
new scene gets added:

```sh
TYPEWRITER_SNAPSHOT=<folder> cargo test -p typewriter --release -- --ignored snapshot
```

How the scene *looks*, through a live wgpu device, is `depth::gpu::tests::gpu_snapshot`: one
`tall-<backend>.ppm` per backend that answers — driver-dependent, so not for CI:

```sh
TYPEWRITER_GPU_SNAPSHOT=<folder> xvfb-run -a cargo test -p typewriter -- --ignored --nocapture gpu_snapshot
```

### CI

`checks.yml` runs fmt, clippy and tests on every push — keep it in step with the commands above. Check a run where CI is the only witness (a `vX.Y.Z` tag, a change to `release.yml` or the packaging) and report a 
failure with its log (`gh`, the GitHub MCP server, or the API). `scripts/smoke-test.sh` proves a build opens a window wherever `xvfb-run` is installed.

### Releases

Semver on `version` in `Cargo.toml`. To release: bump it, commit and
push, wait for Checks, then push an annotated tag `vX.Y.Z` whose message is the release notes.
`release.yml` builds Linux (glibc and musl) and Windows, each Linux build smoke-tested first, and
publishes once every artifact is attached. `release.yml` and `windows.yml` can be run by hand for dry
runs.

## Code style

- `rustfmt` defaults, idiomatic naming, edition 2024 idioms.
- Small, pure functions in the core. Unit tests sit next to the code; scenarios go in `tests/`.
- Errors: `thiserror` in the core, `anyhow` at the app boundary.
- No `unwrap()`/`expect()` outside tests unless the invariant is stated at the call site.
- Reuse before adding.
- Name units: `_seconds`, `_mm`, `_percent`, `half_line`; or say them in the doc comment.

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
