# ADR 0013: scoped `unsafe` for the GL viewport

- Status: **Proposed by lane R (M7.R1), 2026-10-16; pending the director's confirmation
  at the merge.** Lane R took it as the decision AGENTS.md lets a lane take ("decide,
  record why, keep going"); it is not on the owner's list. Rejecting it means reverting
  M7.R1's GL path, since the ticket cannot be built without it (Context).
- Date: 2026-10-16
- Scope: where the workspace may contain `unsafe` code, and how that is enforced.
- Relates to: AGENTS.md "Never panic", ADR 0002 Amendment 1 (the viewport is L5 view
  code), ADR 0007 Amendment 3 (dependency table: `glow`, `glam`), `docs/gaps.md`.

## Context

M7.R1 ("3D placeholder", `plan/execution-plan.md` M7 lane R) draws the Render Window's
viewport through eframe's glow backend: an egui `PaintCallback` holding an
`egui_glow::CallbackFn`, with no second GL crate and no new framework. Every OpenGL call
`glow` 0.17 offers (`HasContext`: creating shaders, buffers and vertex arrays, setting
uniforms, drawing) is an `unsafe fn`, and egui_glow exposes no safe drawing API for a
callback. AGENTS.md, until this ADR, said `unsafe_code` is forbidden outright, and the
workspace set the lint to `forbid`, which no module can relax. The two cannot both hold.

Alternatives considered:

1. Draw the 3D scene with egui's own 2D meshes, projected on the CPU (no `unsafe`). It
   works for R1's lines and one box but has no depth buffer: R3's boxes and R4's meshes
   would need per-triangle sorting on the CPU every frame, and the ticket asks for GL.
2. A safe GL wrapper crate (`glium`, `three-d`): a second GL stack or a framework beside
   eframe's, both excluded by the ticket, and `three-d` pins its own `glow`.
3. Keep `forbid` and give the viewport its own crate with its own lint table: a new
   workspace crate the `INTERNAL_ONLY` rule would have to admit, for one file.

## Decision

1. `unsafe_code` is **denied** workspace-wide (`[workspace.lints.rust]`), no longer
   forbidden, so that one file can allow it.
2. The files that may use `unsafe` (or name the `unsafe_code` lint) are listed in
   `UNSAFE_ALLOWED` in `xtask/src/smuggle.rs`. Today there is one:
   `crates/ui-egui/src/viewport/gl.rs` (package `kitkraft-ui-egui`). `cargo xtask layers`
   scans every source file of every workspace package and fails on `unsafe` or
   `unsafe_code` in any other file, and on an entry whose file no longer exists. Adding a
   file to the list needs an amendment here.
3. The allowed file keeps its `unsafe` to GL calls on the context egui_glow paints with,
   on objects it created itself, with data from owned slices whose lengths GL is told;
   no raw pointers, mapped buffers or handles from outside. Its module docs state these
   conditions and every `unsafe` block points to them. It never reads plugin data: game
   geometry reaches it only as engine command results already turned into finite
   matrices and float buffers (ADR 0002 Amendment 1).
4. Parsing code (L0 to L4) stays free of `unsafe` without exception: the reason for the
   rule there, hostile input, is unchanged.

## Consequences

- AGENTS.md "Never panic" says `unsafe` is denied and names this ADR.
- `docs/gaps.md` lists the viewport's GL calls beside zlib-rs as code whose soundness
  rests on `unsafe` the workspace lints cannot check.
- A future renderer change (wgpu, for instance) would remove the entry, and the lint could
  return to `forbid`.
