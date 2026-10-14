# Task 3 Report: Multi-Mode SIMD Hairline & Bitmap Rendering (`crates/render`)

## Summary & Scope
Task 3 implemented multi-mode rendering dispatch in `crates/render` for the Interactive Vectorizer Workflow. This enables Illustrator-parity view modes for Image Trace groups:
1. `TracingResult`: Renders vector paths only (`draw_children`).
2. `TracingResultWithOutlines`: Renders vector paths and superimposes screen-space antialiased cyan hairlines (`[0, 160, 255, 255]`).
3. `Outlines`: Renders black screen-space antialiased hairlines (`[0, 0, 0, 255]`) without path fills.
4. `OutlinesWithSource`: Renders the source image bitmap (bypassing `node.visible = false` via `force: true`) plus cyan hairlines (`[0, 160, 255, 255]`).
5. `SourceImage`: Renders the source image bitmap exclusively (bypassing `node.visible = false` via `force: true`).

In addition, `bounds_of` was updated so that when an Image Trace group is viewed in `SourceImage` or `OutlinesWithSource` mode, its cull bounds include the source bitmap even though `children[0].visible == false`.

## Implementation Details

### 1. `is_trace_group` Helper (`crates/render/src/lib.rs`)
Added helper function:
```rust
fn is_trace_group(n: &Node) -> bool {
    matches!(n.kind, NodeKind::Group { clip: false, .. })
        && n.name.as_deref() == Some("Image Trace")
        && n.children().is_some_and(|c| c.first().is_some_and(|i| matches!(i.kind, NodeKind::Image(_))))
}
```

### 2. Multi-Mode Dispatch in `draw_content` (`crates/render/src/lib.rs`)
In `draw_content(&mut self, ctx: &mut RenderContext, f: &Frame, n: &Node, knockout: bool)`:
When `is_trace_group(n)` is true and outline rendering is off (`!f.opts.outline`):
```rust
if is_trace_group(n) && !f.opts.outline {
    let view = n.trace_view();
    if let NodeKind::Group { children, .. } = &n.kind
        && let Some((source_img, vector_paths)) = children.split_first()
    {
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

### 3. Bounds Calculation in `bounds_of` (`crates/render/src/lib.rs`)
Updated child bounds union logic:
```rust
let mut acc: Option<Rect> = None;
for c in children {
    if c.visible || (is_trace_group(a) && matches!(a.trace_view(), TraceView::SourceImage | TraceView::OutlinesWithSource)) {
        acc = vectorcraft_geom::union_opt(acc, self.bounds_of(c));
    }
}
acc
```

## Testing & Verification

### TDD Cycle
1. Added `crates/render/tests/trace_render_tests.rs` with `test_render_all_five_trace_view_modes` and `test_render_trace_group_with_missing_image_graceful_fallback`.
2. Executed test and confirmed test failure on `TraceView::Outlines`: center pixels previously rendered vector fills instead of showing background.
3. Implemented changes in `crates/render/src/lib.rs`.
4. Extended tests with `test_render_trace_group_source_image_bounds_and_visibility` and verified hairline antialiasing edges and bounds propagation inside parent containers.
5. All 3 integration tests passed:
   - `test_render_all_five_trace_view_modes`
   - `test_render_trace_group_with_missing_image_graceful_fallback`
   - `test_render_trace_group_source_image_bounds_and_visibility`
6. Ran full test suite across `vectorcraft-render`: 200 tests passed (163 unit tests + 37 integration tests across 6 suites).
7. Verified `cargo clippy -p vectorcraft-render --all-targets -- -D warnings`: passed with 0 warnings.
8. Verified `cargo fmt -p vectorcraft-render --check`: passed.

## Commits
- Commit `73f8b2b1`: `feat(render): implement multi-mode hairline and bitmap rendering for Image Trace`
