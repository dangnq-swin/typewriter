# Roadmap

Milestones are ordered. Each one should leave the app in a working, testable state.
Items marked ❓ need input from the maintainer before implementation starts.
M0–M9 are done; the git history has the details.

---

## M10: Scratchpad

- [x] A 48-page pocket memo book (dot grid, kraft cover), written in pencil, opened with the
      1 / ! key (typewriters have no such key), the notebook icon beside the folder icon, or
      the book lying true to scale beside the folder in the folder view. It slides up in the
      bottom right corner: page 1 alone, then two pages to a spread. Page Up / Page Down or the
      corner arrows turn leaves; a full page runs on to the next. One per project, saved with
      the spread it lies open at in the folder file (format version 4), left out of exports.
      Esc or a click away closes it

## M11: Margins & machine rules

- [x] Margin release on Home, margins set at the carriage with Shift+Home / Shift+End; on
      the scale, click a stop to release, drag one to move it
- [x] Margin-release indicator on the scale: the stops stand lifted and faded
- [x] Machine rules in the settings (`[machine.rules]`): backspace, margin lock, free
      movement, and Delete (digital, traceless) in the correction cycle. Defaults for new
      projects, also applied to the project in the machine; a reopened project keeps its own

## M12: Mechanics

- [x] Platen knob: Up / Down roll the paper a half-line whatever the rules (superscripts,
      footnote marks); knobs either side of the paper turn with a drag or the wheel
- [x] Dead keys, per machine profile (`dead_keys`; the SM9 has none): the accent strikes
      without moving the carriage, é from the keyboard is typed as accent then letter and
      drawn as the font's é. Without the dead key, accented letters don't type
- [x] Type jams: a rule (off by default). Two strikes within 30 ms tangle, and only
      Backspace frees them (folder format version 5)
- [x] Shift misalignment: with ink realism, capitals sit a hair above the line

## M13: Paper handling

- [x] Roll a finished sheet back in from the folder (Sheet… → Roll back in) and keep typing
      on it, out of line by a random third of a cell per feeding; fed out, it goes back to
      its place (folder format version 6)
- [x] Copy holder: put a finished sheet on a stand left of the machine (Sheet… → Put on the
      copy holder) and read it while typing, under a line guide moved by clicking a line.
      Not saved with the project

## M14: Writing log

- [x] Words per day as a small calendar in the folder view: a month tent calendar standing
      beyond the scratchpad, brought up close with a click, each day's net words pencilled in
      (days in local time, weeks from Monday), turned back month by month to the first day
      written on. The folder file keeps words per day in place of sessions (format version 7;
      older files' sessions are folded into their days)

## M15: Windows builds for testers

- [x] Projects, drafts and settings kept where Windows keeps them (`%APPDATA%\typewriter`)
      instead of the XDG paths, which Windows lacks; paths shown in full, not as `~`
- [x] No console window beside the app on Windows; `typewriter-import`, a console program of its
      own, imports there
- [x] Typing on Windows keyboard layouts, dead keys included, checked by a tester
- [x] CI (GitHub Actions): fmt, clippy and tests on Linux for every push; a Windows build,
      started by hand, that runs the tests and leaves the app as a download on the run

## M16: Desktop integration

- [x] Open projects from the file manager: `*.typr` files of type
      `application/x-typewriter-folder`, set up by `scripts/install.sh`. Opened while the app is
      open, a project goes to it, asking first about unsaved work
- [x] Folder format 1.0, versioned major.minor: format 8 pinned, older files no longer opened
      (`scripts/convert-format-8.sh` converts format 8)
- [ ] Print: send the PDF export to a printer
- [ ] Verify and support X11 (Wayland is the current target)
- [ ] macOS builds

## M17: Look

- [ ] Configurable background: own texture path or a flat colour instead of the bundled
      paper photo
- [ ] Night desk: a dim room with a lamp's pool of light on the paper
- [ ] Carriage-return lever animation

## M18: Ribbon

- [ ] ❓ Two-colour ribbon (black / red) switch
- [ ] Ribbon wear: ink fades gradually and is refreshed by "changing the ribbon"

## M19: More machines

- [ ] Profiles with their own sounds and typeface
- [ ] Elite (12 cpi) type option for the SM9
- [ ] ❓ Additional profiles (e.g. Olivetti Lettera 32, Hermes 3000, IBM Selectric), each with
      its own pitch, typeface, bell offset and sounds

## M20: Paper

- [ ] ❓ Other stock: onion skin, coloured bond, US Letter beside A4, chosen when feeding a
      sheet (a real stack of paper beside the machine)
- [ ] ❓ Index cards and envelopes: small stock fed the same way, typed on and filed

## M21: Filing away

- [ ] ❓ Export as .odt, the other half of `typewriter import`
- [ ] ❓ Packaging: Flatpak and an AppImage, beside `scripts/install.sh`

## M22: Testers

- [ ] ❓ A round of fixes for what trips up the first testers, on the M15 builds. Choices held
      back for their feedback (key rebinding among them) are settled afterwards

---

# The desk edition

The immersive, game-like side of the project lives apart from the typewriter app so that the
plain app stays a focused writing tool. It is a separate crate in this workspace (a second
binary), sharing `typewriter-core` and the app's drawing, so that every fix reaches both.
The design principles in `AGENTS.md` hold here as well: only what a real office has.

## D1: A second binary

- [ ] Split `typewriter-app` into a library (machine, paper, folder, audio, filing) and a thin
      binary, so a second crate can build on it
- [ ] `crates/typewriter-desk`: a binary that opens the same projects and settings, and can
      be installed beside `typewriter`

## D2: A desk in perspective

- [ ] ❓ The desk as a scene rather than fixed screen positions: the copy holder stands left of
      the document at a fixed angle, and the view (position, field of view) can be moved and
      zoomed, more like a game camera than a page on screen

## D3: Around the desk

- [ ] ❓ Drawers holding the projects: open one by pulling its folder out
- [ ] ❓ A wastepaper basket for scrunched sheets
- [ ] ❓ A shelf of machines: change profile by lifting another typewriter onto the desk

## D4: Nothing but the desk

The app's icons, plates and menus become the things they stand for. Built in stages, easiest
first, each one tried on its own before the next.

### Stage 1: What is already drawn, lying out

- [ ] ❓ The open folder and the memo book on the desk in place of their icons, a finished
      sheet landing on the folder's stack
- [ ] ❓ Sounds for the desk: a sheet sliding onto the stack, the book set down, the calendar
      turned (new CC0 assets)
- [ ] ❓ The tent calendar standing on the desk, not only in the folder view

### Stage 2: Simple things beside the machine

- [ ] ❓ The copy holder's stand always there; lift the sheet off it instead of the ×
- [ ] ❓ A stack of blank paper: take a sheet from it to feed one in (Insert still works)

### Stage 3: Tools on the desk

- [ ] ❓ Correction tools in place of the Correct plate: correction paper tabs, a typewriter
      eraser with its brush, a bottle of fluid. Pick one up to use it
- [ ] ❓ Goals in place of the Goal plate: an egg timer for minutes, a tally slip for words

### Stage 4: Sheets by hand

- [ ] ❓ Drag a finished sheet onto the machine to roll it back in, or onto the stand
- [ ] ❓ Renumber a sheet by pencilling on its corner

### Stage 5: The machine itself

- [ ] ❓ The typewriter drawn around the paper: the paper bail's scale, the margin rack, the
      platen knobs, the card holder's pointer
- [ ] ❓ Its own levers and keys in place of plates and chords: the line-space lever, tab set
      and clear keys

## Someday: A walkable office

- [ ] ❓ An immersive desk you can get up from: walk over to a photocopier for copies of
      finished sheets (instead of carbon copies at the machine)
