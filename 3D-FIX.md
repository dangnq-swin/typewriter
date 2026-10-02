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

## 2. Larger: the GPU projects

- [ ] **Positions in millimetres, projected on the GPU.** Today `Eye::at` projects every
      vertex on the CPU and the shader gets screen positions. Interpolation in screen space
      is affine, so the paper's texture and colours are not perspective-correct (hence the
      fine sheet mesh). Send millimetre positions and the eye as a uniform (a
      view-projection matrix
      with `w`), and the GPU interpolates correctly and does the divide. Needs section 1
      first; the machine's shadow on the desk and the panel's controls (`Panel::paint`, after
      the depth pass), still flat, line up with `Eye::at` on the CPU. The click rects (`Panel::rect`, knobs) still need `Eye::at` on the CPU. Large: do
      it last, and only if the earlier items leave a reason — the ROADMAP's free camera and
      moving components are that reason.
      - Give the shader one camera interface: a view-projection matrix uniform that CPU
        `Eye::at` builds the same matrix from — click rects and flat parts can't then drift
        from the GPU's, the eye direction the lighting takes comes from it too, and a moving
        camera is just a new matrix.
      - Say where each solid's transform, from its millimetres to the machine's, lives: a
        moving part will want its own, not one baked into its vertices.
