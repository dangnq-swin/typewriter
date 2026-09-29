# Roadmap

Milestones are ordered. Each one should leave the app in a working, testable state.
Items marked ❓ need input from the maintainer before implementation starts.
M0–M8 are done; the git history has the details.

---

## M9: Release (Linux)

- [ ] README with screenshots and usage

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

- [ ] Roll a finished sheet back in from the folder and keep typing on it, slightly out of
      line as a re-fed sheet is
- [ ] ❓ Copy holder: put a finished sheet on a stand beside the machine and read it while
      typing (e.g. retyping a page), with a line guide to keep your place
- [ ] ❓ Carbon copy: type with a carbon sheet for a faded duplicate in the folder
- [ ] ❓ Paper sizes: US Letter and index cards

## M14: Writing log

- [ ] Words per day as a small calendar in the folder view

## M15: Desktop integration

- [ ] Open `*.folder.ron` files from the file manager (a MIME type)
- [ ] Print: send the PDF export to a printer
- [ ] Verify and support X11 (Wayland is the current target)
- [ ] Windows and macOS builds
- [ ] ❓ Packaging: Flatpak, AppImage or a distro package (e.g. a Gentoo ebuild)

## M16: Look

- [ ] Configurable background: own texture path or a flat colour instead of the bundled
      paper photo
- [ ] Night desk: a dim room with a lamp's pool of light on the paper
- [ ] Carriage-return lever animation

## M17: Ribbon

- [ ] ❓ Two-colour ribbon (black / red) switch
- [ ] Ribbon wear: ink fades gradually and is refreshed by "changing the ribbon"

## M18: More machines

- [ ] Profiles with their own sounds and typeface
- [ ] Elite (12 cpi) type option for the SM9
- [ ] ❓ Additional profiles (e.g. Olivetti Lettera 32, Hermes 3000, IBM Selectric), each with
      its own pitch, typeface, bell offset and sounds

## M19: Key rebinding

- [ ] ❓ Rebindable keys, still held to the key binding rules in `AGENTS.md`
