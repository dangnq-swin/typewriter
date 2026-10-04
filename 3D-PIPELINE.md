# The 3D pipeline

The branch that answers ROADMAP's foundations question: the depth pass becomes
a dedicated 3D pipeline — its own renderer, not an egui paint callback —
because the desk keeps growing 3D work (real shadows done, scene camera, more
machines, the walkable office someday). Measured on 2026-10-05, on the host's
Radeon 780M: the current pass costs ~2.2 ms a frame CPU-side, flat across
window sizes (3 solids, ~55k vertices kept and re-projected), and ~1.4 ms
GPU-side wall at the desk's size — inside a 60 fps budget on hardware and on
software alike. The migration is chosen for where the desk is going, not for
today's numbers.

Each task leaves the app building, tested and snapshot-checked
(`AGENTS.md`): fmt, clippy `-D warnings`, tests, drawings looked at. Perf
tasks are judged on the bench. `typewriter-core` stays free of rendering;
profile data stays TOML.

## Hardware note

`/dev/dri` is granted to the sandbox profile, and the bench answers both
backends of the host's Radeon 780M — RADV Vulkan and radeonsi GL. The depth
pass today, per frame (lamp and main min/median from timestamp queries,
50 frames; wall is the pipelined batch):

| scene | backend | lamp ms | main ms | wall ms |
|---|---|---|---|---|
| desk 1600×1000 | vulkan | 0.075/0.116 | 0.805/1.235 | 1.387 |
| desk 1600×1000 | gl | 0.076/0.112 | 0.801/1.155 | 1.311 |
| full 1920×1080 | vulkan | 0.072/0.110 | 0.849/1.287 | 1.523 |
| full 1920×1080 | gl | 0.066/0.086 | 0.768/1.016 | 1.163 |
| wide 3000×1400 | vulkan | 0.045/0.061 | 1.531/2.207 | 2.842 |
| wide 3000×1400 | gl | 0.041/0.049 | 1.383/1.713 | 1.872 |
| far-back 1536×960@25 | vulkan | 0.090/0.105 | 0.367/0.427 | 0.669 |
| far-back 1536×960@25 | gl | 0.118/0.120 | 0.455/0.477 | 0.736 |

CI keeps its display-free witness for tests; hardware numbers come from
maintainer runs like these.

## How the work divides

One agent at a time for the first two tasks; then four in parallel against
the interface task 1 leaves; then testing and bench together; retirement last.

```
T0 ── T1 ──┬─ T2 (geometry)   ─┬─ T6 (tests) ∥ T7 (bench) ── T8 (retire)
           ├─ T3 (light)      ─┤
           ├─ T4 (sheets)     ─┤
           └─ T5 (egui, pick) ─┘
```

Agents own whole files; two agents never edit one file in a wave. Where a
task needs a neighbour's file, it waits for the next wave.

## Commits

One commit a task, so the branch reads as the plan does and no wave collapses
into one big commit. The session lead — not the agents — commits: agents work
in parallel in one tree and never commit; between waves the lead runs fmt,
clippy, tests and snapshots branch-wide, then lands each task's files as its
own commit. A task may split in two where the diff wants it, but no task
merges into its neighbour's commit. A wave's done-when is judged after the
wave's integration, before its commits land.

| # | task | commit |
|---|---|---|
| 0 | this plan | `docs: the 3d pipeline's plan` |
| 1 | T0 | `feat: the 3d pipeline's shape, decided and sketched` |
| 2 | T1 | `feat: the 3d pipeline's spine` |
| 3 | T2 | `feat: the machine's parts build into the 3d pipeline` |
| 4 | T3 | `feat: the lamp lights the 3d pipeline` |
| 5 | T4 | `feat: sheets and print draw in the 3d pipeline` |
| 6 | T5 | `feat: egui and the click contract over the 3d pipeline` |
| 7 | T6 | `test: snapshots headless through the 3d pipeline` |
| 8 | T7 | `test: the bench times the 3d pipeline` |
| 9 | T8 | `refactor: the old depth pass retires` |

## T0 — architecture spike, one agent, first

Decide how the renderer composes with the window, and write the decision as
an ADR section at the foot of this file. The candidates:

- **eframe keeps the window and egui; the renderer owns the 3D frame.** One
  full-window wgpu pass the app drives through eframe's render state, egui
  paints over it. Least windowing risk; the renderer is still "its own".
- **Own winit + wgpu renderer, egui-wgpu as a UI layer.** The conventional
  shape; eframe's conveniences (paint callbacks, tessellation, the depth
  buffer config) go, egui's text and widgets stay through egui-wgpu.

Judged by: how the snapshot and bench harnesses reach the renderer headless,
how the click contract survives, what plain mode keeps (plain mode draws no
3D and must not pay for it), and how much of `depth/`'s maths either keeps.
The spike ends with the chosen shape sketched in code — a skeleton that
compiles and draws a clear-colour frame through the new path, both modes
still working the old way beside it.

## T1 — renderer core, one agent, after T0

The pipeline's spine, at whatever module or crate shape T0 chose (candidate:
a `render3d` module of `typewriter`, or a new `typewriter-render` crate):

- Device/surface/swapchain or eframe render-state integration per T0's ADR.
- Pipelines: opaque, decal, shadow-map (2048², `SHADOW_MAP` stays the
  contract), text-on-surface placeholder.
- The frame: depth buffer, the camera uniform from `Camera`'s clip rows
  (`depth::Camera` keeps its maths; `Eye::camera` keeps feeding it — a test
  pins CPU and GPU to the same projection, as today).
- **The interface the parallel tasks build against** — deliver it compiling
  and unit-tested before the wave opens:
  - scene submission: opaque then decal layers, meshes named and replaceable
    per frame (today's `Layer`/`Solid`/`Placing`, minus the egui vertex
    types),
  - per-solid placement in absolute millimetres, standing solids uploaded
    once and moved by transform only,
  - the shadow pass hook lighting's casters register into,
  - a test-only capture path a headless device can render through.
- GPU vertex layout in millimetres (the shader projects; the CPU stops
  re-projecting per frame — that is where today's ~1.8 ms goes).

## Wave 2 — four agents in parallel, after T1

**T2 — geometry, the machine's parts.** `machine/` solid builders
(body, case, cover, carriage, keyboard, side controls, panel, knobs, levers,
bail, support, `standing.rs`, `geometry.rs`) build into the renderer's mesh
types. Standing solids stay cached; the carriage's `shifted` placement
becomes a transform. Keys instance where they repeat. Done-when: the parts build into the
renderer's meshes and the test capture shows them placed; the desk draws
whole once the wave lands.

**T3 — light and shadow.** `depth/lighting.rs`, `depth.wgsl`,
`machine/light.rs` into the renderer's pipelines: the area-head lamp (the
disc facing the printing point, penumbra widening with depth — the design
rule in `AGENTS.md`), the shadow-map pass from the lamp, shade bands. The
CPU twin of the lighting maths stays in step, test-pinned, until T6 decides
its fate. Done-when: lamp shadows match today's snapshots.

**T4 — sheets, print, decals.** `machine/sheet.rs`, `canvas.rs`,
`printing_point.rs`, the print cache: the sheet's mesh (curl, lift, the
1.27 mm bends), the paper texture perspective-correct, print above the
typing line laid out magnified near the eye, decals (print, edges, glass)
drawn after opaques with the depth bias as a pipeline constant. Done-when:
the feeding and typed snapshots match.

**T5 — egui composition, camera, picking.** `stage.rs` hooks,
`typewriter-ui`'s render modules, `machine/eye.rs`: egui chrome over or under
the 3D pass per T0's ADR, the flat desk of folders and notes unchanged in
folder view, and the click contract — parts sensed by `render::CLICK`, click
rects computed from the same camera the shader reads (CPU picking; GPU
picking only if the bench says the rects are wrong at depth). Done-when:
clicks land where the eye sees, keyboard typing works end to end.

## Wave 3 — two agents in parallel, after wave 2

**T6 — the snapshot strategy.** `depth/raster.rs` (the CPU twin) and
`app/snapshot.rs`: decide what the headless witness becomes — render through
the test-only capture on a real (or llvmpipe) device, or keep a CPU raster in
step with the new shaders — and land it. Stage.rs's scene list survives;
`gpu_snapshot` grows or shrinks accordingly. CI (`checks.yml`) keeps running
without a display. Done-when: `TYPEWRITER_SNAPSHOT=<folder>` produces the
scenes from a path CI can afford.

**T7 — the bench.** Port `stage::tests::bench` and `gpu_bench` to the new
pipeline: the same tables (ms, median, run_ui, tessellate, solids, vertices;
lamp, main, span, cpu, wall), new-pipeline rows beside old-path rows while
both live. Re-take hardware numbers when `/dev/dri` lands. Done-when: the
branch's perf claim cites the bench.

## T8 — retirement, one agent, last

Delete the old paint-callback path (`depth/mod.rs`'s begin/gather/end, the
egui-wgpu callback in `depth/gpu.rs`), whatever T6 retired of `raster.rs`,
dead bench rows; `stage.rs` hooks shrink to the new contract. ROADMAP's
foundations section leaves the file (git history and the release notes keep
it); README's Controls if anything moved. Done-when: fmt, clippy, tests,
snapshots pass with no old path in the tree, and the workspace builds both
modes.

## Decisions log (ADR)

*(T0 writes the composition decision here; later waves append theirs.)*