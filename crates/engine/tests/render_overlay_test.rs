//! Integration tests for `render.overlay`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::json;
use vectorcraft_engine::Session;

#[test]
fn test_render_overlay_emits_valid_png() {
    let mut s = Session::headless();
    s.run("file.new", &json!({"width": 200, "height": 200})).unwrap();
    s.run("shape.rectangle", &json!({"x": 20, "y": 20, "width": 60, "height": 40})).unwrap();

    let temp_dir = vectorcraft_testkit::temp_dir("overlay_test");
    let out_path = temp_dir.join("test_overlay.png").to_string_lossy().to_string();

    let res = s.run("render.overlay", &json!({"path": out_path})).unwrap();
    assert_eq!(res["annotated_count"], 1);
    assert_eq!(res["dimensions"]["width"], 400);
    assert_eq!(res["dimensions"]["height"], 400);

    assert!(std::path::Path::new(&out_path).exists());
    let file_bytes = std::fs::read(&out_path).unwrap();
    assert!(file_bytes.len() >= 8 && &file_bytes[..8] == b"\x89PNG\r\n\x1a\n");

    let decoded = image::load_from_memory(&file_bytes).unwrap();
    assert_eq!(decoded.width(), 400);
    assert_eq!(decoded.height(), 400);

    let _ = std::fs::remove_file(&out_path);
}

#[test]
fn test_render_overlay_rejects_non_finite_scale() {
    let mut s = Session::headless();
    s.run("file.new", &json!({"width": 200, "height": 200})).unwrap();

    assert!(s.run("render.overlay", &json!({"path": "dummy.png", "scale": -1.0})).is_err());
    assert!(s.run("render.overlay", &json!({"path": "dummy.png", "scale": 0.0})).is_err());
    assert!(s.run("render.overlay", &json!({"path": "dummy.png", "scale": "NaN"})).is_err());
}

#[test]
fn test_render_overlay_missing_path() {
    let mut s = Session::headless();
    s.run("file.new", &json!({"width": 200, "height": 200})).unwrap();

    assert!(s.run("render.overlay", &json!({})).is_err());
}

#[test]
fn test_render_overlay_node_ids_filter() {
    let mut s = Session::headless();
    s.run("file.new", &json!({"width": 200, "height": 200})).unwrap();

    let r1 = s.run("shape.rectangle", &json!({"x": 10, "y": 10, "width": 40, "height": 40})).unwrap()["id"].as_u64().unwrap();
    let _r2 = s.run("shape.rectangle", &json!({"x": 80, "y": 80, "width": 40, "height": 40})).unwrap()["id"].as_u64().unwrap();

    let temp_dir = vectorcraft_testkit::temp_dir("overlay_test");
    let out_path = temp_dir.join("test_filter.png").to_string_lossy().to_string();

    let res = s.run("render.overlay", &json!({"path": out_path, "node_ids": [r1]})).unwrap();
    assert_eq!(res["annotated_count"], 1);

    let _ = std::fs::remove_file(&out_path);
}

#[test]
fn test_render_overlay_ungrouped_only() {
    let mut s = Session::headless();
    s.run("file.new", &json!({"width": 200, "height": 200})).unwrap();

    let _r1 = s.run("shape.rectangle", &json!({"x": 10, "y": 10, "width": 30, "height": 30})).unwrap()["id"].as_u64().unwrap();
    let r2 = s.run("shape.rectangle", &json!({"x": 50, "y": 50, "width": 30, "height": 30})).unwrap()["id"].as_u64().unwrap();
    s.run("select.set", &json!({"ids": [r2]})).unwrap();
    s.run("object.group", &json!({})).unwrap();

    let temp_dir = vectorcraft_testkit::temp_dir("overlay_test");
    let out_path = temp_dir.join("test_ungrouped.png").to_string_lossy().to_string();

    let res = s.run("render.overlay", &json!({"path": out_path, "ungrouped_only": true})).unwrap();
    assert_eq!(res["annotated_count"], 1);

    let _ = std::fs::remove_file(&out_path);
}
