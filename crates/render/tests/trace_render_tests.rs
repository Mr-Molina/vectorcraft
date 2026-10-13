//! Integration tests for Image Trace multi-mode hairline and bitmap rendering.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;
use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::{Appearance, Document, Node, NodeId, NodeKind, TraceView};
use vectorcraft_geom::{Affine, Rect, Shape};
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

    let mut img_node = Node::new(
        NodeId(10),
        NodeKind::Image(vectorcraft_doc::ImageObject {
            key: "test_key".into(),
            xf: Affine::IDENTITY,
            width: 100,
            height: 100,
            link: None,
            placement: Default::default(),
        }),
    );
    img_node.name = Some("Source Image".into());
    img_node.visible = false;

    let rect_path = vectorcraft_geom::PathData::from_bezpath(&Rect::new(10.0, 10.0, 90.0, 90.0).to_path(0.1));
    let mut rect_node = Node::path(NodeId(11), rect_path, Appearance::default());
    rect_node.appearance.set_fill(Paint::solid(Color::rgb8(0, 0, 255)));

    let mut trace_group = Node::group(NodeId(1), vec![Arc::new(img_node), Arc::new(rect_node)]);
    trace_group.name = Some("Image Trace".into());

    let opts = RenderOptions { outline: false, background: Some([255, 255, 255, 255]), artboards: true, ..Default::default() };

    for view in
        [TraceView::TracingResult, TraceView::TracingResultWithOutlines, TraceView::Outlines, TraceView::OutlinesWithSource, TraceView::SourceImage]
    {
        trace_group.set_trace_view(view);
        let mut test_doc = doc.clone();
        test_doc.insert(None, 0, trace_group.clone()).unwrap();
        let rendered = r.render(&test_doc, 200, 200, Affine::IDENTITY, &opts);
        assert_eq!(rendered.width, 200);
        assert_eq!(rendered.height, 200);
        assert!(rendered.pixels.iter().any(|&b| b != 0));

        let px_at = |x: usize, y: usize| -> [u8; 4] {
            let idx = (y * 200 + x) * 4;
            [rendered.pixels[idx], rendered.pixels[idx + 1], rendered.pixels[idx + 2], rendered.pixels[idx + 3]]
        };

        // Differentiate mode behaviors at center (50, 50):
        let center = px_at(50, 50);
        match view {
            TraceView::TracingResult => {
                // Vector path fill is blue: [0, 0, 255, 255]
                assert_eq!(center, [0, 0, 255, 255], "TracingResult should show vector blue fill");
            }
            TraceView::TracingResultWithOutlines => {
                // Vector path fill is blue at center: [0, 0, 255, 255]
                assert_eq!(center, [0, 0, 255, 255], "TracingResultWithOutlines should show vector blue fill");
                // On the border at (10, 50), hairline [0, 160, 255, 255] is painted
                let edge = px_at(10, 50);
                assert!(edge[1] > 50, "TracingResultWithOutlines edge should have cyan green component, got {edge:?}");
            }
            TraceView::Outlines => {
                // Outlines only: interior is white background [255, 255, 255, 255]
                assert_eq!(center, [255, 255, 255, 255], "Outlines should show background at center");
                // On the border at (10, 50), black hairline [0, 0, 0, 255] antialiased over white background
                let edge = px_at(10, 50);
                assert!(edge[0] < 200 && edge[1] < 200 && edge[2] < 200, "Outlines edge should be darkened by black hairline, got {edge:?}");
            }
            TraceView::OutlinesWithSource => {
                // Source image is red: [255, 0, 0, 255]
                assert_eq!(center, [255, 0, 0, 255], "OutlinesWithSource should show red image at center");
                // On the border at (10, 50), hairline [0, 160, 255, 255] is painted
                let edge = px_at(10, 50);
                assert!(edge[1] > 50 || edge[2] > 50, "OutlinesWithSource edge should have cyan component, got {edge:?}");
            }
            TraceView::SourceImage => {
                // Source image is red: [255, 0, 0, 255]
                assert_eq!(center, [255, 0, 0, 255], "SourceImage should show red image at center");
            }
        }
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

#[test]
fn test_render_trace_group_source_image_bounds_and_visibility() {
    let mut doc = Document::new(200.0, 200.0);
    let mut r = Renderer::new();

    // 10x10 green image positioned at (0, 0)
    let img = image::RgbaImage::from_pixel(10, 10, image::Rgba([0, 255, 0, 255]));
    let mut png_bytes = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png).unwrap();
    doc.images.insert("green_img".into(), vectorcraft_doc::ImageBlob::new("image/png", png_bytes));

    let mut img_node = Node::new(
        NodeId(10),
        NodeKind::Image(vectorcraft_doc::ImageObject {
            key: "green_img".into(),
            xf: Affine::IDENTITY,
            width: 50,
            height: 50,
            link: None,
            placement: Default::default(),
        }),
    );
    img_node.visible = false;

    // Vector path at (100, 100) -> (150, 150)
    let rect_path = vectorcraft_geom::PathData::from_bezpath(&Rect::new(100.0, 100.0, 150.0, 150.0).to_path(0.1));
    let mut rect_node = Node::path(NodeId(11), rect_path, Appearance::default());
    rect_node.appearance.set_fill(Paint::solid(Color::rgb8(0, 0, 255)));

    let mut trace_group = Node::group(NodeId(1), vec![Arc::new(img_node), Arc::new(rect_node)]);
    trace_group.name = Some("Image Trace".into());
    trace_group.set_trace_view(TraceView::SourceImage);

    // Enclosing group
    let parent_group = Node::group(NodeId(2), vec![Arc::new(trace_group)]);
    doc.insert(None, 0, parent_group).unwrap();

    let opts = RenderOptions { background: Some([255, 255, 255, 255]), ..Default::default() };
    let rendered = r.render(&doc, 200, 200, Affine::IDENTITY, &opts);

    // Green image should be visible at (25, 25)
    let idx = (25 * 200 + 25) * 4;
    let px = [rendered.pixels[idx], rendered.pixels[idx + 1], rendered.pixels[idx + 2], rendered.pixels[idx + 3]];
    assert_eq!(px, [0, 255, 0, 255], "SourceImage inside parent group should render green image even though node.visible is false");

    // Vector path should NOT be rendered in SourceImage mode
    let idx_vec = (125 * 200 + 125) * 4;
    let px_vec = [rendered.pixels[idx_vec], rendered.pixels[idx_vec + 1], rendered.pixels[idx_vec + 2], rendered.pixels[idx_vec + 3]];
    assert_eq!(px_vec, [255, 255, 255, 255], "Vector path should not be drawn in SourceImage mode");
}
