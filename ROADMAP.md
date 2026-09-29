# Roadmap

Milestones are ordered. Each one should leave the app in a working, testable state.
Items marked ❓ need input from the maintainer before implementation starts.
M0–M8 are done; the git history has the details.

---

## M9: Release (Linux)

- [ ] README with screenshots and usage

## M10: Scratchpad

- [ ] ❓ A scratchpad opened with the 1 / ! key (typewriters have no such key, so it types
      nothing on the machine). Details to be decided

## M11: Margins & machine rules

- [ ] ❓ Key bindings for margin release and for setting the left / right margin at the
      carriage (the core supports both)
- [ ] Margin-release indicator on the scale
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
