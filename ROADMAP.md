# Roadmap

What comes next, in order. Each step should leave the app in a working, testable state. Items
marked ❓ need input from the maintainer before implementation starts. Finished work leaves this
file: the git history and the release notes keep it.

---

## Look

- [ ] Night desk: a dim room with a lamp's pool of light on the paper
- [ ] Carriage-return lever animation

## Ribbon

- [ ] ❓ Two-colour ribbon (black / red) switch
- [ ] Ribbon wear: ink fades gradually and is refreshed by "changing the ribbon"

## More machines

- [ ] Profiles with their own sounds and typeface
- [ ] Elite (12 cpi) type option for the SM9
- [ ] ❓ Additional profiles (e.g. Olivetti Lettera 32, Hermes 3000, IBM Selectric), each with
      its own pitch, typeface, bell offset and sounds

## Paper

- [ ] ❓ Other stock: onion skin, coloured bond, US Letter beside A4, chosen when feeding a
      sheet (a real stack of paper beside the machine)
- [ ] ❓ Index cards and envelopes: small stock fed the same way, typed on and filed

## Filing away

- [ ] ❓ Export as .odt, the other half of `typewriter import`
- [ ] ❓ A truly static Linux build: CPU drawing, pure-Rust Wayland and X11, sound without ALSA
      (a static binary can't load the system's graphics and window libraries)
- [ ] ❓ A Flatpak

## Testers

- [ ] ❓ A round of fixes for what trips up the first testers, on the release builds. Choices
      held back for their feedback (key rebinding among them) are settled afterwards

---

# The desk edition

The immersive, game-like side of the project lives apart from the typewriter app so that the
plain app stays a focused writing tool. It is a separate crate in this workspace (a second
binary), sharing `typewriter-core` and the app's drawing, so that every fix reaches both.
The design principles in `AGENTS.md` hold here as well: only what a real office has.

## A second binary

- [ ] `crates/typewriter-desk`: a binary on the app's library that opens the same projects and
      settings, and can be installed beside `typewriter`

## A desk in perspective

- [ ] ❓ The desk as a scene rather than fixed screen positions: the copy holder stands left of
      the document at a fixed angle, and the view (position, field of view) can be moved and
      zoomed, more like a game camera than a page on screen

## Around the desk

- [ ] ❓ Drawers holding the projects: open one by pulling its folder out
- [ ] ❓ A wastepaper basket for scrunched sheets
- [ ] ❓ A shelf of machines: change profile by lifting another typewriter onto the desk

## Nothing but the desk

The app's icons, plates and menus become the things they stand for, easiest first, each group
tried on its own before the next.

### What is already drawn, lying out

- [ ] ❓ The open folder and the memo book on the desk in place of their icons, a finished
      sheet landing on the folder's stack
- [ ] ❓ Sounds for the desk: a sheet sliding onto the stack, the book set down, the calendar
      turned (new CC0 assets)
- [ ] ❓ The tent calendar standing on the desk, not only in the folder view

### Simple things beside the machine

- [ ] ❓ The copy holder's stand always there; lift the sheet off it instead of the ×
- [ ] ❓ A stack of blank paper: take a sheet from it to feed one in (Insert still works)

### Tools on the desk

- [ ] ❓ Correction tools in place of the Correct plate: correction paper tabs, a typewriter
      eraser with its brush, a bottle of fluid. Pick one up to use it
- [ ] ❓ Goals in place of the Goal plate: an egg timer for minutes, a tally slip for words

### Sheets by hand

- [ ] ❓ Drag a finished sheet onto the machine to roll it back in, or onto the stand
- [ ] ❓ Renumber a sheet by pencilling on its corner

### The machine itself

- [ ] ❓ The typewriter drawn around the paper: the paper bail's scale, the margin rack, the
      platen knobs, the card holder's pointer
- [ ] ❓ Its own levers and keys in place of plates and chords: the line-space lever, tab set
      and clear keys

## Someday: A walkable office

- [ ] ❓ An immersive desk you can get up from: walk over to a photocopier for copies of
      finished sheets (instead of carbon copies at the machine)
