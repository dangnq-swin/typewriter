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

T0 is done. T1a opens the crate and plain mode's window alone; then T1b
(normal mode's window) and T1c (the renderer's device core) run in parallel
on disjoint files; then the machine's parts, the lamp and the sheets build
against T1c's interface in parallel; T5 switches the desk over; testing and
bench together; retirement last.

```
T0 ── T1a ──┬─ T1b (normal mode's window) ─┬─ T2 ∥ T3 ∥ T4 ── T5 ── T6 ∥ T7 ── T8
            └─ T1c (renderer device core) ─┘
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
wave's integration, before its commits land — T1b ∥ T1c land as two commits
after their joint integration.

| # | task | commit |
|---|---|---|
| 0 | this plan | `docs: the 3d pipeline's plan` — landed |
| 1 | this restructure | `docs: the pipeline takes its own surface` — landed |
| 2 | T1a | `feat: the render crate opens plain mode's window` — landed |
| 3 | T1b | `feat: normal mode moves onto the render crate's window` |
| 4 | T1c | `feat: the render crate's device core` |
| 5 | T2 | `feat: the machine's parts build into the render crate` |
| 6 | T3 | `feat: the lamp lights the render crate` |
| 7 | T4 | `feat: sheets and print draw in the render crate` |
| 8 | T5 | `feat: the desk draws through the render crate` |
| 9 | T6 | `test: snapshots headless through the render crate` |
| 10 | T7 | `test: the bench times the render crate` |
| 11 | T8 | `refactor: eframe and the old depth pass retire` |

## T0 — done, decided

The spike put the composition question to two candidates and recommended
composing through eframe (candidate 1), sketching it as a `render3d` module
beside `depth/` with an eframe-callback install and a bare-device
clear-colour test. The maintainer overruled it and decided candidate 2 —
ADR 1 below records the decision and what carries forward. The skeleton
sketched the overruled shape and is dropped from the tree. What the spike
established holds regardless: egui-wgpu 0.36's internals as verified (its
egui mesh pipeline neither tests nor writes depth; a callback's `paint`
runs inside its one render pass; `prepare` command buffers submit before it
and are wiped by its clear) — under our own loop these become facts about a
component we drive, and the pass order is ours: egui-under meshes, the 3D
pass, egui-over meshes. Headless reach proved symmetric — both shapes reach
a device the same way, `depth/gpu.rs`'s rig already running on a bare
headless device, offscreen target and readback — while the snapshot harness
reaches the pass with no device and no window at all
(`TypewriterApp::nowhere` skips `Stage::start`; its door is
`Stage::snapshot_callback` and the CPU twin). `depth/`'s maths was never in
question: composition touches none of it.

## T1a — the crate and plain mode's window, one agent, first

`crates/typewriter-render` is born, its module layout stubbed so the later
tasks own disjoint files: `window/` (the winit loop, the surface), `ui/`
(egui driven through egui-wgpu), and the renderer core's home (T1c's). The
loop gets what a window owes both modes: open and close, resize, DPI
(points-per-point into egui's input), fullscreen both ways — plain mode's
settings card switches it, and `app/mod.rs` follows the window's state —
and egui's repaint signal hooked into the loop: the one-instance hand-over
(`instance.rs`) repaints from a listener thread and must wake it. Input
maps through egui-winit; plain mode types through it. Done-when: **plain
mode works end to end on the new window** — its whole UI is egui, no 3D at
all, and `typewriter-plain` opens through it — while normal mode still runs
on eframe, the old path building beside it.

Safety rail for the migration, built here and proved at T1b: we drive
egui-wgpu's renderer ourselves, so the **old depth pass's paint callback
keeps working** through it — the desk never disappears.

## T1b — normal mode's window, one agent, after T1a

The desk's chrome and flat parts on the new loop through egui-wgpu — the
room's backdrop, the panel's controls, the scale, the flat sheet wrappers —
while the machine area stays drawn by the **old depth pass** through the
callback we now drive: that is the incremental migration path. `Stage::start`'s
install takes a `RenderState` we build from the loop's own device, queue
and renderer (every field is public — nothing inside egui-wgpu blocks it),
and the surface carries the depth attachment the pass depth-tests into. The
viewport commands the app leans on port: fullscreen bookkeeping (the
Windows inner-size workaround included), the Wayland focus retry after a
hand-over, close and cancel-close. Done-when: normal mode's desk draws as
today — snapshots match — and the smoke test is green on xvfb for both
binaries.

## T1c — the renderer's device core, one agent, parallel with T1b

Disjoint files: T1b owns app wiring in `typewriter`/`typewriter-ui`, T1c
owns the crate's renderer core. The pipelines registry (opaque, decal,
shadow-map — 2048², `SHADOW_MAP` stays the contract — and a
text-on-surface placeholder), the camera uniform from `Camera`'s clip rows
(`depth::Camera` keeps its maths; `Eye::camera` keeps feeding it — a test
pins CPU and GPU to the same projection, as today), our own depth buffer,
the frame's pass order, and the **bare-device tests** beside it all. And
**the interface the parallel tasks build against** — delivered compiling
and unit-tested before wave 2 opens:

- scene submission: opaque then decal layers, meshes named and replaceable
  per frame (today's `Layer`/`Solid`/`Placing`, minus the egui vertex
  types),
- per-solid placement in absolute millimetres, standing solids uploaded
  once and moved by transform only,
- the shadow pass hook lighting's casters register into,
- a test-only capture path a headless device can render through — the
  spike's readback test is its seed, now living in the new crate.

GPU vertex layout in millimetres (the shader projects; the CPU stops
re-projecting per frame — that is where today's ~1.8 ms goes).

## Wave 2 — three agents in parallel, after T1c's interface

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

## T5 — the switch and the input contract, one agent, after wave 2

`stage.rs` hooks, `typewriter-ui`'s render modules, `machine/eye.rs`: the
desk moves off the old depth pass onto the renderer's submission — the
hooks gather into it, the machine's meshes draw through it, and the old
callback draws its last frame (T8 deletes it). The click contract verified:
parts sensed by `render::CLICK`, click rects computed from the same
`Camera` the shader reads (CPU picking; GPU picking only if the bench says
the rects are wrong at depth). The flat desk of folders and notes is
unchanged in folder view. And the clear becomes the room's colour in normal
mode — the maintainer's call, landed here. Done-when: clicks land where the
eye sees, keyboard typing works end to end, the snapshots match.

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
dead bench rows; `stage.rs` hooks shrink to the new contract. And eframe
leaves the workspace: the dependency goes from `typewriter-ui` and both
binaries (whose egui imports come straight from `egui` — egui itself stays,
egui-wgpu needs it), README's build notes if they mention it;
`scripts/smoke-test.sh` stays, proving the new window on xvfb. ROADMAP's
foundations section leaves the file (git history and the release notes keep
it); README's Controls if anything moved. Done-when: fmt, clippy, tests,
snapshots pass with no old path and no eframe in the tree, and the
workspace builds both modes.

## Decisions log (ADR)

### ADR 1 — decided: the renderer takes its own surface — winit + wgpu, egui-wgpu as the UI layer

Decided 2026-10-05, the maintainer's call, overruling T0's recommendation
of composing through eframe. Candidate 2 of this plan's two: the app gets
its own winit + wgpu event loop and surface, egui-wgpu drives the UI over
it, and the renderer lives in a new `typewriter-render` crate under
`crates/`. eframe goes.

**Why.** The desk's 3D trajectory — scene camera, more machines, someday
the walkable office — wants surface ownership, pass order and frame pacing
now, and the windowing cost is accepted and deliberately divided into small
subtasks: T1a opens the crate and plain mode's window, T1b moves normal
mode's window over, T1c builds the device core in parallel.

**What the spike found that holds regardless, and what it becomes here.**
egui-wgpu 0.36, verified in its source: its egui mesh pipeline neither tests
nor writes the depth attachment (compare `Always`, write off), a paint
callback's `paint` runs inside its one render pass, and command buffers a
callback returns from `prepare` are submitted before that pass, wiped by
its clear. Under eframe these were constraints we lived inside; under our
own loop they are facts about a component we drive: the pass order is ours
— egui-under meshes, the 3D pass, egui-over meshes — the clear is ours (the
room's colour, at T5), and the depth buffer is ours to keep. Headless reach
proved symmetric — both shapes reach a device the same way, because the
pipeline's core is device-side (`depth/gpu.rs`'s tests already run on a
bare headless device, offscreen target and readback, no window) — so T1c's
core and its tests inherit that shape unchanged. The snapshot harness
reaches the pass with no device and no window at all
(`app/snapshot.rs`'s `TypewriterApp::nowhere` skips `Stage::start`; its
door is `Stage::snapshot_callback` and the CPU twin) — untouched by this
decision, settled by T6. And `depth/`'s maths was never in question:
composition touches none of `lighting.rs`'s CPU twins, `depth.wgsl`, the
decal bias (a `DepthBiasState` constant on the pipeline), or `Eye`/
`Camera`'s clip rows; the click contract — egui owning `Ui`, `Part`s and
`render::CLICK` — survives either shape.

**Version rule.** `typewriter-render` depends on wgpu directly, pinned to
the version egui-wgpu's released pair wants (0.36.2 → wgpu 30.0.1, per
Cargo.lock), so the renderer's device and pipeline types are the same crate
egui-wgpu's renderer holds — a second wgpu would be a type mismatch, not a
version difference. winit rides the same rule: egui-wgpu 0.36.2's own
`winit` feature pins 0.30.13.

**Settled with it.** The spike's open questions close: the renderer is a
crate, not a module of `typewriter`; and the clear becomes the room's
colour in normal mode, landing at T5. The spike's skeleton — `render3d.rs`
beside `depth/`, the eframe-callback install — sketched the overruled shape
and is dropped from the tree.