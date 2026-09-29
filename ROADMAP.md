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
- [ ] Machine rules in the settings: backspace, margin lock, free movement, correction Off in
      the cycle (defaults for new projects, also applied to the project in the machine)

## M12: Mechanics

- [ ] ❓ Platen knob: roll the paper up or down by half lines (superscripts, footnote marks)
- [ ] ❓ Dead keys: accents strike without moving the carriage, so é, ü, ñ are overtyped
- [ ] ❓ Type jams: two keys struck almost together tangle their typebars and print nothing
      until freed (a strictness setting, off by default)
- [ ] Shift misalignment: capitals sit a hair off the baseline, as on a worn machine (part of
      ink realism)

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
