# Task 2 Report: Engine Commands & Preview Undo Coalescing (`crates/engine`)

## Summary of Completed Work
Implemented Task 2 for the Interactive Vectorizer Workflow in `crates/engine` (L5 crate):
1. **Registered and implemented `imageTrace.setView` command**:
   - Command signature: `imageTrace.setView` with parameters `{id?: NodeId, view: "tracingResult"|"tracingResultWithOutlines"|"outlines"|"outlinesWithSource"|"sourceImage"}`.
   - Validates that targets are Image Trace groups via `trace_targets(s, p)` (rejecting plain shapes with `"select an Image Trace object"`).
   - Updates `TraceView` on the target nodes under undoable session edit `"Image Trace View Mode"`.
2. **Integrated `preview: bool` undo coalescing into `imageTrace.make`**:
   - Initiates an undo group with `s.begin_undo_group()` on `preview: true` if one is not already open.
   - Coalesces rapid preview parameter tweaks into a single undo step.
   - Finalizes the open undo group with `s.end_undo_group(false)` on `preview: false`.
   - Preserves or updates `view` metadata (`active_view = view_override.unwrap_or(existing_view)`).
3. **Verified Expand and Release Invariants**:
   - In `trace_expand`: retains vector paths with solid fill appearances and drops the hidden image regardless of active view mode.
   - In `trace_release`: restores the image node with `visible: true`.
4. **Added Comprehensive TDD Integration Tests in `crates/engine/src/tests_build.rs`**:
   - `test_image_trace_set_view_and_expand_invariants`: tests all 5 `TraceView` variants, non-trace node rejection, and expand invariant.
   - `test_image_trace_preview_coalescing`: tests 4 rapid preview drag frames, 1 final commit frame, asserts history length increases by only 1, and verifies single undo reverts to original source image.

## Quality Gates & Verification
- **Test Results**:
  - `cargo test -p vectorcraft-engine -- test_image_trace_`: 2/2 passed
  - `cargo test -p vectorcraft-engine -- image_trace`: 7/7 passed
- **Clippy**:
  - `cargo clippy -p vectorcraft-engine --all-targets -- -D warnings`: Clean (0 warnings, 0 errors).
- **Rustfmt**:
  - `cargo fmt -p vectorcraft-engine --check`: Passed.
- **Commit**:
  - `bf1811d4 feat(engine): implement imageTrace.setView and preview undo coalescing`

## Report Metadata
- **Status**: DONE
- **Files Modified**:
  - `crates/engine/src/cmd/buildcmds.rs`
  - `crates/engine/src/tests_build.rs`
