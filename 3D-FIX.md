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

Measured headless on 2026-10-02 by the bench — `TYPEWRITER_BENCH=<frames> cargo test -p
typewriter-desk -- --ignored bench`, without `--release` for the debug column: the typing
view's frame (`ctx.run_ui` plus tessellation), a 1600 × 1000 window, the desk edition
against the plain app.

| Frame | debug | release |
|---|---|---|
| Plain, empty page | 0.6 ms | 0.04 ms |
| Plain, full page | 2.8 ms | 0.2 ms |
| Desk, empty page | 74 ms | 4.6 ms |
| Desk, full page | 87 ms | 5.5 ms |

The desk's pass holds 3 solids a frame since the GPU projected — the joins by texture
swallow what were 317 — over 49 k vertices on an empty page, 57 k on a full one.

Where the desk's debug frame goes: the caps ~21 ms, `depth::end`'s debug walk checking
each vertex stands where it is drawn ~18, the printing point ~14, the rest of
`paint_front` ~10, the sheets 5–11, everything else under 4. In release, `paint_front`
is ~4 of the frame and the caps ~1.6.

Before the GPU projected (2026-10-01): Desk ~140/~20 debug/release on an empty page,
~155/~21 on a full one, 35–65 k vertices, `Gpu::prepare` 7–14 ms debug. Part-level
timings are probes written against `snapshot::time_frames`; revert them afterwards.

---

## 3. Follow-up: the fine meshes were a cure for affine blends

- [ ] The GPU projects since 2026-10 (old section 2): uv, colour and millimetres now
      blend perspective-correctly, so the sheet's 1.27 mm grid, `Solids`' screen-space
      `subdivided` guard and the guide's `fill_bent` cutting are no longer needed for
      correctness. Coarsen them where the bench (see *To measure again*) shows a win.
      Window `z` still blends affine — a rasterizer divides every attribute but depth —
      so a face must stay planar, or be cut, to order against its own coplanar decals.
