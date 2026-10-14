# Task 5 Brief: Layer 5 & 3 Engine Command: `render.overlay`

## Objective
Implement the `render.overlay` engine command in `crates/engine/src/cmd/docmenu.rs` (registered in `specs()`) and create comprehensive integration tests in `crates/engine/tests/render_overlay_test.rs`.

## Input Files
- Modify: `crates/engine/src/cmd/docmenu.rs`
- Create: `crates/engine/tests/render_overlay_test.rs`

## Requirements
1. **Command Specification**:
   - ID: `"render.overlay"`
   - Label: `"Render Labeled Overlay"`
   - Categories: `[]`
   - Shortcut: `None`
   - Help: `"{path: string, scale?: float, node_ids?: [int], ungrouped_only?: bool} → {path, annotated_count, dimensions: {width, height}}"`
   - Enabled check: `has_doc`
   - Handler: `render_overlay`

2. **Parameter Validation & Handling**:
   - `path`: Required string. Error if missing: `bad("render.overlay", "missing target path")`.
   - `scale`: Optional float, default `2.0`. Must validate with `scale.is_finite() && scale > 0.0`. If invalid, return `Err(bad("render.overlay", "scale must be a positive finite number"))`.
   - `node_ids`: Optional array of integers (node IDs). If provided, only annotate these specific nodes if they exist in the document and have `geometric_bounds()`.
   - `ungrouped_only`: Optional bool, default `false`. If true and `node_ids` is not provided, only annotate top-level children of layers that are not `NodeKind::Group`.
   - If neither `node_ids` nor `ungrouped_only` is specified, annotate all layer children that have `geometric_bounds()`.

3. **Rendering & Annotation**:
   - Fetch the first artboard: `doc.artboards.first().ok_or_else(|| bad("render.overlay", "document has no artboard"))?`.
   - Render the artboard region using:
     `let rendered = vectorcraft_render::Renderer::new().render_region(doc, artboard.rect, scale, true);`
   - Convert rendered pixel buffer to straight alpha:
     `let mut img = image::RgbaImage::from_raw(rendered.width, rendered.height, rendered.to_straight()).ok_or_else(|| bad("render.overlay", "invalid rendered image buffer"))?;`
   - For each target node:
     - Compute pixel bounds mapped from artboard coordinate space:
       `x0 = ((bounds.x0 - artboard.rect.x0) * scale).round() as i32;`
       `y0 = ((bounds.y0 - artboard.rect.y0) * scale).round() as i32;`
       `x1 = ((bounds.x1 - artboard.rect.x0) * scale).round() as i32;`
       `y1 = ((bounds.y1 - artboard.rect.y0) * scale).round() as i32;`
     - Draw semi-transparent bounding box interior tint (e.g. RGBA [0, 150, 255, 40]).
     - Draw 1px bounding box border (e.g. RGBA [0, 120, 240, 220]).
     - Draw high-contrast numbered badge displaying `node_id` using a built-in 3x5 bitmap font (scaled 2x) with dark background and white digits.
     - Increment `annotated_count`.
   - Ensure target directory exists (`std::fs::create_dir_all`).
   - Save image to `path` using `img.save(path).map_err(...)`.
   - Return:
     ```json
     {
         "path": path,
         "annotated_count": annotated,
         "dimensions": { "width": img.width(), "height": img.height() }
     }
     ```

4. **Safety & Invariants**:
   - Zero panic guarantee: NO `unwrap()`, `expect()`, `panic!`, `todo!`, `unimplemented!` in runtime code (`crates/engine/src/cmd/docmenu.rs`).
   - All pixel accesses clamped or bounds-checked (`0 <= px < img.width()`, etc.).
   - Clean-room invariant: 100% clean-room CIE / geometric logic. No Adobe dependencies or code.

5. **TDD Test Suite (`crates/engine/tests/render_overlay_test.rs`)**:
   - `test_render_overlay_emits_valid_png`: Creates a new doc, draws a rectangle, runs `render.overlay`, asserts `annotated_count == 1`, verifies file exists and is valid PNG, cleans up temp file.
   - `test_render_overlay_rejects_non_finite_scale`: Asserts `scale: NaN` and `scale: -1.0` return an error.
   - `test_render_overlay_missing_path`: Asserts missing path returns an error.
   - `test_render_overlay_node_ids_filter`: Creates 2 rectangles, passes `node_ids: [rect1]`, asserts `annotated_count == 1`.
   - `test_render_overlay_ungrouped_only`: Creates 1 loose rectangle and 1 group with 1 rectangle; with `ungrouped_only: true`, asserts only 1 is annotated.
