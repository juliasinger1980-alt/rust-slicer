# Project: Rust FDM Slicer (inspired by OrcaSlicer)

## Goal
Build a fast, reliable 3D-printing slicer in Rust. It takes an STL/3MF mesh and produces G-code.
The reference implementation is OrcaSlicer's slicing engine (`libslic3r`), cloned next to this
project at `./OrcaSlicer/src/libslic3r`. Use it to understand algorithms. Do NOT copy code
line by line. Read the algorithm, explain it, then write idiomatic Rust.

**License:** This project is a derivative of AGPL-3.0 code, so it is licensed AGPL-3.0.

## Tech stack
- Rust (stable), Cargo workspace
- `rayon` for parallelism
- `nalgebra` (or `glam`) for math
- A Clipper2 binding for polygon boolean/offset operations
- `stl_io` or `tobj`-style crates for mesh loading
- `svg` crate for debug output
- `proptest` for geometry property tests

## Architecture
Keep the engine separate from any UI or CLI.

```
crates/
  slicer-core/   # mesh, slicing, perimeters, infill, gcode (no I/O besides loading)
  slicer-cli/    # command line: slicer input.stl -o out.gcode --config cfg.toml
  slicer-debug/  # SVG/preview dumping helpers
tests/           # integration tests + reference comparisons
reference/       # outputs sliced by OrcaSlicer for comparison (JSON/G-code)
```

## Staged plan (do ONE stage per session, in order)
1. **Mesh loading + slicing into layers.** Output: closed polygon outlines per layer.
2. **Perimeters.** Inward offsets of outlines (wall count, line width).
3. **Infill.** Rectilinear first. Other patterns (grid, gyroid) come later.
4. **Top/bottom solid layers.**
5. **G-code generation.** Marlin flavor first. Extrusion math, retraction, temps, fan.
6. **Travel optimization + seam placement.**
7. **Supports, then advanced features** (arachne-style walls, brim/skirt, etc.).

Do not start a stage until the previous one passes its tests and is committed.

## Rules for every task
- **Tests before moving on.** Each stage needs unit tests and at least one test comparing against
  the OrcaSlicer reference output (layer count, polygon area per layer, total path length,
  estimated print time) within a stated tolerance.
- **Visual debugging.** Every geometry stage must be able to dump per-layer SVGs to
  `debug_out/` so results can be inspected by eye.
- **Never weaken a test to make it pass.** If a test fails, fix the code, or explain why the
  test's expectation is wrong and ask me before changing it.
- **Explain before implementing.** For any non-trivial algorithm, first summarize how Orca does it
  and which edge cases it handles, then implement.
- **Small diffs.** Prefer small, reviewable changes. No sweeping refactors unless I ask.
- **No `unsafe`** unless there is a measured performance reason, documented in a comment.
- **Run `cargo fmt`, `cargo clippy`, and `cargo test`** before declaring a task done.
- **Use integer coordinates** (scaled, like Clipper) for polygon operations to avoid floating
  point robustness bugs. Convert to floats only at the edges.
- **Handle bad input.** Non-manifold meshes, zero-area faces, and degenerate polygons must not
  panic. Return errors or repair them.

## Performance guidelines
- Measure first. Use `cargo bench` (criterion) or a profiler before optimizing.
- Slice layers in parallel with `rayon`. Layers are independent for stages 1 to 4.
- Avoid unnecessary allocations and cloning in hot loops.
- Keep a benchmark model set (calibration cube, benchy, one thin-wall model, one overhang model)
  and track slice time against OrcaSlicer's for each.

## Printer safety (important)
Generated G-code can damage a printer if wrong.
- Always include sane start/end G-code, and clamp temperatures and feedrates to configured limits.
- Validate that all coordinates stay inside the configured bed volume.
- Never assume a G-code file is safe: preview it in a G-code viewer before printing.
- First real prints should be small and simple (a 20mm calibration cube), watched in person.

## Workflow
- Commit after each passing stage with a clear message (`stage 2: perimeters, tests passing`).
- If a task is ambiguous or large, propose a plan and wait for confirmation instead of guessing.
- When unsure about Orca's behavior, say so and point to the relevant file in `libslic3r`
  rather than inventing behavior.