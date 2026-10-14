# Task 1 Report: TraceView Data Model & Node Accessors (`crates/doc`)

## Execution Summary
- **Target Components**: `crates/doc/src/node.rs`, `crates/doc/src/lib.rs`
- **Status**: DONE
- **Commit**: `ba84e604` (`feat(doc): add TraceView enum, node accessors, and crate re-export`)

## Implementation Details
1. **`TraceView` Enum**:
   - Defined in `crates/doc/src/node.rs` with 5 variants:
     - `TracingResult` (default, aliases: `tracingResult`, `result`)
     - `TracingResultWithOutlines` (aliases: `tracingResultWithOutlines`, `resultWithOutlines`)
     - `Outlines` (aliases: `outlines`, `outline`)
     - `OutlinesWithSource` (aliases: `outlinesWithSource`, `outlinesWithSourceImage`)
     - `SourceImage` (aliases: `sourceImage`, `source`)
   - Configured with `#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]` and `#[serde(rename_all = "camelCase")]`.

2. **`Node` Accessor Methods**:
   - `Node::trace_view(&self) -> TraceView`: Safely parses the `"view"` key from the node's `trace` JSON metadata if present, defaulting cleanly to `TraceView::TracingResult` on missing or malformed inputs without panics.
   - `Node::set_trace_view(&mut self, view: TraceView)`: Inserts or updates the `"view"` key in the node's `trace` JSON object. If `trace` was `None` or contained a non-object value, it initializes a valid JSON object map before inserting.

3. **Crate Re-export**:
   - Re-exported `TraceView` in `crates/doc/src/lib.rs` alongside `ImageObject, LAYER_COLORS, LayerColor, LiveShape, Node, NodeId, NodeKind, OpacityMask`.

4. **Testing & Quality Verification**:
   - Added unit test `test_trace_view_serialization_and_fallback` in `crates/doc/src/node.rs` verifying:
     - `n.trace_view()` returns `TraceView::TracingResult` when `trace` is `None`.
     - Malformed JSON gracefully defaults to `TraceView::TracingResult`.
     - Round-trip serialization/deserialization for all 5 variants.
     - `set_trace_view` and `trace_view` mutation roundtrip.
     - Aliases deserialization for all configured legacy and alternative aliases.
   - Verified that the unit test initially failed before implementation and subsequently passed.
   - Ran full test suite in `vectorcraft-doc` (143 unit tests passed, 0 failures).
   - Verified `cargo clippy -p vectorcraft-doc --all-targets -- -D warnings` passed with 0 warnings.
   - Verified `cargo fmt -p vectorcraft-doc --check` passed cleanly.
