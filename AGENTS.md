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
| Calm mode | Distraction-free mode: lines are dimmed progressively the further they are from the line being typed, **strongly**: from full ink on the typing line down to 10 % about 4 lines away (a smooth curve, by distance on the sheet). It hides the carriage scale, the plates below it and the folder icon; the margin frame, the typing pointer and the calm mode icon stay. Chrome and dimming fade in and out together over 250 ms. **Esc** turns it on and off in the typing view (in the folder, Esc still goes back); a small icon right of the folder icon also turns it on and off, and stays shown in calm mode (it does not fade). The falloff (default 4 lines, 1–10) and the faintest ink (default 10 %, 0–60 %) are settings |
| Fullscreen | **F11** toggles fullscreen, on its own (calm mode does not change it). Keyboards without F11 use the desktop's own shortcut. F11 works while a sheet is being fed |
| Sounds | Key strike, carriage return, margin bell, plus blocked input, space bar, backspace, tab, sheet feed, eraser, correction fluid (a brush dab; a strike through the correction slip sounds like any strike), scrunching up a sheet (a burst of newspaper crumpling, 0.9 s), and a platen ratchet click per line when the paper is rolled (four variants). Played with **rodio**. Keys have several variants (never the same twice in a row); the bell picks one of its samples at random |
| Silent mechanisms | A profile's `[sounds]` table can silence a mechanism: `carriage_return` (Enter pressed) and `line_feed` (Enter held, rolling the paper), both on by default. The **SM9's carriage return is silent**, so its return sound is bundled but not played; its platen ratchet is audible |
| Sound source | **CC0 samples** (BigSoundBank, Freesound), proposed by the agent and approved by the maintainer. `scripts/prepare-sounds.sh` downloads them to `target/` and cuts the WAV clips in `assets/sounds/`, which are embedded in the binary. Freesound clips are from its 128 kbps previews (originals need a login) |
| Volume / mute | In the settings: **master volume** (0–100 %, loudness following the square of the slider) and **Mute**, plus toggles for six groups: Keys (strikes, space bar, backspace, tab), Margin bell (also the goal bell), Platen & carriage return (ratchet clicks, and the return where the machine makes one), Sheet feed (and scrunching up), Corrections (eraser, fluid), Blocked input. A profile's silent mechanisms stay silent whatever the settings |
| UI text | **Labels, not explanations.** Controls carry short labels; no notes explaining behaviour the controls already show or that is documented elsewhere (e.g. where profile files go, what is remembered). Hover tooltips on the plates are fine. Problems (a broken profile, a missing machine) are still shown |
| Settings | A **gear icon** right of the calm mode icon (fades in calm mode like the folder icon; not while a sheet is fed) opens a **settings card** on the dimmed desk: one scrolling card with sections Sound, Look & calm, Goals, Machine, Projects (Autosave), and **Reset to defaults**. Changes apply at once; there is no OK/Cancel. Esc (unless a field is being edited), the gear or Close go back; keys do not reach the machine meanwhile. Saved to **`$XDG_CONFIG_HOME/typewriter/config.toml`** (TOML via `serde` + `toml` in the app), crash-safe, 0.5 s after the last change and on quit. Every key has a default, unknown keys are ignored and hand-edited values are clamped; a file that cannot be read is renamed to `config.toml.unreadable` before the first write, with a notice. **Machine rules** (backspace, margin lock, free movement, correction Off) are not in the settings yet (Later); when added, they are the defaults for new projects and also apply to the project in the machine, which keeps its own |
| Profiles | Built-in profiles (the SM9) plus **user profiles**, data only: `$XDG_DATA_HOME/typewriter/profiles/*.toml`, read at start and when the settings open, in file name order. A user profile cannot reuse a taken name; broken files are listed with the reason in the settings. They use the bundled sounds and font. The settings' **Machine** section chooses the machine for **new projects** (default SM9; a missing one falls back to it) and shows its pitch, paper and grid. A project keeps its machine (by name) and opens only if that profile is found. Schema documented in `docs/profiles.md` (its example is tested) |
| Focus goals | One goal per **session**, set on a **Goal plate** right of the Correct plate: clicking it cycles Off → 250 / 500 / 1000 words → 15 / 25 / 50 minutes → Off (hidden in calm mode like the other plates; no key). A session runs from putting a project in the machine (launch, New, Open) until quitting or switching projects; the goal carries over to the next project's session. **Words** are net: words readable on the sheets now less those at the start (a word is a run of visible characters on a line holding a letter or digit; corrected letters drop out). **Time** is typing time: the gaps between keys, leaving out any gap over 60 s, counted as each key is struck. The plate reads e.g. "Goal: 312 / 500 words" or "Goal: 12 / 25 min"; when the goal is reached the **margin bell rings once** and the plate shows a check mark (choosing a goal already met shows the check without the bell). The chosen goal is remembered in the settings. The settings also take a **custom goal**: a word target and a minute target, each switched on with its own checkbox (defaults 750 words and 30 min, both off). With one on it is that goal; with **both on** it is one goal, "N words or M minutes", **reached by whichever comes first** (the plate reads e.g. "Goal: 312 / 750 words or 12 / 30 min"). It comes last in the plate's cycle (skipped if equal to a preset). Editing the custom goal while it is the one chosen changes the chosen goal with it; a remembered goal no longer in the cycle is followed by the first |
| Session stats | Kept in the project file (`sessions`, format version 2; version 1 files load with none): per session its start (Unix seconds), seconds typed and net words, recorded only once something was typed. The folder view shows the totals below the path, e.g. "4 sessions · 2 h 10 min · 1,840 words" |
| Font | **Courier Prime** (SIL OFL), vendored in `assets/fonts/`. Closest free match to the SM9's slab-serif Modern Pica; no libre font reproduces it exactly. Notes use **Caveat** Regular (SIL OFL, static cut from googlefonts/caveat), also vendored |
| Sheet order and scrunching | Both act on the **chosen (pulled-out) sheet** in the folder, from the **Sheet…** plate (disabled with no finished sheets). **Renumber…** turns the sheet's number into a small field (digits only, hint "1–N"): Enter moves it to that position and the others shift along, Esc or clicking elsewhere leaves it; a number out of range changes nothing (the field's hint shows the range). **Shift + arrow keys** in the folder move the chosen sheet one place (up/left = older, no key repeat); elsewhere Shift+arrows act as plain arrows. The chosen sheet follows its move. **Scrunch up…** (or **Delete** in the folder) asks first in a dialog, "Scrunch up sheet N? It can't be smoothed out again." (Scrunch up / Keep it; Esc or clicking outside keeps it; keys do not reach the machine meanwhile). Confirmed, the sheet is removed from the project for good (no wastebasket), crumples into a lumpy ball with creases where it was pulled out and is tossed off the desk to the lower left, over 0.9 s with the crumple sound; the next sheet slides out in its place. Session words are counted again afterwards. Only finished sheets |
| Autosave plate | A plate under the **right end of the carriage scale**, flush with it and travelling with it (on a row of its own below if the left plates reach that far), hidden in calm mode like the other plates: "Autosave: On" with a status dot, **green** when saved, **amber** for 0.4 s after a write, **red** if the last write failed (it retries on the next change; the tooltip gives the reason); "Autosave: Draft" for an unsaved draft; "Autosave: Off" with autosave turned off. The tooltip says where it is saved or what is unsaved. **Clicking it** saves now, or opens Save As for a draft |
| Notes | A finished sheet can carry **one pencilled note in its top margin**. In the open (read) sheet, **clicking the top margin** (the hint line below the sheet says so; while writing it shows the writing keys instead) starts writing straight on the paper in Caveat, soft graphite, 0.2 in em; Enter starts a new line; clicking elsewhere or Esc finishes (keys do not reach the machine or flip sheets meanwhile). It runs from the left to the right margin, wraps, and is limited to the lines the top margin has room for (3 on the SM9; input beyond is refused). On finishing, the note is kept **as written**, one line per written line (wrapped lines included), so screen and PDF match; trailing blanks are dropped, and an empty note clears it. Drawn tilted up to 0.8°, the same for a sheet every time (straight while being written). In the folder, a note shows as faint pencil strokes on the top and pulled-out sheets. Only finished sheets take notes. Stored per page (`note`, format version 3; older files load without). Exports: PDF draws it as on screen (Caveat embedded only when a note exists); plain text puts it first in square brackets, Markdown as a `>` quote, followed by a blank line |
| CI/CD | None for now |
| Release | **`scripts/install.sh`** builds (release, `--locked`) and installs for the current user under **`PREFIX`** (default `~/.local`, no sudo): `bin/typewriter`, `share/applications/typewriter.desktop` (named after the window's app id; `Exec` is the full path, since `~/.local/bin` is not always on the desktop's PATH; opens a file given to it) and the icon `share/icons/hicolor/scalable/apps/typewriter.svg`. It refreshes the desktop database, and GTK's icon cache only if one already exists. `--uninstall` removes those three files, never projects, drafts or settings. No MIME type for `*.folder.ron`. Also installable with **`cargo install`** (`--path crates/typewriter-app` or `--git`, binary only). Every asset is embedded, so the binary stands alone. Building needs the ALSA headers (`alsa-lib`) and `pkg-config`. **No packaging** for now (Flatpak, AppImage, distro packages are Later). The **icon** (`assets/icons/typewriter.svg`) was drawn for the project: a sheet with typed lines in a platen, on a manila folder. The README's Controls section is kept up to date with the key bindings; the rest of the README is the maintainer's |
| Storage | Own native document format (keeps strikeouts, overtyping and session stats) with **export** to Markdown, plain text and PDF. **RON** (`ron` crate, in the core: it (de)serialises strings, the app does the file I/O), versioned (`version` field). One document = one **project**, saved as a file `<name>.folder.ron` and shown as the manila folder: its sheets, the sheet in the machine and the carriage (position, margins, tab stops, spacing, correction method), the session stats, the notes on sheets, plus the profile name |
| Projects | The UI says **project** (not folder or document). The user names and places project files themselves, through the desktop's own dialogs (**`rfd`**, XDG portal backend). A new project is an **unsaved draft** ("Untitled"), autosaved to `$XDG_DATA_HOME/typewriter/drafts/` until **Save As…** moves it to the chosen name and place. Autosave is crash-safe (temp file + rename): after 2 s without a change, on every sheet feed and on quit. **Autosave can be turned off** in the settings (Projects): a saved project is then written only by Save (the folder menu or the Autosave plate); drafts are still cached in the drafts folder. **Leaving a project** (closing the window, New project, or Open project once a file is picked) asks first when work would be put away unsaved: a draft with something typed gets "Keep this draft?" (Save As… / Keep as draft / Discard / Cancel; Save As then leaves once saved, a cancelled dialog stays), and with autosave off, unsaved changes get "Save changes to “name”?" (Save / Don't save / Cancel). An untouched draft is not asked about. Closing is held back (`CancelClose`) until answered. **Crash detection**: a `running` marker (the process id) in `$XDG_DATA_HOME/typewriter/` is written at start and removed on a clean exit; found at start with that process gone, the last run crashed, and the reopened project comes with the notice "Recovered your work from when Typewriter last closed unexpectedly." On launch the **last project** is reopened (its path is kept in `$XDG_DATA_HOME/typewriter/last-folder`); a path on the command line opens that project instead. The **folder view** shows the project's path above the folder and its name on the tab, and four menu plates below it: **Current project…** (Save: writes now and says so, or Save As for a draft; Save As…; Rename), **Other projects…** (New project, Open project…), **Sheet…** (Renumber…, Scrunch up…; see *Sheet order and scrunching*) and **Export…** (To Markdown, To Text file, To PDF). **Rename** is inline: the tab becomes a text field (Enter renames the file where it is, Esc cancels; keys do not reach the machine meanwhile); clicking the tab renames too, or saves a draft as. Rename and Export are disabled for a draft |
| Export | Markdown (`.md`), plain text (`.txt`) and **PDF** (`.pdf`), written **next to the project file** with the same name (a draft must be saved first). Text exports keep only visible glyphs: corrected letters are left out, and an overstruck cell exports its top glyph, except `'` over `.`, which exports as `!`. Sheets are separated by a form feed in plain text and by `---` in Markdown; blank lines follow the vertical space on the sheet; the left margin is trimmed. The **PDF** is the sheets **as typed, on white**: one page per sheet at paper size, Courier Prime embedded (subset), with ink realism, overstrikes and corrections drawn exactly as on screen (the same drawing list), no margin frame. Made with **`printpdf`** (default features off) |
| Core dependencies | `serde` (derive), `thiserror`, `toml`, `ron` in `typewriter-core`. Core parses TOML and RON from strings; the app does the file reads and writes |
| Vertical position | Tracked in **half-line** steps (1/12 in at 6 lpi), like the platen ratchet. Line spacing 1 / 1.5 / 2 = 2 / 3 / 4 half-lines |
| Default strictness | Authentic: Backspace moves the carriage back without erasing (overtyping), right margin locks until the one-shot margin release (cleared on return), free cursor movement off |
| Erasing | Separate **Erase** action (Shift+Backspace or Delete), with three period methods. Corrections cover what was struck, which stays in the cell's stack, and can be typed over. **`paper`** (correction paper, the default): Erase puts the slip in front of the ribbon (drawn translucent over the typing point) and takes it out again; characters struck meanwhile advance the carriage and are covered in chalk in their own shape, a little fuller than the type, so retyping the wrong letter whitens it. Chalk hides only glyphs of the character struck, so an overstruck character (e.g. `!` typed as `'` + Backspace + `.`) needs each of its parts struck again through the slip; the eraser and fluid cover the whole cell. Backspace works as usual; switching method or feeding a sheet takes the slip out. **`eraser`**: Erase steps back and rubs out the character, leaving a 12 % ghost of the ink and a pale, streaked scuff. **`fluid`**: Erase steps back and dabs on an uneven, slightly spilling blob that dries in **3 s**, glossy and brighter while wet; a strike on wet fluid **smudges** (blurred, fainter ink; the app tells the core when a dab has dried). `off` blocks Erase. There is no digital (clean) removal and no lift-off tape |
| Correction plate | Plate right of the Zoom plate (the Goal plate follows it): "Correct: Paper" ("Paper (slip in)" while the slip is in), "Eraser" or "Fluid". **Clicking it or F4** cycles Paper → Eraser → Fluid; Off comes with the machine rules settings (Later). Hidden in calm mode like the other plates |
| SM9 defaults | A4 at Pica = 82 columns x 70 lines. Left margin col 10, right margin col 72 (locks before it), bell 8 columns before the right margin, top margin 6 lines |
| Page end | A return or line feed on the last line emits `PageEnd` (the core does not feed). The app answers it by **feeding a new sheet**, as Insert does, since many compact (e.g. 60 %) keyboards have no Insert key |
| Keys | **No Ctrl bindings** (a typewriter has no Control key), except **Ctrl+S = save** (Save As for a draft; in every view, also while a sheet is fed; no repeat), kept for muscle memory; Shift is fine. **No 1 or ! key**, as on typewriters: 1 is typed as a lowercase l, and ! as ' + Backspace + . (the characters 1 and ! are ignored, whatever key or layout sends them). The key is kept free for the scratchpad (M10). Insert = feed a new sheet. F1 / F2 / F3 = line spacing 1 / 1.5 / 2. F4 = next correction method. Esc = calm mode on/off (typing view) or back (folder); an open menu (the folder's plates, a dropdown in the settings) closes first, and only the next Esc goes back. F11 = fullscreen. Backspace = carriage back (no erase). Shift+Backspace or Delete = Erase; **in the folder, Delete scrunches up the chosen sheet** (after the same confirm dialog as Sheet… → Scrunch up…). Enter = return; **holding Enter rolls the paper** on a line per key repeat (`LineFeed`: platen knob, carriage stays put), into a new sheet at the page end. Space, Backspace, Shift+Backspace, Delete, Insert, Tab, Esc, F4 and F11 ignore key repeat. In the folder, Shift + arrow keys move the chosen sheet one place. Hold Shift and tap Tab: 1× sets a tab stop, 2× clears the nearest stop within 3 columns, 3× clears all (acts on Shift release). Full list in `crates/typewriter-app/src/input.rs`; rebinding is on the backburner |
| Platen view | The typing point stays fixed on screen. The paper scrolls up and, by default, slides sideways like the carriage (can be turned off). A single pointer just below the typing line marks the typing point |
| Blocked input | A short horizontal jolt of the sheet (plus a sound from M4) |
| Carriage scale | A ruler below the typing line, travelling with the carriage like the SM9 scale: column ticks and numbers, margin brackets, tab stop pointers |
| Spacing indicator | Plate labelled "Spacing:" right below the scale, flush with its left end (paper edge, travels with the carriage). Two circles: first filled; second empty (1), left half filled (1.5) or filled (2). **Clicking it** moves the lever one notch: 1 → 1.5 → 2 → 1 |
| Sheet | No fill of its own. A thin frame around the writing area, 1 mm outside the margins (just clear of the type), with its four sides extended to the paper's edges to show the sheet's width and height. It follows the carriage margins; its bottom edge is the bottom of the sheet, since type can go down to the last line (the profile has no bottom margin) |
| Texture bundling | `image` crate (JPEG only), as a build dependency and at runtime. `build.rs` evens out the original in `assets/paper/` and re-encodes it at JPEG quality 70, at most 2048 px per side (the GPU texture limit), into the binary; the original file stays as supplied |
| New sheet | **Insert** feeds a fresh sheet at any time; so does Return on the last line. Margins, tab stops and spacing carry over; the carriage starts at the top margin. A blank sheet is not filed, it just stays in the machine. A feed takes about 8.5 s, as two clips played back to back: `feed-out.wav` (steady platen ratchet clicks, one every 60 ms, 1.6 s) and `feed-in.wav` (Gate13 1:49–1:57, with the pauses between its parts — paper in, page turn, ratchet run — tightened to 0.15 s and 0.25 s). The script makes the clicks as long as the time the tightened pauses gave back, stretched by 40 % so the old sheet does not rush out. **All input is ignored until the feed has finished** (until the new sheet has settled, timed from the clips, with or without a sound device) |
| Sheet feed animation | During the clicks, the finished sheet winds up and out **at a steady speed** (its whole travel over the clicks' duration). Then the new sheet rises from the bottom of the window into place **in step with the knob turns** (driven by `feed-in.wav`'s loudness, so it moves only while the knob is heard). While moving, the sheet shows a soft shadow and a curled top edge that flattens as it lands. Once it lands (always at least 250 ms before its sound ends), its shadow fades out while the typing pointer fades in, **together over 250 ms**, and the feed is over: input unlocks then, without waiting for the quiet end of the sound (about 7.3 s after the key press with the bundled clips). Ruler and spacing plate stay put |
| First sheet | A **new document** starts by winding its first sheet in: the feed animation and `feed-in.wav` without the wind-out (there is no finished sheet), input locked until it settles. **Reopening** a project (on launch or with Open project) resumes as on a real typewriter: its current sheet winds in the same way, typed text and all, landing at the start position (top margin, carriage at the left margin); then the platen turns **one notch (half-line) every 60 ms**, a ratchet click each (the pace of the wind-out clicks), down to the line typing stopped at. The carriage stays at the left margin, so typing resumes at the start of that line. Input stays locked until it arrives |
| Finished sheets | Kept in the document and browsable in a **2.5D folder**: a manila folder in perspective on a dimmed desk, its sheets in a **neat stack** (aligned, newest on top; the folder keeps one size however many sheets it holds, and a thick stack packs its sheets tighter, rising at most 3 % of a sheet's height). The **chosen sheet slides out to the left**, a full sheet width, **turning 45° top-left**, over 250 ms, staying at its height in the stack, so the sheets above it still lie over it; it is outlined and numbered. Only the sheet on top of the stack (the second newest while the newest is pulled out) and a pulled-out one show their words, as faint ink bars (egui cannot draw text in perspective). The composition is shifted right so the folder and the pulled-out sheet are centred together. Sheets are chosen with the **keys only**: the arrow keys (up/left = older) or Page Up / Page Down. Enter or a click on the pulled-out sheet opens it read-only at full size, where the same keys flip sheets. Opened with Page Up or the folder icon at the bottom left, with the newest sheet chosen; Esc goes back one level; typing returns to the machine and types |
| Zoom | The **plain mouse wheel** (no keys) steps 50–200 % in 10 % steps, one step per notch; 100 % = 96 points per inch. The level shows as a percentage on a plate right of the Spacing plate; **double-clicking** it resets to 100 %. egui's own UI zoom keys are off. Remembered between runs in the settings |
| Ink realism | On by default, very subtle: each strike lands up to 0.4 pt off and prints up to 10 % lighter, derived from its position so it never changes on redraw. Toggled in the settings (also applies to PDF export) |
| Margin controls | On the backburner: no key bindings for margin release or setting margins yet (the core supports them) |

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
    ├── profiles.md             # machine profile schema, adding a machine
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
