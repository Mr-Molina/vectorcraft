# Task 5 Report: Layer 5 & 3 Engine Command: `render.overlay`

## Summary
Successfully implemented the `render.overlay` engine command in `crates/engine/src/cmd/docmenu.rs` and added comprehensive integration tests in `crates/engine/tests/render_overlay_test.rs`.

## Details of Changes
1. **Command Specification**:
   - Registered `render.overlay` with label `"Render Labeled Overlay"` in `crates/engine/src/cmd/docmenu.rs::specs()`.
   - Enabled condition: `has_doc`.
   - Categories: `[]`, Shortcut: `None`.
   - Help string: `"{path: string, scale?: float, node_ids?: [int], ungrouped_only?: bool} → {path, annotated_count, dimensions: {width, height}}"`.

2. **Parameter Validation & Handling**:
   - `path`: Validated as required non-empty string param; missing path returns `bad("render.overlay", "missing target path")`.
   - `scale`: Validated as positive finite float, defaulting to `2.0`. Invalid scale returns `bad("render.overlay", "scale must be a positive finite number")`.
   - `node_ids`: Optional filter by node ID, deduplicated and verified to exist with `geometric_bounds()`.
   - `ungrouped_only`: Optional bool defaulting to `false`. When true and `node_ids` is not provided, only layer direct children that are not `NodeKind::Group` are annotated.
   - Default: Annotates all layer children with `geometric_bounds()`.

3. **Rendering & Overlay Drawing**:
   - Rendered the first artboard using `vectorcraft_render::Renderer::new().render_region(...)` to straight RGBA.
   - Safe pixel blending helper (`blend_pixel`) with full boundary clipping to guarantee zero panics or out-of-bounds indexing.
   - Drew semi-transparent bounding box interior tint (RGBA `[0, 150, 255, 40]`).
   - Drew 1px bounding box border (RGBA `[0, 120, 240, 220]`).
   - Built a custom 3x5 bitmap font (scaled 2x) to render high-contrast dark badges with crisp white digits for each annotated `NodeId`.
   - Created output directories automatically via `std::fs::create_dir_all`.
   - Saved PNG to `path` and returned `{ path, annotated_count, dimensions: { width, height } }`.

4. **Safety & Zero-Panic Invariants**:
   - Zero `unwrap()`, `expect()`, `panic!`, `todo!`, or `unimplemented!` in runtime code.
   - Clippy workspace checks (`-D warnings`) pass cleanly with 0 warnings.
   - All pixel coordinates clamped to image dimensions `[0, width]` / `[0, height]`.
   - All float calculations validated with `.is_finite()`.

5. **Test Results**:
   - `cargo test -p vectorcraft-engine --test render_overlay_test`
   - All 5 integration tests pass:
     - `test_render_overlay_missing_path`: ok
     - `test_render_overlay_rejects_non_finite_scale`: ok
     - `test_render_overlay_ungrouped_only`: ok
     - `test_render_overlay_node_ids_filter`: ok
     - `test_render_overlay_emits_valid_png`: ok
