# Interactive Vectorizer Workflow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement a high-performance, clean-room interactive vectorizer workflow for VectorCraft, introducing 5 Illustrator-parity View modes (`Tracing Result`, `Tracing Result with Outlines`, `Outlines`, `Outlines with Source Image`, `Source Image`), live slider debouncing with single-undo coalescing, and interactive panel/control bar controls.

**Architecture:** Extend `crates/doc` with a persisted `TraceView` enum; implement `imageTrace.setView` and preview transaction coalescing in `crates/engine`; add SIMD hairline outline and source bitmap composition in `crates/render`; wire the debounced live preview loop and View selectors in `crates/ui-egui`.

**Tech Stack:** Rust (edition 2024), `vello_cpu` (SIMD sparse strip rasterization), `kurbo` geometry, `egui` immediate mode UI, `serde` / `serde_json`.

**Spec:** [`docs/superpowers/specs/2026-10-13-interactive-vectorizer-workflow-design.md`](file:///s:/Github/vectorcraft/docs/superpowers/specs/2026-10-13-interactive-vectorizer-workflow-design.md)

**Execution Waves:**
- **Wave 1 (Foundation)**: Task 1 (`crates/doc` Data Model & `TraceView`)
- **Wave 2 (Parallel Core)**:
  - Task 2 (`crates/engine` Commands & Single-Undo Coalescing)
  - Task 3 (`crates/render` Multi-Mode SIMD Hairline & Bitmap Rendering)
- **Wave 3 (UI & Interactive Controls)**: Task 4 (`crates/ui-egui` Image Trace Panel, Live Drag Loop & Control Bar)
- **Wave 4 (Verification & Quality Gates)**: Task 5 (CI Pipeline, Roadmap Update & End-to-End Regression Verification)

---

## Global Constraints
- **Clean-room rule**: Never reference, use, or incorporate Adobe code, binaries, prologs, or assets.
- **Never crash**: No panics (`unwrap()`, `expect()`, `panic!`, `unreachable!`, `todo!`) in shipped code outside tests. Use `Result`, `Option`, and `?` propagation.
- **Strict Layering**: `cargo xtask layers` must pass without violation (L1 `doc`, L2 `trace`, L3 `render`, L5 `engine`, L6 `ui-egui`).
- **WASM Support**: Must compile cleanly on `wasm32-unknown-unknown` (`cargo xtask wasm`).
- **Quality Gates**: All tasks must adhere to `cargo xtask ci` (`fmt`, `clippy -D warnings`, `test`, `assets`, `brands`, `layers`, `wasm`).

---

## Review Focus
1. **Malformed or Missing Trace Metadata**: Legacy or handcrafted `Node.trace` values lacking `"view"` must gracefully default to `TraceView::TracingResult` without panicking or erroring.
2. **Non-Trace Target Validation**: Executing `imageTrace.setView` on plain shapes (e.g. rectangles), text, or un-traced groups must return a clean `EngineError::Other("select an Image Trace object")` without corrupting node state.
3. **Expand Invariant Across All View Modes**: Executing `imageTrace.expand` while in `Outlines` or `SourceImage` view must produce the identical, fully-colored vector shapes as expanding from `TracingResult`.
4. **Undo Coalescing Under Rapid Dragging**: Continuous slider dragging with `preview: true` must amend the active transaction in-place via `Session::begin_undo_group`, committing exactly one undo step when dragging ceases via `Session::end_undo_group`.
5. **Missing or Corrupt Child Image Handling**: If `children[0]` is unexpectedly missing or invalid in `SourceImage` or `OutlinesWithSource` mode, the renderer must safely fall back without crashing or aborting the frame.

---

### Task 1: TraceView Data Model & Node Accessors (`crates/doc`)

**Files:**
- Modify: `crates/doc/src/node.rs:460-480`
- Modify: `crates/doc/src/lib.rs:70-75`
- Test: `crates/doc/src/node.rs` (inline tests)

**Dependencies:**
- Blocked By: None
- Parallel Wave: Wave 1

**Interfaces:**
- Consumes: `serde::{Deserialize, Serialize}`
- Produces: `pub enum TraceView`, `pub fn Node::trace_view(&self) -> TraceView`, `pub fn Node::set_trace_view(&mut self, view: TraceView)`

#### Verification Pattern: Standard TDD
- [ ] **Step 1: Write the failing unit tests in `crates/doc/src/node.rs`**

```rust
#[test]
fn test_trace_view_serialization_and_fallback() {
    let mut n = Node::group(NodeId(1), vec![]);
    // Default should be TracingResult when trace is None
    assert_eq!(n.trace_view(), TraceView::TracingResult);

    // Malformed JSON should gracefully default to TracingResult (Review Focus 1)
    n.trace = Some(Box::new(serde_json::json!({ "garbage": 123 })));
    assert_eq!(n.trace_view(), TraceView::TracingResult);

    // Round-trip test all 5 variants
    for (val_str, expected) in [
        ("tracingResult", TraceView::TracingResult),
        ("tracingResultWithOutlines", TraceView::TracingResultWithOutlines),
        ("outlines", TraceView::Outlines),
        ("outlinesWithSource", TraceView::OutlinesWithSource),
        ("sourceImage", TraceView::SourceImage),
    ] {
        n.trace = Some(Box::new(serde_json::json!({ "view": val_str })));
        assert_eq!(n.trace_view(), expected);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**
Run: `cargo test -p vectorcraft-doc -- test_trace_view_serialization_and_fallback`
Expected: FAIL (`TraceView` not found).

- [ ] **Step 3: Implement `TraceView` and accessors in `crates/doc/src/node.rs` and re-export in `crates/doc/src/lib.rs`**

In `crates/doc/src/node.rs`:
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

impl Node {
    /// Extracts the active `TraceView` if this is an Image Trace group, defaulting to `TracingResult`.
    pub fn trace_view(&self) -> TraceView {
        self.trace
            .as_ref()
            .and_then(|t| t.get("view"))
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default()
    }

    /// Sets or updates the active `TraceView` in this node's trace metadata.
    pub fn set_trace_view(&mut self, view: TraceView) {
        let mut obj = self.trace.take().map(|b| *b).unwrap_or_else(|| serde_json::json!({}));
        if let Some(map) = obj.as_object_mut() {
            if let Ok(v) = serde_json::to_value(view) {
                map.insert("view".into(), v);
            }
        }
        self.trace = Some(Box::new(obj));
    }
}
```

In `crates/doc/src/lib.rs:72`:
```rust
pub use node::{ImageObject, LAYER_COLORS, LayerColor, LiveShape, Node, NodeId, NodeKind, OpacityMask, TraceView};
```

- [ ] **Step 4: Run test to verify it passes**
Run: `cargo test -p vectorcraft-doc -- test_trace_view_serialization_and_fallback`
Expected: PASS.

- [ ] **Step 5: Commit**
```bash
git add crates/doc/src/node.rs crates/doc/src/lib.rs
git commit -m "feat(doc): add TraceView enum, node accessors, and crate re-export"
```

---

### Task 2: Engine Commands & Preview Undo Coalescing (`crates/engine`)

**Files:**
- Modify: `crates/engine/src/cmd/buildcmds.rs:115-155, 620-765`
- Test: `crates/engine/src/tests_build.rs`

**Dependencies:**
- Blocked By: Task 1
- Parallel Wave: Wave 2 (parallel with Task 3)

**Interfaces:**
- Consumes: `vectorcraft_doc::{Node, NodeId, NodeKind, TraceView}`
- Produces: Commands `imageTrace.setView`, `imageTrace.make` with `preview: bool` support, updated `imageTrace.expand` and `imageTrace.release`

#### Verification Pattern: Standard TDD
- [ ] **Step 1: Write integration tests in `crates/engine/src/tests_build.rs`**

```rust
#[test]
fn test_image_trace_set_view_and_expand_invariants() {
    let mut s = Session::new();
    s.execute("file.new", &json!({ "width": 200, "height": 200 })).unwrap();
    let r = vectorcraft_trace::Raster::from_fn(40, 40, |x, y| {
        if x < 20 { [0, 0, 0, 255] } else { [255, 255, 255, 255] }
    });
    let img_id = add_image(&mut s, &r, vectorcraft_geom::Affine::IDENTITY);

    // Initial make
    let res = s.execute("imageTrace.make", &json!({ "preset": "Black and White Logo" })).unwrap();
    let trace_id = NodeId(res["id"].as_u64().unwrap());

    // Test imageTrace.setView across all modes
    for v in ["tracingResultWithOutlines", "outlines", "outlinesWithSource", "sourceImage", "tracingResult"] {
        s.execute("imageTrace.setView", &json!({ "id": trace_id.0, "view": v })).unwrap();
        let doc = &s.doc().unwrap().doc;
        assert_eq!(doc.node(trace_id).unwrap().trace_view(), serde_json::from_value(json!(v)).unwrap());
    }

    // Review Focus 2: non-trace target validation (test plain rectangle)
    let rect_res = s.execute("shape.rectangle", &json!({ "x": 0, "y": 0, "width": 10, "height": 10 })).unwrap();
    let rect_id = rect_res["id"].as_u64().unwrap();
    let err = s.execute("imageTrace.setView", &json!({ "id": rect_id, "view": "outlines" }));
    assert!(err.is_err());
    assert!(err.unwrap_err().to_string().contains("select an Image Trace object"));

    // Switch trace group to outlines mode and expand (Review Focus 3)
    s.execute("imageTrace.setView", &json!({ "id": trace_id.0, "view": "outlines" })).unwrap();
    s.execute("imageTrace.expand", &json!({ "id": trace_id.0 })).unwrap();

    let doc = &s.doc().unwrap().doc;
    let expanded = doc.node(trace_id).unwrap();
    assert!(expanded.trace.is_none());
    assert!(expanded.name.is_none());
    // Must contain vector paths with solid fill paint (not Paint::None)
    let children = expanded.children().unwrap();
    assert!(!children.is_empty());
    assert!(children.iter().all(|c| !c.appearance.fill_paint().is_none()));
}

#[test]
fn test_image_trace_preview_coalescing() {
    let mut s = Session::new();
    s.execute("file.new", &json!({ "width": 200, "height": 200 })).unwrap();
    let r = vectorcraft_trace::Raster::from_fn(40, 40, |x, y| {
        if x < 20 { [0, 0, 0, 255] } else { [255, 255, 255, 255] }
    });
    let img_id = add_image(&mut s, &r, vectorcraft_geom::Affine::IDENTITY);

    // Initial state before trace
    let initial_count = s.doc().unwrap().history.undo.len();

    // Emulate 4 rapid preview drag frames (Review Focus 4)
    for threshold in [100, 110, 120, 130] {
        s.execute("imageTrace.make", &json!({
            "preset": "Black and White Logo",
            "params": { "threshold": threshold },
            "preview": true
        })).unwrap();
    }

    // Commit final drag release
    s.execute("imageTrace.make", &json!({
        "preset": "Black and White Logo",
        "params": { "threshold": 140 },
        "preview": false
    })).unwrap();

    // Verify history grew by exactly 1 undo step
    assert_eq!(s.doc().unwrap().history.undo.len(), initial_count + 1);

    // Undoing once must revert completely back to the un-traced image
    s.execute("edit.undo", &json!({})).unwrap();
    let doc = &s.doc().unwrap().doc;
    assert!(doc.node(img_id).is_some());
}
```

- [ ] **Step 2: Run tests to verify they fail**
Run: `cargo test -p vectorcraft-engine -- test_image_trace_set_view_and_expand_invariants`
Expected: FAIL (`imageTrace.setView` unknown command).

- [ ] **Step 3: Implement `imageTrace.setView` and `preview` handling in `crates/engine/src/cmd/buildcmds.rs`**
  - Register `imageTrace.setView` command in `specs()`:
    ```rust
    cmd!(
        "imageTrace.setView",
        "View",
        ["Object", "Image Trace"],
        None,
        "{id?: NodeId, view: \"tracingResult\"|\"tracingResultWithOutlines\"|\"outlines\"|\"outlinesWithSource\"|\"sourceImage\"} set Image Trace display mode",
        has_selection_or_ids,
        trace_set_view
    ),
    ```
  - Implement `trace_set_view(s: &mut Session, p: &Value) -> Result<Value>`:
    ```rust
    fn trace_set_view(s: &mut Session, p: &Value) -> Result<Value> {
        const C: &str = "imageTrace.setView";
        let view_str = str_param(p, "view").ok_or_else(|| bad(C, "missing view"))?;
        let view: vectorcraft_doc::TraceView =
            serde_json::from_value(json!(view_str)).map_err(|e| bad(C, format!("unknown view `{view_str}`: {e}")))?;
        let targets = trace_targets(s, p)?;
        s.edit("Image Trace View Mode", |d, _| {
            for id in &targets {
                let n = d.node_mut(*id).ok_or(EngineError::NoNode(*id))?;
                n.set_trace_view(view);
            }
            Ok(())
        })?;
        Ok(json!({ "ids": ids_json(&targets), "view": view_str }))
    }
    ```
  - Extend `trace_make(s: &mut Session, p: &Value, expand: bool)`:
    ```rust
    let preview = p.get("preview").and_then(Value::as_bool).unwrap_or(false);
    if preview && s.active().is_some_and(|st| st.undo_group.is_none()) {
        s.begin_undo_group();
    }
    let view_override: Option<vectorcraft_doc::TraceView> =
        p.get("view").and_then(|v| serde_json::from_value(v.clone()).ok());
    ```
    During node group creation, preserve existing `trace_view()` from `target` if present, or apply `view_override`:
    ```rust
    if !expand {
        let active_view = view_override.unwrap_or(existing_view);
        let mut trace_meta = json!({ "preset": preset_label, "params": params_json });
        if let Ok(v) = serde_json::to_value(active_view) {
            trace_meta["view"] = v;
        }
        g.trace = Some(Box::new(trace_meta));
    }
    ```
    And at the end of `trace_make`:
    ```rust
    if !preview && s.active().is_some_and(|st| st.undo_group.is_some()) {
        s.end_undo_group(false);
    }
    ```
  - In `trace_expand`: ensure `children.retain(|c| !matches!(c.kind, NodeKind::Image(_)))` preserves all vector paths and drops `NodeKind::Image` regardless of active view mode.

- [ ] **Step 4: Run tests to verify they pass**
Run: `cargo test -p vectorcraft-engine -- test_image_trace_`
Expected: PASS.

- [ ] **Step 5: Commit**
```bash
git add crates/engine/src/cmd/buildcmds.rs crates/engine/src/tests_build.rs
git commit -m "feat(engine): implement imageTrace.setView and preview undo coalescing"
```

---

### Task 3: Multi-Mode SIMD Hairline & Bitmap Rendering (`crates/render`)

**Files:**
- Modify: `crates/render/src/lib.rs:635-650, 870-915`
- Test: `crates/render/tests/trace_render_tests.rs`

**Dependencies:**
- Blocked By: Task 1
- Parallel Wave: Wave 2 (parallel with Task 2)

**Interfaces:**
- Consumes: `vectorcraft_doc::{Node, NodeKind, TraceView}`
- Produces: Visual rendering of all 5 `TraceView` modes via `vello_cpu` hairlines and bitmaps

#### Verification Pattern: Standard TDD
- [ ] **Step 1: Write render tests in `crates/render/tests/trace_render_tests.rs`**

```rust
use std::sync::Arc;
use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::{Appearance, Document, Node, NodeId, NodeKind, TraceView};
use vectorcraft_geom::{Affine, BezPath, Rect, Shape};
use vectorcraft_render::{RenderOptions, Renderer};

#[test]
fn test_render_all_five_trace_view_modes() {
    let mut doc = Document::new(200.0, 200.0);
    let mut r = Renderer::new();

    // Encode a 10x10 mock PNG using image::RgbaImage directly (avoiding undeclared trace dependency)
    let img = image::RgbaImage::from_pixel(10, 10, image::Rgba([255, 0, 0, 255]));
    let mut png_bytes = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png).unwrap();
    doc.images.insert("test_key".into(), vectorcraft_doc::ImageBlob::new("image/png", png_bytes));

    let img_node = Node {
        id: NodeId(10),
        name: Some("Source Image".into()),
        kind: NodeKind::Image(vectorcraft_doc::ImageObject {
            key: "test_key".into(),
            xf: Affine::IDENTITY,
            width: 100,
            height: 100,
            link: None,
            placement: Default::default(),
        }),
        visible: false,
        ..Default::default()
    };
    let rect_path = vectorcraft_geom::PathData::from_bezpath(&Rect::new(10.0, 10.0, 90.0, 90.0).to_path(0.1));
    let mut rect_node = Node::path(NodeId(11), rect_path, Appearance::default());
    rect_node.appearance.set_fill(Paint::solid(Color::rgb8(255, 0, 0)));

    let mut trace_group = Node::group(NodeId(1), vec![Arc::new(img_node), Arc::new(rect_node)]);
    trace_group.name = Some("Image Trace".into());

    let opts = RenderOptions {
        outline: false,
        background: Some([255, 255, 255, 255]),
        artboards: true,
        ..Default::default()
    };

    for view in [
        TraceView::TracingResult,
        TraceView::TracingResultWithOutlines,
        TraceView::Outlines,
        TraceView::OutlinesWithSource,
        TraceView::SourceImage,
    ] {
        trace_group.set_trace_view(view);
        let mut test_doc = doc.clone();
        test_doc.insert(None, 0, trace_group.clone()).unwrap();
        let rendered = r.render(&test_doc, 200, 200, Affine::IDENTITY, &opts);
        assert_eq!(rendered.width, 200);
        assert_eq!(rendered.height, 200);
        assert!(rendered.pixels.iter().any(|&b| b != 0));
    }
}

#[test]
fn test_render_trace_group_with_missing_image_graceful_fallback() {
    let mut doc = Document::new(200.0, 200.0);
    let mut r = Renderer::new();

    let mut corrupt_group = Node::group(NodeId(1), vec![]);
    corrupt_group.name = Some("Image Trace".into());
    corrupt_group.set_trace_view(TraceView::SourceImage);

    doc.insert(None, 0, corrupt_group).unwrap();
    let opts = RenderOptions::default();
    let rendered = r.render(&doc, 200, 200, Affine::IDENTITY, &opts);
    assert_eq!(rendered.width, 200);
}
```

- [ ] **Step 2: Run test to verify it fails**
Run: `cargo test -p vectorcraft-render --test trace_render_tests`
Expected: FAIL (view modes rendering not yet implemented).

- [ ] **Step 3: Implement View Mode rendering in `crates/render/src/lib.rs`**
  - Add helper function `fn is_trace_group(n: &Node) -> bool`:
    ```rust
    fn is_trace_group(n: &Node) -> bool {
        matches!(n.kind, NodeKind::Group { clip: false, .. })
            && n.name.as_deref() == Some("Image Trace")
            && n.children().is_some_and(|c| c.first().is_some_and(|i| matches!(i.kind, NodeKind::Image(_))))
    }
    ```
  - In `draw_content`:
    ```rust
    if is_trace_group(n) && !f.opts.outline {
        let view = n.trace_view();
        if let Some((source_img, vector_paths)) = children.split_first() {
            match view {
                TraceView::TracingResult => {
                    self.draw_children(ctx, f, vector_paths, knockout);
                }
                TraceView::TracingResultWithOutlines => {
                    self.draw_children(ctx, f, vector_paths, knockout);
                    for vp in vector_paths {
                        if let Some(bp) = vp.stroke_path() {
                            self.hairline(ctx, f, &bp, [0, 160, 255, 255]);
                        }
                    }
                }
                TraceView::Outlines => {
                    for vp in vector_paths {
                        if let Some(bp) = vp.stroke_path() {
                            self.hairline(ctx, f, &bp, [0, 0, 0, 255]);
                        }
                    }
                }
                TraceView::OutlinesWithSource => {
                    self.draw_node(ctx, f, source_img, true);
                    for vp in vector_paths {
                        if let Some(bp) = vp.stroke_path() {
                            self.hairline(ctx, f, &bp, [0, 160, 255, 255]);
                        }
                    }
                }
                TraceView::SourceImage => {
                    self.draw_node(ctx, f, source_img, true);
                }
            }
            return;
        }
    }
    ```
  - In `bounds_of`: If `is_trace_group(a)` and `view` is `SourceImage` or `OutlinesWithSource`, union bounds with `self.bounds_of(&children[0])`.

- [ ] **Step 4: Run test to verify it passes**
Run: `cargo test -p vectorcraft-render --test trace_render_tests`
Expected: PASS.

- [ ] **Step 5: Commit**
```bash
git add crates/render/src/lib.rs crates/render/tests/trace_render_tests.rs
git commit -m "feat(render): implement multi-mode hairline and bitmap rendering for Image Trace"
```

---

### Task 4: Interactive Panel, Live Drag Loop & Control Bar (`crates/ui-egui`)

**Files:**
- Modify: `crates/ui-egui/src/panels/image_trace.rs`
- Modify: `crates/ui-egui/src/panels/properties.rs`
- Test: `crates/ui-egui/src/panels/image_trace.rs` (tests module)

**Dependencies:**
- Blocked By: Task 2, Task 3
- Parallel Wave: Wave 3

**Interfaces:**
- Consumes: `imageTrace.setView`, `imageTrace.make { preview }`, `imageTrace.expand`, `imageTrace.release`
- Produces: Integrated View dropdown, Preview checkbox, debounced drag interaction, and Control Bar shortcuts

#### Verification Pattern: Standard TDD / UI Verification
- [ ] **Step 1: Write headless UI test in `crates/ui-egui/src/panels/image_trace.rs`**

```rust
#[test]
fn test_image_trace_panel_with_view_modes_headless() {
    let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
    app.session.execute("file.new", &json!({"width": 200, "height": 200})).unwrap();

    let img = image::RgbaImage::from_fn(20, 20, |x, y| image::Rgba([x as u8, y as u8, 0, 255]));
    let mut png = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();

    let _img_id = app.session.edit("img", |d, sel| {
        d.images.insert("img1".into(), vectorcraft_doc::ImageBlob::new("image/png", png));
        let id = d.alloc_id();
        let l = d.default_layer();
        d.insert(
            l,
            0,
            vectorcraft_doc::Node::new(
                id,
                vectorcraft_doc::NodeKind::Image(vectorcraft_doc::ImageObject {
                    key: "img1".into(),
                    width: 20,
                    height: 20,
                    xf: vectorcraft_geom::Affine::IDENTITY,
                    link: None,
                    placement: Default::default(),
                }),
            ),
        )?;
        sel.set([id]);
        Ok(id)
    }).unwrap();

    let trace_res = app.session.execute("imageTrace.make", &json!({ "preset": "Default" })).unwrap();
    let trace_id = vectorcraft_doc::NodeId(trace_res["id"].as_u64().unwrap());
    app.session.select(|_, sel| sel.set([trace_id])).unwrap();

    for _ in 0..3 {
        let ctx = egui::Context::default();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            show(&mut app, ui);
            menu(&mut app, ui);
            control_bar(&mut app, ui);
        });
        out.textures_delta.clear();
    }
}
```

- [ ] **Step 2: Run test to verify initial headless frame executes**
Run: `cargo test -p vectorcraft-ui-egui -- test_image_trace_panel_with_view_modes_headless`
Expected: PASS/FAIL depending on current implementation.

- [ ] **Step 3: Update `crates/ui-egui/src/panels/image_trace.rs`**
  - Add view modes catalog and shared dropdown helper:
    ```rust
    pub(crate) const VIEW_MODES: [(&str, &str); 5] = [
        ("tracingResult", "Tracing Result"),
        ("tracingResultWithOutlines", "Tracing Result with Outlines"),
        ("outlines", "Outlines"),
        ("outlinesWithSource", "Outlines with Source Image"),
        ("sourceImage", "Source Image"),
    ];

    pub(crate) fn view_dropdown(app: &mut VectorcraftApp, ui: &mut Ui, current: &str, width: f32) {
        let current_label = VIEW_MODES.iter().find(|m| m.0 == current).map_or("Tracing Result", |m| m.1);
        let labels: Vec<&str> = VIEW_MODES.iter().map(|m| m.1).collect();
        if let Some(i) = widgets::dropdown(ui, "trace-view-mode", tl!(current_label), &labels, width) {
            let chosen = VIEW_MODES[i].0;
            let _ = app.run("imageTrace.setView", json!({ "view": chosen }));
        }
    }
    ```
  - In `TraceUi`:
    - Add `view: String` (default `"tracingResult"`).
    - Add `preview: bool` (default `true`).
    - Add `last_drag_time: f64`.
  - In `show()`:
    - Render `View:` dropdown in top horizontal row beside Preset using `view_dropdown`.
    - Render eye icon button: when clicked or held, toggle view between current view and `SourceImage`.
    - In slider interaction loop:
      - If `changed && st.preview`: debounce to 60ms, dispatching `app.run("imageTrace.make", json!({ "preset": preset, "params": st.params, "preview": true }))`.
      - If `release`: dispatch `app.run("imageTrace.make", json!({ "preset": preset, "params": st.params, "preview": false }))`.
    - Render `Preview` checkbox in bottom action row.
  - In `control_bar()`:
    - Add `View:` dropdown beside `Preset:`.
  - In `crates/ui-egui/src/panels/properties.rs`:
    - When Image Trace object is selected, render `View:` dropdown using `image_trace::view_dropdown`.

- [ ] **Step 4: Run tests to verify all tests pass**
Run: `cargo test -p vectorcraft-ui-egui -- image_trace`
Expected: PASS.

- [ ] **Step 5: Commit**
```bash
git add crates/ui-egui/src/panels/image_trace.rs crates/ui-egui/src/panels/properties.rs
git commit -m "feat(ui): add View mode selector, Preview toggle, and debounced drag loop to Image Trace"
```

---

### Task 5: CI Pipeline, Roadmap Update & End-to-End Verification

**Files:**
- Modify: `ROADMAP.md`
- Test: Full repository test suite (`cargo xtask ci`)

**Dependencies:**
- Blocked By: Task 1, Task 2, Task 3, Task 4
- Parallel Wave: Wave 4

**Interfaces:**
- Consumes: All completed tasks
- Produces: Clean `cargo xtask ci` report and updated roadmap closing the view modes gap

#### Verification Pattern: Standard TDD / System Quality Gate
- [ ] **Step 1: Update `ROADMAP.md`**
  - Update `ROADMAP.md` under M12 / Image Trace: mark "Image Trace View modes (Outlines, Source Image)" as shipped.
  - In the gap list table, update Image Trace row.
- [ ] **Step 2: Run formatting and lints**
Run: `cargo fmt --check`
Run: `cargo clippy --workspace --all-targets -D warnings`
Expected: PASS (zero warnings, zero errors).
- [ ] **Step 3: Run layering and WASM checks**
Run: `cargo xtask layers`
Run: `cargo xtask wasm`
Expected: PASS (zero violations, clean wasm compilation).
- [ ] **Step 4: Execute full test suite**
Run: `cargo test --workspace`
Expected: PASS (100% green across all crates).
- [ ] **Step 5: Run full CI gate**
Run: `cargo xtask ci`
Expected: PASS (all 7 stages passing: `fmt`, `clippy`, `test`, `assets`, `brands`, `layers`, `wasm`).
- [ ] **Step 6: Commit**
```bash
git add ROADMAP.md
git commit -m "chore: update ROADMAP for Image Trace view modes and complete CI verification"
```
