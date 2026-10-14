//! Integration tests for `render.overlay`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::{Value, json};
use vectorcraft_doc::NodeId;
use vectorcraft_engine::Session;

fn session() -> Session {
    let mut s = Session::headless();
    s.run("file.new", &json!({"width": 200, "height": 200})).unwrap();
    s
}

fn rect(s: &mut Session, x: f64, y: f64, w: f64, h: f64) -> u64 {
    s.run("shape.rectangle", &json!({"x": x, "y": y, "width": w, "height": h})).unwrap()["id"].as_u64().unwrap()
}

fn undo_depth(s: &Session) -> usize {
    s.doc().unwrap().history.undo.len()
}

fn doc_json(s: &mut Session) -> Value {
    s.run("document.json", &json!({})).unwrap()
}

/// Two rectangles, the second grouped: returns (loose rect, grouped rect, group).
fn grouped_scene(s: &mut Session) -> (u64, u64, u64) {
    let r1 = rect(s, 10.0, 10.0, 30.0, 30.0);
    let r2 = rect(s, 50.0, 50.0, 30.0, 30.0);
    s.run("select.set", &json!({"ids": [r2]})).unwrap();
    s.run("object.group", &json!({})).unwrap();
    let g = s.doc().unwrap().doc.parent_of(NodeId(r2)).unwrap();
    assert!(!s.doc().unwrap().doc.node(g).unwrap().is_layer(), "r2 should be inside a group");
    (r1, r2, g.0)
}

#[test]
fn emits_valid_png_in_memory() {
    let mut s = session();
    rect(&mut s, 20.0, 20.0, 60.0, 40.0);
    let res = s.run("render.overlay", &json!({})).unwrap();
    assert_eq!(res["annotatedCount"], 1);
    assert_eq!(res["width"], 200);
    assert_eq!(res["height"], 200);
    assert!(res["path"].is_null());
    let bytes = vectorcraft_format::base64_decode(res["pngBase64"].as_str().unwrap()).unwrap();
    assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    let img = image::load_from_memory(&bytes).unwrap();
    assert_eq!((img.width(), img.height()), (200, 200));

    let res = s.run("render.overlay", &json!({"scale": 2.0})).unwrap();
    let img = image::load_from_memory(&vectorcraft_format::base64_decode(res["pngBase64"].as_str().unwrap()).unwrap()).unwrap();
    assert_eq!((res["width"].as_u64(), res["height"].as_u64()), (Some(400), Some(400)));
    assert_eq!((img.width(), img.height()), (400, 400));
}

#[test]
fn no_file_written_without_path() {
    let dir = vectorcraft_testkit::temp_dir("overlay-nopath");
    let before: Vec<_> = std::fs::read_dir(&dir).unwrap().collect();
    let cwd_before: Vec<_> = std::fs::read_dir(".").unwrap().filter_map(|e| e.ok()).map(|e| e.file_name()).collect();
    let mut s = session();
    rect(&mut s, 20.0, 20.0, 60.0, 40.0);
    s.run("render.overlay", &json!({})).unwrap();
    let after: Vec<_> = std::fs::read_dir(&dir).unwrap().collect();
    let cwd_after: Vec<_> = std::fs::read_dir(".").unwrap().filter_map(|e| e.ok()).map(|e| e.file_name()).collect();
    assert_eq!(before.len(), after.len());
    assert_eq!(cwd_before.len(), cwd_after.len());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn writes_file_when_path_given() {
    let dir = vectorcraft_testkit::temp_dir("overlay-path");
    let out = dir.join("overlay.png").to_string_lossy().to_string();
    let mut s = session();
    rect(&mut s, 20.0, 20.0, 60.0, 40.0);
    let res = s.run("render.overlay", &json!({"path": out})).unwrap();
    assert_eq!(res["path"], out.as_str());
    let file = std::fs::read(&out).unwrap();
    let b64 = vectorcraft_format::base64_decode(res["pngBase64"].as_str().unwrap()).unwrap();
    assert_eq!(file, b64);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn render_overlay_writes_file_to_path() {
    let dir = vectorcraft_testkit::temp_dir("overlay-path-qa");
    let out = dir.join("overlay_qa.png").to_string_lossy().to_string();
    let mut s = session();
    rect(&mut s, 20.0, 20.0, 60.0, 40.0);
    let res = s.run("render.overlay", &json!({"path": out})).unwrap();
    assert_eq!(res["path"], out.as_str());
    let file = std::fs::read(&out).unwrap();
    let b64 = vectorcraft_format::base64_decode(res["pngBase64"].as_str().unwrap()).unwrap();
    assert_eq!(file, b64);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn huge_scale_rejected_and_document_unchanged() {
    let mut s = session();
    rect(&mut s, 20.0, 20.0, 60.0, 40.0);
    let before = doc_json(&mut s);
    let depth = undo_depth(&s);
    assert!(s.run("render.overlay", &json!({"scale": 1e6})).is_err());
    assert_eq!(doc_json(&mut s), before);
    assert_eq!(undo_depth(&s), depth);
}

#[test]
fn bad_scale_rejected() {
    let mut s = session();
    for raw in [r#"{"scale": -1}"#, r#"{"scale": 0}"#, r#"{"scale": 0.0}"#, r#"{"scale": "NaN"}"#, r#"{"scale": "2"}"#, r#"{"scale": 1e400}"#] {
        // serde_json parses 1e400 as an error or infinity depending on features; skip if it fails to parse.
        let Ok(p) = serde_json::from_str::<Value>(raw) else { continue };
        assert!(s.run("render.overlay", &p).is_err(), "{raw} should be rejected");
    }
}

#[test]
fn malformed_params_rejected() {
    let mut s = session();
    let r1 = rect(&mut s, 20.0, 20.0, 60.0, 40.0);
    for p in [
        json!({"ids": "5"}),
        json!({"ids": ["a"]}),
        json!({"ids": [-1]}),
        json!({"ids": [1.5]}),
        json!({"ids": [r1, "x"]}),
        json!({"ungroupedOnly": "yes"}),
        json!({"artboard": "0"}),
        json!({"artboard": -1}),
        json!({"path": 5}),
        json!({"path": ""}),
    ] {
        assert!(s.run("render.overlay", &p).is_err(), "{p} should be rejected");
    }
    let too_many: Vec<u64> = (0..100_001).collect();
    assert!(s.run("render.overlay", &json!({"ids": too_many})).is_err());
}

#[test]
fn ids_filter_dedupes_and_skips_unknown() {
    let mut s = session();
    let r1 = rect(&mut s, 10.0, 10.0, 40.0, 40.0);
    let _r2 = rect(&mut s, 80.0, 80.0, 40.0, 40.0);
    let res = s.run("render.overlay", &json!({"ids": [r1, r1, 999_999]})).unwrap();
    assert_eq!(res["annotatedCount"], 1);
    let res = s.run("render.overlay", &json!({"ids": []})).unwrap();
    assert_eq!(res["annotatedCount"], 0);
}

#[test]
fn nested_id_inside_group_is_annotated() {
    let mut s = session();
    let (_r1, r2, _g) = grouped_scene(&mut s);
    let res = s.run("render.overlay", &json!({"ids": [r2]})).unwrap();
    assert_eq!(res["annotatedCount"], 1);
    // The box is drawn at the child's world position (50,50)-(80,80): the tint changes the
    // pixel just inside its corner compared with a plain render of the same art.
    let png = vectorcraft_format::base64_decode(res["pngBase64"].as_str().unwrap()).unwrap();
    let img = image::load_from_memory(&png).unwrap().to_rgba8();
    let plain = s.run("render.overlay", &json!({"ids": []})).unwrap();
    let plain = image::load_from_memory(&vectorcraft_format::base64_decode(plain["pngBase64"].as_str().unwrap()).unwrap()).unwrap().to_rgba8();
    assert_ne!(img.get_pixel(78, 78), plain.get_pixel(78, 78));
    assert_eq!(img.get_pixel(150, 150), plain.get_pixel(150, 150));
}

#[test]
fn default_targets_are_top_level_objects() {
    let mut s = session();
    let (_r1, _r2, _g) = grouped_scene(&mut s);
    // The loose rectangle and the group, not the group's child.
    let res = s.run("render.overlay", &json!({})).unwrap();
    assert_eq!(res["annotatedCount"], 2);
}

#[test]
fn ungrouped_only_excludes_group_children() {
    let mut s = session();
    let (r1, r2, g) = grouped_scene(&mut s);
    let res = s.run("render.overlay", &json!({"ids": [r1, r2, g], "ungroupedOnly": true})).unwrap();
    assert_eq!(res["annotatedCount"], 1, "only the loose rectangle: groups and their children are excluded");
    let res = s.run("render.overlay", &json!({"ids": [r2], "ungroupedOnly": true})).unwrap();
    assert_eq!(res["annotatedCount"], 0);
    // Default targets too: the group is dropped, the loose rectangle stays.
    let res = s.run("render.overlay", &json!({"ungroupedOnly": true})).unwrap();
    assert_eq!(res["annotatedCount"], 1);
}

#[test]
fn hidden_objects_skipped_by_default() {
    let mut s = session();
    let r1 = rect(&mut s, 10.0, 10.0, 30.0, 30.0);
    rect(&mut s, 50.0, 50.0, 30.0, 30.0);
    s.run("select.set", &json!({"ids": [r1]})).unwrap();
    s.run("object.hide", &json!({})).unwrap();
    let res = s.run("render.overlay", &json!({})).unwrap();
    assert_eq!(res["annotatedCount"], 1);
}

#[test]
fn out_of_range_artboard_rejected() {
    let mut s = session();
    assert!(s.run("render.overlay", &json!({"artboard": 1})).is_err());
    assert!(s.run("render.overlay", &json!({"artboard": 0})).is_ok());
}

#[test]
fn is_a_query_without_undo_step() {
    let mut s = session();
    rect(&mut s, 20.0, 20.0, 60.0, 40.0);
    let depth = undo_depth(&s);
    let can_undo_before = vectorcraft_engine::cmd::can_undo(&s).is_ok();
    s.run("render.overlay", &json!({"scale": 1.5})).unwrap();
    assert_eq!(undo_depth(&s), depth);
    assert_eq!(vectorcraft_engine::cmd::can_undo(&s).is_ok(), can_undo_before);
}

fn set_artboard(s: &mut Session, r: vectorcraft_geom::Rect) {
    let st = s.doc_mut().unwrap();
    std::sync::Arc::make_mut(&mut st.doc).artboards[0].rect = r;
}

#[test]
fn size_check_uses_renderer_rounding() {
    let mut s = session();
    // 7999.5001 x 8000.5001 rounds to 8000 x 8001 px = 64,008,000 > 64e6.
    set_artboard(&mut s, vectorcraft_geom::Rect::new(0.0, 0.0, 7999.5001, 8000.5001));
    let err = s.run("render.overlay", &json!({})).unwrap_err().to_string();
    assert!(err.contains("too large"), "{err}");
    // Just inside the limit: 8000 x 8000 px is allowed by the check (not rendered here).
}

#[test]
fn max_raster_side_limit_is_32768() {
    let mut s = session();
    // 32_769 px side exceeds MAX_RASTER_SIDE (32_768) even with area well within 64M px (32_769 x 1).
    set_artboard(&mut s, vectorcraft_geom::Rect::new(0.0, 0.0, 32_769.0, 1.0));
    let err = s.run("render.overlay", &json!({})).unwrap_err().to_string();
    assert!(err.contains("too large"), "{err}");

    // 65_535 px side also exceeds MAX_RASTER_SIDE (32_768).
    set_artboard(&mut s, vectorcraft_geom::Rect::new(0.0, 0.0, 65_535.0, 1.0));
    let err = s.run("render.overlay", &json!({})).unwrap_err().to_string();
    assert!(err.contains("too large"), "{err}");
}

#[test]
fn inverted_or_overflowing_artboard_rejected() {
    let mut s = session();
    for r in [
        vectorcraft_geom::Rect::new(100_000.0, 0.0, 0.0, 10.0),
        vectorcraft_geom::Rect::new(0.0, 10.0, 10.0, 0.0),
        vectorcraft_geom::Rect::new(1.7e308, 0.0, -1.7e308, 10.0),
        vectorcraft_geom::Rect::new(-1.7e308, 0.0, 1.7e308, 10.0),
        vectorcraft_geom::Rect::new(0.0, 0.0, 0.0, 10.0),
    ] {
        set_artboard(&mut s, r);
        assert!(s.run("render.overlay", &json!({})).is_err(), "{r:?} should be rejected");
    }
}

#[test]
fn off_artboard_objects_not_counted() {
    let mut s = session();
    let on = rect(&mut s, 20.0, 20.0, 40.0, 40.0);
    let off = rect(&mut s, 500.0, 500.0, 40.0, 40.0);
    let left = rect(&mut s, -300.0, 20.0, 40.0, 40.0);
    let res = s.run("render.overlay", &json!({})).unwrap();
    assert_eq!(res["annotatedCount"], 1);
    let res = s.run("render.overlay", &json!({"ids": [on, off, left]})).unwrap();
    assert_eq!(res["annotatedCount"], 1);
    // Partly visible boxes still count.
    rect(&mut s, 180.0, 180.0, 40.0, 40.0);
    let res = s.run("render.overlay", &json!({})).unwrap();
    assert_eq!(res["annotatedCount"], 2);
}
