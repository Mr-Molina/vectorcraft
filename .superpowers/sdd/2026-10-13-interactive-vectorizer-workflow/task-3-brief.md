# Task 3 Brief: Multi-Mode SIMD Hairline & Bitmap Rendering (`crates/render`)

## Requirements & Scope
- **Files to Modify:**
  - `crates/render/src/lib.rs:635-650, 870-915`
- **Files for Tests:**
  - `crates/render/tests/trace_render_tests.rs`

## Context & Available Interfaces
- `crates/render` is Layer 3. It depends on `vectorcraft-doc` (L1), `image`, `vello_cpu`, `vectorcraft-geom`, `vectorcraft-color`.
- `vectorcraft_doc::TraceView` is available:
  `TracingResult`, `TracingResultWithOutlines`, `Outlines`, `OutlinesWithSource`, `SourceImage`.
- `Node::trace_view(&self) -> TraceView` extracts the view mode.
- `Node::stroke_path(&self) -> Option<vectorcraft_geom::BezPath>` extracts the geometric BezPath of any path or compound node!
- In `Renderer`: `self.hairline(&mut self, ctx: &mut RenderContext, f: &Frame, bp: &BezPath, rgba: [u8; 4])` renders single-pixel antialiased hairline strokes in screen space.
- In `Renderer`: `self.draw_node(&mut self, ctx: &mut RenderContext, f: &Frame, n: &Node, force: bool)` draws a node. Passing `force: true` bypasses `!n.visible` checks, which is essential because `children[0]` (the source bitmap) of an Image Trace group is stored with `visible = false`!

## Task Details
1. **Add `is_trace_group` helper in `crates/render/src/lib.rs`**:
```rust
fn is_trace_group(n: &Node) -> bool {
    matches!(n.kind, NodeKind::Group { clip: false, .. })
        && n.name.as_deref() == Some("Image Trace")
        && n.children().is_some_and(|c| c.first().is_some_and(|i| matches!(i.kind, NodeKind::Image(_))))
}
```

2. **Update `draw_content` in `crates/render/src/lib.rs`**:
In `draw_content(&mut self, ctx: &mut RenderContext, f: &Frame, n: &Node, knockout: bool)`:
When `is_trace_group(n)` is true and `!f.opts.outline`:
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

3. **Update `bounds_of` in `crates/render/src/lib.rs`**:
When calculating bounds for a non-fx group, if it is an Image Trace group and `view` is `SourceImage` or `OutlinesWithSource`, union bounds with `self.bounds_of(c)` even if `c.visible == false`:
```rust
        let b = match &a.kind {
            NodeKind::Layer { children, clip: false, .. } | NodeKind::Group { children, clip: false } if !fx::has_object_fx(a) => {
                let mut acc: Option<Rect> = None;
                for c in children {
                    if c.visible || (is_trace_group(a) && matches!(a.trace_view(), TraceView::SourceImage | TraceView::OutlinesWithSource)) {
                        acc = vectorcraft_geom::union_opt(acc, self.bounds_of(c));
                    }
                }
                acc
            }
            _ => brush_fx::cull_bounds(a),
        };
```

4. **Write TDD Integration Tests in `crates/render/tests/trace_render_tests.rs`**:
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

5. **Quality & Formatting Checks**:
- `cargo test -p vectorcraft-render --test trace_render_tests`
- `cargo clippy -p vectorcraft-render --all-targets -- -D warnings`
- `cargo fmt -p vectorcraft-render --check`
- Commit: `git commit -m "feat(render): implement multi-mode hairline and bitmap rendering for Image Trace"`
