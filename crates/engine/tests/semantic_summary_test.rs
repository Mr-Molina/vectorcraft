//! Integration tests for `document.semantic_summary`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::json;
use vectorcraft_engine::Session;

#[test]
fn test_semantic_summary_detects_background_and_palette() {
    let mut s = Session::headless();
    s.run("file.new", &json!({"width": 500, "height": 500})).unwrap();
    s.run("shape.rectangle", &json!({"x": 0, "y": 0, "width": 500, "height": 500})).unwrap();
    s.run("shape.ellipse", &json!({"x": 100, "y": 100, "width": 50, "height": 50})).unwrap();

    let summary = s.run("document.semantic_summary", &json!({})).unwrap();
    assert_eq!(summary["total_nodes"], 2);
    assert!(summary["background_card_id"].is_number());
    assert!(!summary["palette"].as_array().unwrap().is_empty());
}

#[test]
fn test_semantic_summary_artboard_divergence_fallback() {
    let mut s = Session::headless();
    s.run("file.new", &json!({"width": 300, "height": 300})).unwrap();
    s.run("shape.rectangle", &json!({"x": 10, "y": 10, "width": 50, "height": 50})).unwrap();
    let summary = s.run("document.semantic_summary", &json!({})).unwrap();
    assert!(summary["background_card_id"].is_null());
    assert_eq!(summary["total_nodes"], 1);
}

#[test]
fn test_semantic_summary_selection_only() {
    let mut s = Session::headless();
    s.run("file.new", &json!({"width": 400, "height": 400})).unwrap();
    let _r1 = s.run("shape.rectangle", &json!({"x": 10, "y": 10, "width": 50, "height": 50})).unwrap();
    let r2 = s.run("shape.ellipse", &json!({"x": 100, "y": 100, "width": 50, "height": 50})).unwrap();
    s.run("select.set", &json!({"ids": [r2["id"]]})).unwrap();

    let summary = s.run("document.semantic_summary", &json!({"selection_only": true})).unwrap();
    assert_eq!(summary["total_nodes"], 1);
}
