# 3D fixes

Clean-up after the desk edition moved onto the depth pass (its `depth.rs`): what is slow,
what is still shaded or ordered by hand, and what the depth buffer or the GPU could do instead.
Worked through gradually, across sessions, on the `desk-viewpoint` branch.

Work top to bottom: the GPU's lighting first, then the larger moves. Each item should leave
the desk working and pass fmt, clippy and tests. Check drawing changes with the snapshots (see
`AGENTS.md`) before calling them done. A finished item is checked (`- [x]`), not deleted, while
its section still has open items; once a whole section is done it leaves the file. Delete the
file once it is empty.

---

## Baseline

Measured headless on 2026-10-01: the typing view's frame (`ctx.run_ui` plus tessellation), a
1600 × 1000 window, the desk edition against the plain app.

| Frame | debug | release |
|---|---|---|
| Plain, empty page | 0.3 ms | 0.02 ms |
| Desk, empty page | ~140 ms | ~20 ms |
| Desk, full page | ~155 ms | ~21 ms |
| Desk, empty page, strokes cut and the guide on a grid | ~37 ms | ~3.7 ms |
| Desk, full page, strokes cut and the guide on a grid | ~50 ms | ~5.1 ms |
| Desk, empty page, then 6 draws, `face_y` solved | ~31 ms | ~2.7 ms |
| Desk, full page, then 6 draws, `face_y` solved | ~47 ms | ~4.1 ms |

Where the desk's debug frame went, before the last two rows:

- `paint_on_way` (the sheet in depth, now the desk's `machine::paint_sheets`): ~80 ms, on an
  empty page.
- `paint_over_sheets`: ~55 ms, 45 of it the alignment guide's plates.
- Everything else under 1 ms per hook. Within the machine, the caps take ~10 ms in debug, the
  key levers ~3.5, the scale ~2, the cover's opening ~2 and the case's frame ~2.
- Packing vertices for the GPU (`Gpu::prepare`): 7–14 ms in debug, 0.2 ms in release.
- 317 solids a frame (one draw call each), 35–65 k vertices in depth; 12–26 k once strokes
  were cut. The margin frame had made 44,652 vertices out of 48, and each guide plate 7,328
  out of 28.

**To measure again:** add a temporary function to `app/snapshot.rs` that builds the app as
`render` does, and have it time N frames of `ctx.run_ui` and `ctx.tessellate`. Count the
solids and their vertices on the desk's side, in `depth::end`. Call it from an ignored test in
the desk's `stage.rs` with `Desk` and `Plain`, in debug and `--release`. For a single part, time it directly on a
`Canvas::depth` inside `ctx.run_ui`. Revert the probes afterwards.

---

## 1. Hand-made shading and offsets the GPU could do

- [ ] **Light in the shader.**
      - [x] **Matte in the shader.** Depth vertices carry a `Shade` (a material and a
            normal), the light is a uniform, and `depth.wgsl` lights matte plastic per pixel;
            `Shade::apply` is its Rust twin, for the snapshot rasterizer and flat canvases.
      - [x] **Polished, streak and chrome.** `Shade::Polished` carries its shine colour and
            sharpness, `Shade::Streak` a tangent rather than a normal, and `Shade::Chrome`
            reads the room's bands from the lighting uniform, so a plate is one quad: the
            platen, the knobs, the bail's rim, the rod and the segment's rim are lit per
            pixel now. Lines (`eye.line`) take no shade and stay lit on the CPU
            (`Paint::lit`), as does each strip of the return lever — one colour to a frame
            pair, by which side the eye sees it — and the screw's face. Measured again on the
            typing view: the CPU's frame is as it was (~46 ms debug, ~4.6 ms release, whole
            desk), and a vertex is 48 bytes where it was 40 for about the same count (~51 k a
            frame, 72 fewer since the plates stopped banding). The light moved to the shader;
            the time did not.
      - [x] **A lamp, and the gradients it replaces.** The light is a point now:
            the lamp stands 254 mm to the writer's left, 170 in front of the
            printing point and 400 above the desk — its head over the front of the
            keys, where a real desk lamp would be put: lighting the sheet across,
            crowding nothing. Each depth vertex carries the millimetres it stands
            at (a 60-byte vertex, until section 2 projects on the GPU), the shader
            lights a fragment from its millimetres toward the lamp, and the CPU
            twins — `Shade::apply`, the flat canvas, the snapshot rasterizer — blend
            the same millimetres. The gradients that were lighting are gone: the cover and
            the panel are one matte slope each, the bail's front is one quad with
            a normal (its three stops and `BAR_MID` went with them), the caps'
            dished tops take normals that turn from back to front, and the paper
            and its print are a matte material — `sheet::paper_shade` is gone, and
            correction chalk still matches the paper, being lit alike. The lamp
            burns the room's own white, by the maintainer's eye: position and
            falloff register it, a tint would only yellow the page. Retuned
            against the snapshots; the plain app's pixels are unchanged. Fake
            occlusion stays until there are real shadows: `body::paint_deck` (the
            carriage's shade), `case::paint_well`, `case::paint_inner_walls`'
            `[top, foot]`, `cover::paint_plate_wall`'s foot, the cover's shadow in
            its opening and the keys' shadows.
- [ ] **One kind of bar: the key levers and the type bars.** Both draw an elongated steel
      bar the same way — a dark stroke with a bright hair along its lit edge — and each does
      it its own way. `keyboard::paint_steel` (the key levers, the stems the key tops sit
      on, and the lock's and the selectors' posts through it): 1.27 of `STEM` with 0.36 of
      `STEM_SHINE.gamma_multiply(shine)` 0.46 mm to the writer's left,
      `shine = streak(along, 10).max(0.25)`. `cover::paint_type_basket` (the type bars, the
      arms that strike): 2 of `TYPE_BAR` with 0.5 of
      `METAL_SHINE.gamma_multiply(0.12 + 0.88 * streak(along, 12))` 0.6 mm aside
      across the fan. One helper for both, one width, one aside, one floor under the
      streak. Then light them like the rest of the machine: a stroke carrying a
      `Shade::Streak` needs `Solids::add` to take a shade per vertex, and the streak must
      lerp a feathered decal's straight rgb toward its shine, not its premultiplied colour,
      or the halo shines. A point light makes a streak baked once per bar wrong anyway,
      since it changes along the bar: do this with the lamp.
- [ ] **Depth bias instead of offsets.** Things that lie on a face are set nearer by hand:
      `eye::LYING_MM` with `lying_depth`, `carriage::UNDER_PAPER_MM`,
      `printing_point::GUIDE_OFF_PAPER`, and the `+ 0.05` on the wall marks in
      `side_controls::paint_marks`. The guide's offset has a narrow window: in front of the
      print, its glass behind the ribbon (its test says so). Try a slope-scaled
      `DepthBiasState` on the decal pipeline. Depth is `1 - near / distance`, not linear, so a
      fixed millimetre offset is a varying bias: check the platen's edges against the paper at
      25 % and 200 %. The snapshot rasterizer needs the same bias.
- [ ] **Backface checks the depth buffer makes redundant.** These only skip hidden faces:
      - `case.rs`: `Eye::sees` for the inner walls and the case's front.
      - `carriage::paint_side_plate`: `eye.faces`, and its outlines.
      - `lever.rs`: `eye.faces` for the bracket's inner face and the screw's rim.

      All are in depth now: drop the checks, or keep them only as culling, said so in a
      comment. The lever's `seen` normal picks which side is lit and stays, as does
      `Eye::sees` in `sheet::paint_sheets` (which side the print is on).

## 2. Larger: the GPU projects

- [ ] **Positions in millimetres, projected on the GPU.** Today `Eye::at` projects every
      vertex on the CPU and the shader gets screen positions. Interpolation in screen space
      is affine, so the paper's texture and colours are not perspective-correct (hence the
      fine sheet mesh). Send millimetre positions and the eye as a uniform (a
      view-projection matrix
      with `w`), and the GPU interpolates correctly and does the divide. Needs section 1
      first; the machine's shadow on the desk and the panel's controls (`Panel::paint`, after
      the depth pass), still flat, line up with `Eye::at` on the CPU. The click rects (`Panel::rect`, knobs) still need `Eye::at` on the CPU. Large: do
      it last, and only if the earlier items leave a reason.
