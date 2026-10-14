# Task 1 Brief: TraceView Data Model & Node Accessors (`crates/doc`)

## Requirements & Scope
- **Files to Modify:**
  - `crates/doc/src/node.rs`
  - `crates/doc/src/lib.rs`
- **Files for Tests:**
  - `crates/doc/src/node.rs` (inline test `test_trace_view_serialization_and_fallback`)

## Task Details
1. Implement `TraceView` in `crates/doc/src/node.rs`:
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

2. Add methods to `Node` in `crates/doc/src/node.rs`:
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

3. Re-export `TraceView` in `crates/doc/src/lib.rs:72`:
```rust
pub use node::{ImageObject, LAYER_COLORS, LayerColor, LiveShape, Node, NodeId, NodeKind, OpacityMask, TraceView};
```

4. TDD Verification:
Write and execute `test_trace_view_serialization_and_fallback` in `crates/doc/src/node.rs`:
- Verify `n.trace_view()` returns `TraceView::TracingResult` when `trace` is `None`.
- Verify malformed JSON gracefully defaults to `TraceView::TracingResult`.
- Verify round-trip serialization/deserialization for all 5 variants.
- Run: `cargo test -p vectorcraft-doc -- test_trace_view_serialization_and_fallback`
- Ensure `cargo clippy -p vectorcraft-doc --all-targets -D warnings` and `cargo fmt --check` pass cleanly.
- Commit: `git commit -m "feat(doc): add TraceView enum, node accessors, and crate re-export"`
