# Interactive Vectorizer Workflow Design Specification

- **Date**: 2026-10-13
- **Status**: Approved
- **Target**: VectorCraft (`crates/doc`, `crates/render`, `crates/engine`, `crates/ui-egui`)
- **Parity Reference**: Adobe Illustrator Image Trace (Clean-Room public documentation)

---

## 1. Executive Summary

VectorCraft includes a high-performance clean-room raster vectorization engine (`vectorcraft-trace`) capable of quantizing, denoising, boundary tracing, and Bézier fitting raster images into vector path networks across 12 built-in presets.

Currently, vectorization occurs in an immediate batch execution mode (`imageTrace.make`), and view modes (`Outlines`, `Source Image`, etc.) remain an open roadmap gap (`ROADMAP.md`).

This specification defines the architectural design for the **Interactive Vectorizer Workflow**:
1. **5 Interactive View Modes**: `Tracing Result`, `Tracing Result with Outlines`, `Outlines`, `Outlines with Source Image`, and `Source Image`.
2. **Native Vector Engine & Renderer Integration**: Persisted `TraceView` metadata on `Node.trace` rendered via `vello_cpu` hairline SIMD passes directly inside `crates/render`.
3. **Live Interactive Preview & Undo Coalescing**: Real-time debounced slider updates during dragging with transaction coalescing, producing a single atomic Undo step upon commit.
4. **Contextual UI Controls**: Illustrator-parity View dropdown, Preview checkbox toggle, quick A/B source comparison, and Control Bar / Properties Panel integrations.

---

## 2. Architecture & Data Model

### 2.1 Layering & Dependency Flow
Following VectorCraft's strict layering rules (`cargo xtask layers`):
- `crates/doc` (**L1**): Hosts the `TraceView` enum and metadata accessor methods on `Node`.
- `crates/trace` (**L2**): Contains the mathematical vectorization algorithms (quantize, contour, fit).
- `crates/render` (**L3**): Reads `Node.trace` and `TraceView` to render vector paths, overlaid wireframe hairlines, or underlying source images without mutating document geometry.
- `crates/engine` (**L5**): Exposes `imageTrace.setView`, enhances `imageTrace.make` with `preview: bool` undo coalescing, and manages `expand` / `release` invariants.
- `crates/ui-egui` (**L6**): Provides the Image Trace panel, View selector, debounced interaction loop, and Control Bar / Properties shortcuts.

### 2.2 Data Structures (`crates/doc`)

```rust
/// The display mode of an Image Trace group.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TraceView {
    /// Render vector paths with their filled appearances (default).
    #[default]
    #[serde(alias = "tracingResult", alias = "result")]
    TracingResult,
    /// Render filled vector paths with high-contrast contour outlines overlaid.
    #[serde(alias = "tracingResultWithOutlines", alias = "resultWithOutlines")]
    TracingResultWithOutlines,
    /// Render 1px vector path outlines only (wireframe preview, no fill).
    #[serde(alias = "outlines", alias = "outline")]
    Outlines,
    /// Render the source image with vector path outlines overlaid.
    #[serde(alias = "outlinesWithSource", alias = "outlinesWithSourceImage")]
    OutlinesWithSource,
    /// Render only the original source image (suppressing vector paths).
    #[serde(alias = "sourceImage", alias = "source")]
    SourceImage,
}
```

In `crates/doc/src/node.rs`, `Node` holds:
```rust
impl Node {
    /// Extracts the active `TraceView` if this is an Image Trace group, defaulting to `TracingResult`.
    pub fn trace_view(&self) -> TraceView {
        self.trace
            .as_ref()
            .and_then(|t| t.get("view"))
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default()
    }
}
```

---

## 3. Renderer Architecture (`crates/render`)

### 3.1 Trace Group Identification
An Image Trace group is identified in `crates/render` by:
- Group node with name `"Image Trace"`.
- First child (`children[0]`) is a `NodeKind::Image`.
- Remaining children (`children[1..]`) are traced vector shapes.

### 3.2 View Mode Dispatching in `draw_content`
Within `Renderer::draw_content(ctx, f, n, knockout)`:
1. When visiting an Image Trace group, inspect `view = n.trace_view()`.
2. Dispatch according to `view`:
   - **`TracingResult`**:
     Execute `self.draw_children(ctx, f, &children[1..], knockout)` as standard. `children[0]` is ignored since `visible == false`.
   - **`TracingResultWithOutlines`**:
     First, execute `self.draw_children(ctx, f, &children[1..], knockout)`.
     Second, iterate over `children[1..]`, extracting each path's `BezPath` and calling `self.hairline(ctx, f, path, [0, 160, 255, 255])` to overlay a crisp cyan contour.
   - **`Outlines`**:
     Skip fills. For each path in `children[1..]`, call `self.hairline(ctx, f, path, [0, 0, 0, 255])` to render a 1px black wireframe outline.
   - **`OutlinesWithSource`**:
     First, draw `children[0]` (the source bitmap) via `self.draw_node(ctx, f, &children[0], false)` regardless of its `visible` flag.
     Second, iterate over `children[1..]` and call `self.hairline(ctx, f, path, [0, 160, 255, 255])`.
   - **`SourceImage`**:
     Draw only `children[0]` via `self.draw_node(ctx, f, &children[0], false)`. Suppress vector paths.

### 3.3 Culling and Bounding Boxes
When `view` is `SourceImage` or `OutlinesWithSource`, bounds calculations in `bounds_of` evaluate `children[0]` alongside `children[1..]` to ensure proper canvas clipping and dirty-rect invalidation.

---

## 4. Engine Commands & Undo Coalescing (`crates/engine`)

### 4.1 Command Specifications
- **`imageTrace.setView`**:
  - Parameters: `{ id?: NodeId, view: TraceView }`
  - Action: Updates `node.trace["view"]` on the target Image Trace group.
  - Undo: Recorded as `"Image Trace View Mode"`.
- **`imageTrace.make`**:
  - Parameters: `{ id?: NodeId, preset?: String, params?: Value, view?: TraceView, preview?: bool }`
  - Interactive Coalescing:
    - If `preview == true`: The edit updates the document tree while marking the transaction as an ephemeral preview. If a preceding uncommitted preview transaction exists, it amends the transaction in place rather than stacking a new undo state.
    - If `preview == false` (or omitted): The transaction commits as a finalized `"Image Trace"` undo record.
- **`imageTrace.expand`**:
  - Invariant: Always retains `children[1..]` with their filled vector colors, sets group `name = None`, sets `trace = None`, and purges `children[0]` (the bitmap). This operates identically regardless of whether `view` was `Outlines` or `SourceImage`.
- **`imageTrace.release`**:
  - Invariant: Replaces the trace group with `children[0]` (the bitmap) restored to `visible = true`.

---

## 5. UI/UX Interaction Model (`crates/ui-egui`)

### 5.1 Image Trace Panel Updates
1. **View Mode Dropdown**:
   - Placed in the top row alongside `Preset`.
   - Options: `Tracing Result`, `Tracing Result with Outlines`, `Outlines`, `Outlines with Source Image`, `Source Image`.
   - Selection runs `imageTrace.setView`.
2. **Preview Toggle**:
   - Checkbox at the bottom of the panel labeled `Preview` (default: `true`).
   - When checked, slider adjustments retrace automatically.
   - When unchecked, parameter edits do not trigger document edits until `Trace` is clicked.
3. **Debounced Live Drag Loop**:
   - While dragging sliders (`paths`, `corners`, `noise`, `threshold`, `colors`):
     - Retrace requests are debounced to ~60ms.
     - While dragging, `imageTrace.make` is called with `preview: true`.
     - When dragging terminates (`drag_stopped`), `imageTrace.make` is called with `preview: false`.
4. **Quick Comparison (Eye Control)**:
   - Clicking and holding an eyeball icon button temporarily switches the active view to `SourceImage`, restoring the previous view mode on release.

### 5.2 Control Bar & Properties Panel Integration
- When an Image Trace object is selected:
  - **Control Bar**: Displays `Preset:` [dropdown], `View:` [dropdown], Panel button, and `[Expand]`.
  - **Properties Panel**: Surfaces `View` dropdown, `[Expand]`, and `[Release]`.

---

## 6. Verification & Quality Gates

### 6.1 Automated Test Suite
1. **`crates/engine/src/tests_build.rs`**:
   - `test_image_trace_view_modes`: Executes `imageTrace.setView` across all 5 modes and validates `node.trace_view()`.
   - `test_image_trace_preview_coalescing`: Emulates 5 consecutive slider adjustments with `preview: true` followed by `preview: false`, verifying that a single undo command reverts the document to the initial state.
   - `test_image_trace_expand_invariant`: Validates that expanding from `Outlines` or `SourceImage` view yields the identical vector output as expanding from `TracingResult`.
   - `test_image_trace_release`: Confirms clean restoration of the bitmap object.
2. **`crates/render` Unit & Integration Tests**:
   - Render contract tests validating that all 5 view modes render headless without panics.
   - Pixel checks verifying wireframe hairlines, bitmap textures, and hybrid composition.
3. **`crates/ui-egui` Headless Tests**:
   - `panel_and_menu_draw_headless`: Verifies that UI panels draw without error across all view states.

### 6.2 Compliance & CI
- Zero warnings under `cargo clippy --workspace --all-targets -D warnings`.
- Strict panic-free code in shipped modules (`no unwrap()`, `no expect()`, `no panic!`).
- Pass all 7 stages of `cargo xtask ci` (`fmt`, `clippy`, `test`, `assets`, `brands`, `layers`, `wasm`).
