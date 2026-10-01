# Roadmap

What comes next, in order: the desk edition's machine first, as that is the work under way;
then small, important changes to both editions; then larger features, the biggest last. Each
step should leave the app in a working, testable state. Items marked ❓ need input from the
maintainer before implementation starts. Finished work leaves this file: the git history and the
release notes keep it.

The desk edition (`typewriter-desk`) is the immersive, game-like side of the project, kept apart
so that the plain app stays a focused writing tool (see `AGENTS.md`). Items for it say so.

---

## The desk's machine

On the `desk-viewpoint` branch.

- [ ] Desk: the rest of the machine in real 3D, on the depth pass the body, carriage and
      sheets now draw on (one wgpu paint callback a frame): the keyboard and its case and the
      side controls, still flat over it. Then the flat sheet's ways leave the desk (its paper
      table's placing, the curl and lift). Text where the sheet comes nearer the eye than the
      typing line, toward its top, is laid out larger to stay as sharp
- [ ] Desk: the margin rack behind the paper support, its two stops set there rather than
      on the bail's scale. The sheet hides it: drag the sheet's top down to fold it back, let go,
      then set the stops. Dragging it up again, or the next key typed, unfolds it
- [ ] ❓ Desk: its own levers and keys in place of plates and chords: the line-space lever, tab
      set and clear keys
- [ ] ❓ Desk: the carriage lock and the touch control beside the keyboard work. The ribbon
      selector beside them waits for the two-colour ribbon
- [ ] ❓ Close the try-out: merge `desk-viewpoint` into `main`
- [ ] `typewriter-desk` installed and packaged beside `typewriter` (`install.sh`, the release
      builds, its own desktop entry)

## The desk's calm mode

- [ ] ❓ Desk: calm mode redesigned for the desk edition. Calm now fades only the app's chrome
      (the scale's marks, the desk icons); the machine stays whole, its knobs and panel controls
      among it, as a depth pass can't fade. Decide what calm means at a desk, and how it shows

## Small and important

- [ ] ❓ Export as .odt, the other half of `typewriter --import`, beside the text and Markdown
      exports
- [ ] Elite (12 cpi) type option for the SM9
- [ ] ❓ Two-colour ribbon (black / red) switch: colour in the folder format. On the desk, the
      ribbon selector beside the keyboard sets it (blue for black, white for stencil, red)
- [ ] ❓ A round of fixes for what trips up the first testers, on the release builds. Choices
      held back for their feedback (key rebinding among them) are settled afterwards

## Nothing but the desk

The desk edition's icons, plates and menus become the things they stand for, easiest first,
each group tried on its own before the next.

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

- [ ] ❓ Correction tools in place of the Correct control: correction paper tabs, a typewriter
      eraser with its brush, a bottle of fluid. Pick one up to use it
- [ ] ❓ Goals in place of the Goal control: an egg timer for minutes, a tally slip for words

### Sheets by hand

- [ ] ❓ Drag a finished sheet onto the machine to roll it back in, or onto the stand
- [ ] ❓ Renumber a sheet by pencilling on its corner

## Ribbon, paper and light

- [ ] Ribbon wear: ink fades gradually and is refreshed by "changing the ribbon"
- [ ] ❓ Other stock: onion skin, coloured bond, US Letter beside A4, chosen when feeding a
      sheet (a real stack of paper beside the machine)
- [ ] ❓ Index cards and envelopes: small stock fed the same way, typed on and filed
- [ ] ❓ Desk: night, a dim room with a lamp's pool of light on the paper

## More machines

- [ ] Profiles with their own sounds and typeface
- [ ] ❓ A machine's look in its profile, built with the second machine: a `[look]` table the app
      reads (knob colours, the scale), the SM9's by default; and a `model` naming the desk
      edition's drawing of it. Open: what the desk draws for a profile without one
- [ ] ❓ Additional profiles (e.g. Olivetti Lettera 32, Hermes 3000, IBM Selectric), each with
      its own pitch, typeface, bell offset and sounds

## Packaging

- [ ] ❓ A Flatpak
- [ ] ❓ A truly static Linux build: CPU drawing, pure-Rust Wayland and X11, sound without ALSA
      (a static binary can't load the system's graphics and window libraries). The desk's 3D
      needs a GPU through wgpu: drawn on the CPU there, or left out of that build

## The desk as a scene

- [ ] ❓ Desk: a scene rather than fixed screen positions: the copy holder stands left of the
      document at a fixed angle, and the view (position, field of view) can be moved and zoomed,
      more like a game camera than a page on screen, on the desk's 3D drawing
- [ ] ❓ Desk: drawers holding the projects: open one by pulling its folder out
- [ ] ❓ Desk: a wastepaper basket for scrunched sheets
- [ ] ❓ Desk: a shelf of machines: change profile by lifting another typewriter onto the desk

## Someday: a walkable office

- [ ] ❓ Desk: an immersive desk you can get up from: walk over to a photocopier for copies of
      finished sheets (instead of carbon copies at the machine)
