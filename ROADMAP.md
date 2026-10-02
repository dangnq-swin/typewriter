# Roadmap

What comes next, in order. Reordered 2026-10: **foundations before features** —
close the desk work left open, measure it, cache what it rebuilds every frame,
restructure its drawing and click pipeline, and only then fix and add. Each
step should leave the app in a working, testable state. Items marked ❓ need
input from the maintainer before implementation starts. A finished item is
checked, not deleted, while its section has items still open; a section leaves
this file once all of it is done: the git history and the release notes keep it.

A change is done as the code says: fmt, clippy, tests, and the drawings looked
at by snapshot (`AGENTS.md`). Perf work is judged on the bench, not on feel.

---

## The desk's foundations: restructure, no change of look

- [x] Split `depth.rs` (1600 lines, four contracts in one file): the pass's
      life (`begin`/`gather`/`end`), the vertex packing and pipelines, the CPU
      rasterizer, and the lighting math the shader mirrors. The "keep in step
      with `depth.wgsl`" twins become a reviewable surface; the WGSL parse test
      sits with the lighting.
- [ ] Cache the print: `render::paper::sheet_marks` walks every visible sheet's
      cells afire a frame, in both modes — nothing in the app caches today.
      Keep a sheet's marks between a strike, a correction, a fluid drying and a
      sheet change. The core says when a sheet took a mark; answer the rest in
      the app.
- [ ] Desk: a click contract for the machine. The `Stage` hooks return bespoke
      rects — `[Rect; 5]` for the panel, `[Rect; 2]` for the knobs, the
      `Scale`'s own stops — and a new clickable part means a new hook. One
      named thing on the machine with its rect, `Intent`s for *taken* and
      *released*; the desk's levers, keys and rack below build on it rather
      than beside it.
- [ ] Desk: the machine's standing solids, built once in their millimetres and
      kept — the case, the cover, the panel, the keyboard — each moving part
      through its own transform (the carriage's slide, the knobs' turn, the
      throw, the bail's anchoring), never re-tessellated a frame
      (~317 solids a frame today). Zoom and the typing line change only the
      projection, which needs the section below: do them together if the bench
      says the cache can't pay for itself alone.

## The desk's foundations: the GPU projects

- [x] Desk: positions to the GPU in millimetres, `Eye` building one
      view-projection matrix that the shader's uniform and the CPU's click
      rects and flat parts both read: the paper's texture and light shade
      perspective-correct (hence today's fine sheet mesh and the 1.27 mm
      bends), a moving view is just a new matrix.
- [ ] Desk: text where the sheet comes nearer the eye than the typing line,
      toward its top, is laid out larger to stay as sharp.
- [ ] Desk: real shadows from the lamp: what the desk hides by hand goes —
      `body::paint_deck`, `case::paint_well`, the inner walls' `[top, foot]`,
      the cover's foot and its opening shade, the keys' shadows,
      `panel::cast_on_panel` — and a depth pass from the lamp, or a shadow
      map, casts them instead.
- [ ] ❓ Desk: whether the depth pass becomes a conventional 3D renderer, its
      own branch, once the measurements above say so: judged against what is
      lost — the headless snapshot's CPU twin, egui's free text and windowing.

## The desk's machine

Its parts drawn on the foundations; items needing the click contract say so.

- [x] Desk: the rest of the machine in real 3D, on the depth pass the body, carriage and
      sheets now draw on (one wgpu paint callback a frame): the keyboard and its case and the
      side controls, still flat over it. Then the flat sheet's ways leave the desk (its paper
      table's placing, the curl and lift)
- [ ] Desk: the margin rack behind the paper support, its two stops set there rather than
      on the bail's scale. The sheet hides it: drag the sheet's top down to fold it back, let go,
      then set the stops. Dragging it up again, or the next key typed, unfolds it.
      Wants the click contract.
- [ ] ❓ Desk: its own levers and keys in place of plates and chords: the line-space lever, tab
      set and clear keys. Wants the click contract.
- [ ] ❓ Desk: the carriage lock and the touch control beside the keyboard work. The ribbon
      selector beside them waits for the two-colour ribbon. Wants the click contract.

## The desk's calm mode

- [ ] ❓ Calm mode redesigned for normal mode. Calm now fades only the app's chrome
      (the scale's marks, the desk icons); the machine stays whole, its knobs and panel controls
      among it, as a depth pass can't fade. Decide what calm means at a desk, and how it shows

## Small and important

- [ ] ❓ A round of fixes for what trips up the first testers, on the release builds. Choices
      held back for their feedback (key rebinding among them) are settled afterwards.
- [ ] ❓ Export as .odt, the other half of `typewriter --import`, beside the text and Markdown
      exports
- [ ] Elite (12 cpi) type option for the SM9
- [ ] ❓ Two-colour ribbon (black / red) switch: colour in the folder format. On the desk, the
      ribbon selector beside the keyboard sets it (blue for black, white for stencil, red)

## Nothing but the desk

The desk's icons, plates and menus become the things they stand for, easiest first,
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

- [ ] ❓ Drag a finished sheet onto the machine to roll it back in, or onto the stand.
      Wants the click contract.
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
      reads (knob colours, the scale), the SM9's by default; and a `model` naming normal mode's
      drawing of it. Open: what it draws for a profile without one
- [ ] ❓ Additional profiles (e.g. Olivetti Lettera 32, Hermes 3000, IBM Selectric), each with
      its own pitch, typeface, bell offset and sounds

## The desk as a scene

The camera work that lets a scene stand in the foundations above; what the
scene is stays here.

- [ ] ❓ Desk: a scene rather than fixed screen positions: the copy holder stands left of the
      document at a fixed angle, and the view (position, field of view) can be moved and zoomed,
      more like a game camera than a page on screen, on the desk's 3D drawing
- [ ] ❓ Desk: drawers holding the projects: open one by pulling its folder out
- [ ] ❓ Desk: a wastepaper basket for scrunched sheets
- [ ] ❓ Desk: a shelf of machines: change profile by lifting another typewriter onto the desk

## Someday: a walkable office

- [ ] ❓ Desk: an immersive desk you can get up from: walk over to a photocopier for copies of
      finished sheets (instead of carbon copies at the machine)
