# 3D fixes

Clean-up after the desk edition moved onto the depth pass (its `depth.rs`): what is slow,
what is still shaded or ordered by hand, and what the depth buffer or the GPU could do instead.
Worked through gradually, across sessions, on the `desk-viewpoint` branch.

Work top to bottom: performance first, then consistency, then the larger moves. Each item should leave the desk working and pass fmt, clippy and tests. Check drawing
changes with the snapshots (see `AGENTS.md`) before calling them done. Finished items leave
this file, as in `ROADMAP.md`; delete the file once it is empty.

The roadmap's first desk item ("the rest of the machine in real 3D") overlaps with section 3.
This file has the details; the roadmap keeps the order.

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
solids and their vertices on the desk's side, in `depth::paint`. Call it from an ignored test in the desk's `stage.rs`
with `Desk` and `Plain`, in debug and `--release`. For a single part, time it directly on a
`Canvas::depth` inside `ctx.run_ui`. Revert the probes afterwards.

---

## 1. Performance

- [ ] ❓ **Optimise the desk crate in dev builds.** What remains of the debug frame is mostly
      the desk's own code run unoptimised (the caps, the levers, the sheets' placing). With
      `[profile.dev.package.typewriter-desk] opt-level = 1` the debug frame went from ~31 to
      ~14 ms (empty page) and ~47 to ~24 ms (full page). Adding `typewriter-app` saved ~1 ms
      more, and every dependency at `opt-level = 2` only ~3 ms, so the desk crate alone is
      the lever. The cost: stepping through the desk in a debugger is less exact. The
      root `Cargo.toml` already does this for image decoding. Needs the maintainer's yes.

## 2. Consistency: one light, one pass

- [ ] **Two lights.** The carriage's `cylinder` and `roller` (`carriage.rs`) light from
      `LIGHT_DEGREES`, 55° from the top: above and in front, with no left. Everything else
      uses `light::toward_light()` (left, above, a little in front). Shade the platen, the rod
      and the bail's rollers from each band's normal with `toward_light()`, as `paint_rod`
      in `keyboard.rs` already does, and drop `LIGHT_DEGREES`.
- [ ] **Paper's shading written twice.** `PAPER_AMBIENT`, `PAPER_LIT` and the formula are in
      both `sheet.rs` (`paper_shade`) and `support.rs` (`paper_table`). Make `paper_table`
      call `sheet::paper_shade`.
- [ ] **Knob shadows placed by hand.** `panel.rs` places them with `KNOB_SHADOW_TURN` and
      `SHADOW_OFF`. Derive them from `toward_light()` projected onto the panel, as
      `keyboard::paint_key_shadow` does for the keys.
- [ ] **Three depth callbacks a frame.** `paint_behind`, the sheets and `paint_front` each
      call `depth::paint`. They share the depth buffer, but a decal is only sure to follow
      the opaque faces behind it within its own callback; a translucent decal (the guide's
      glass) drawn before an opaque face behind it would be painted over. Gather one
      `Solids` per frame (e.g. kept on the `Scene` or the typing view) and paint it once.
      With one callback, its GPU buffers can also be kept across frames and refilled with
      `queue.write_buffer`, not made anew in every `prepare`.

## 3. Still flat or in painter's order

The roadmap's "rest of the machine in real 3D". What each part needs:

- [ ] **Parts that skip the canvas.** These build screen-space meshes with
      `geometry::add_quad` and `canvas.painter().add(..)`. Switching them to `Canvas::depth`
      alone would leave them flat; rewrite each with `eye.quad` into a `Solid` and
      `canvas.mesh`:
      - `case.rs`: `paint_well_shade`, `paint_inner_walls`, `paint_wall_tops`,
        `paint_case_front`.
      - `keyboard.rs`: `paint_rod`.
- [ ] **Key caps as real faces.** `keyboard::paint_cap` fills a screen-space `hull` of the
      projected top and foot, shaded by screen position (`down`, `across`), with outlines
      straight on the painter. Build the top, front, sides and back as faces between the two
      rounded outlines, lit with `matte` by each face's normal. The keyboard block in
      `machine::paint_front` then moves onto the depth canvas and loses its "deepest first"
      order.
- [ ] **The side controls and wall marks** (`side_controls.rs`): onto the depth canvas once
      the caps and steel are. Their outlines go straight to the painter.
- [ ] **The knobs drawn by the desk, in depth.** Then `paint_cover_over_knobs` (the cover
      repainted clipped to each knob's rect) goes, along with the `paint_over_knobs` hook's
      use of it.
- [ ] **The return lever in one pass.** `lever.rs` splits the strap at `behind_knob` into
      `paint_lever_base` and `paint_lever`, clips the base at the side plate, and orders each
      length's strips "curl first, flat top last". In depth: one call, no clip, no order.
      `paint_strap`'s edge strokes go straight to the painter; lay them on the canvas.
- [ ] **The paper bail in 3D.** `carriage::paint_bail` draws on screen at the sheet's scale,
      with the 2D `roller`. Model it with `cylinder` and real arms. The app still prints its
      scale at `bail_scale_top`, flat, so keep the bar's front where the scale lands or move
      the scale onto it. ❓ Ask the maintainer which.
- [ ] **The flat sheet's ways leave the app** once nothing uses them: they are the desk's
      only, so the rule in `AGENTS.md` (*The two editions*) wants them gone from
      `typewriter-app`. `PaperTable` with its `place` and `PLATEN_SHADE` darkening,
      `paint_on_table`, and the `sheet_curl` and `sheet_lift` hooks. The new sheet's way in
      still reads `wrap_inches` and `seen_inches` for its timing: hand those over as plain
      numbers. `paint_in_machine` stays: the plain app's feed uses it.

## 4. Hand-made shading and offsets the GPU could do

- [ ] **Light in the shader.** Gradients painted by hand stand in for lighting:
      - `body::paint_deck`: front to back.
      - `cover::paint`: `IVORY_SHADE` → `IVORY_LIT` down the cover.
      - `panel::paint_face`: `IVORY_LIT` → `IVORY` down the panel.
      - `case::paint_well`: `KEY_BED` → `KEY_BED_FRONT`.
      - `case::paint_inner_walls`: the `[top, foot]` brightness factors.
      - `cover::paint_plate_wall`: `brighten(colour, 0.7)` at the foot.
      - the caps' dished top.

      Give depth vertices a normal (and a material: matte, chrome, paper), pass the light as a
      uniform, and do `matte`, `streak` and the paper's shade in `depth.wgsl`. This changes the
      vertex layout (`VERTEX_BYTES`), `Solid`, and the snapshot rasterizer. Decide per
      gradient whether it is lighting (goes) or the material's own look (stays). The cover's
      shadow in its opening and the well's shade are fake occlusion and stay until there are
      real shadows.
- [ ] **Depth bias instead of offsets.** Things that lie on a face are set nearer by hand:
      `eye::LYING_INCHES` with `lying_depth`, `carriage::UNDER_PAPER_INCHES`,
      `printing_point::GUIDE_OFF_PAPER`, and the `+ 0.002` on the wall marks in
      `side_controls::paint_marks`. Try a slope-scaled `DepthBiasState` on the decal pipeline.
      Depth is `1 - near / distance`, not linear, so a fixed inch offset is a varying bias:
      check the platen's edges against the paper at 25 % and 200 %. The snapshot rasterizer
      needs the same bias.
- [ ] **Backface checks the depth buffer makes redundant.** These only skip hidden faces:
      - `case.rs`: `Eye::sees` for the inner walls and the case's front.
      - `carriage::paint_side_plate`: `eye.faces`, and its outlines.
      - `lever.rs`: `eye.faces` for the bracket's inner face and the screw's rim.

      Once those parts are in depth, drop the checks, or keep them only as culling, said so
      in a comment. The lever's `seen` normal picks which side is lit and stays, as does
      `Eye::sees` in `sheet::paint_sheets` (which side the print is on).

## 5. Larger: the GPU projects

- [ ] **Positions in inches, projected on the GPU.** Today `Eye::at` projects every vertex
      on the CPU and the shader gets screen positions. Interpolation in screen space is
      affine, so the paper's texture and colours are not perspective-correct (hence the fine
      sheet mesh). Send inch positions and the eye as a uniform (a view-projection matrix
      with `w`), and the GPU interpolates correctly and does the divide. Needs everything
      drawn in depth first (sections 3 and 4), since flat parts line up with `Eye::at` on the
      CPU. The click rects (`Panel::rect`, knobs) still need `Eye::at` on the CPU. Large: do
      it last, and only if the earlier items leave a reason.
