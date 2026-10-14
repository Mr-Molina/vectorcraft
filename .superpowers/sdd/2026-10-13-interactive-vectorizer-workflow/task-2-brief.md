# Task 2 Brief: Engine Commands & Preview Undo Coalescing (`crates/engine`)

## Requirements & Scope
- **Files to Modify:**
  - `crates/engine/src/cmd/buildcmds.rs`
- **Files for Tests:**
  - `crates/engine/src/tests_build.rs`

## Context & Available Interfaces
From Task 1, `vectorcraft_doc::TraceView` is available:
- `TraceView::TracingResult`, `TraceView::TracingResultWithOutlines`, `TraceView::Outlines`, `TraceView::OutlinesWithSource`, `TraceView::SourceImage`.
- Methods on `Node`: `n.trace_view() -> TraceView` and `n.set_trace_view(view: TraceView)`.
- On `Session`: `s.begin_undo_group()` and `s.end_undo_group(cancel: bool)`.

## Task Details
1. **Register `imageTrace.setView` command in `crates/engine/src/cmd/buildcmds.rs` `specs()`**:
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

2. **Implement `trace_set_view` in `crates/engine/src/cmd/buildcmds.rs`**:
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
*Note*: `trace_targets(s, p)` already validates that selected/specified objects are Image Trace groups via `is_trace_group(n)`, returning `Err(EngineError::Other("select an Image Trace object".into()))` if targets are plain shapes, text, or un-traced nodes.

3. **Update `trace_make` in `crates/engine/src/cmd/buildcmds.rs`**:
- Parse `preview`:
  ```rust
  let preview = p.get("preview").and_then(Value::as_bool).unwrap_or(false);
  ```
- If `preview` is true and no undo group is currently open on active doc, open one:
  ```rust
  if preview && s.active().is_some_and(|st| st.undo_group.is_none()) {
      s.begin_undo_group();
  }
  ```
- Extract or preserve `view`:
  Check if `view` parameter was passed in `p`:
  ```rust
  let view_override: Option<vectorcraft_doc::TraceView> =
      p.get("view").and_then(|v| serde_json::from_value(v.clone()).ok());
  ```
  Check existing view if `target` is an existing trace group:
  ```rust
  let existing_view = s.doc()?.doc.node(target).map(|n| n.trace_view()).unwrap_or_default();
  let active_view = view_override.unwrap_or(existing_view);
  ```
  When setting `g.trace` (if `!expand`):
  ```rust
  let mut trace_meta = json!({ "preset": preset_label, "params": params_json });
  if let Ok(v) = serde_json::to_value(active_view) {
      trace_meta["view"] = v;
  }
  g.trace = Some(Box::new(trace_meta));
  ```
- At the end of `trace_make`:
  If `!preview` and an undo group is open, finalize it:
  ```rust
  if !preview && s.active().is_some_and(|st| st.undo_group.is_some()) {
      s.end_undo_group(false);
  }
  ```

4. **Verify `trace_expand` and `trace_release`**:
- In `trace_expand`: Ensure `children.retain(|c| !matches!(c.kind, NodeKind::Image(_)))` retains the vector paths with solid fill appearances and drops the hidden image, operating identically regardless of active `trace_view()`.
- In `trace_release`: Restores the image node with `visible: true`.

5. **Write TDD Integration Tests in `crates/engine/src/tests_build.rs`**:
- Write `test_image_trace_set_view_and_expand_invariants`:
  - New session, add image, select it, run `imageTrace.make`.
  - For each mode (`tracingResultWithOutlines`, `outlines`, `outlinesWithSource`, `sourceImage`, `tracingResult`), run `imageTrace.setView`, asserting `doc.node(trace_id).unwrap().trace_view()`.
  - Non-trace validation (Review Focus 2): Create `shape.rectangle`, call `imageTrace.setView` on it, assert error containing `"select an Image Trace object"`.
  - Expand invariant (Review Focus 3): Switch trace to `outlines`, call `imageTrace.expand`, assert `expanded.trace.is_none()`, `expanded.name.is_none()`, children non-empty with `!c.appearance.fill_paint().is_none()`.
- Write `test_image_trace_preview_coalescing`:
  - Initial state before trace: record `initial_count = s.doc().unwrap().history.undo.len()`.
  - Emulate 4 rapid preview drag frames (`preview: true` with changing threshold).
  - Commit final drag release with `preview: false`.
  - Assert `s.doc().unwrap().history.undo.len() == initial_count + 1`.
  - Run `edit.undo` once, assert doc node reverts to original image.

6. **Quality & Formatting Checks**:
- `cargo test -p vectorcraft-engine -- test_image_trace_`
- `cargo clippy -p vectorcraft-engine --all-targets -- -D warnings`
- `cargo fmt -p vectorcraft-engine --check`
- Commit: `git commit -m "feat(engine): implement imageTrace.setView and preview undo coalescing"`
