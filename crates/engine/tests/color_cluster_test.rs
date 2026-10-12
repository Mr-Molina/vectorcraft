//! Integration tests for `color.cluster`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::json;
use vectorcraft_engine::Session;

#[test]
fn test_color_cluster_preview_does_not_mutate() {
    let mut s = Session::headless();
    s.run("file.new", &json!({"width": 200, "height": 200})).unwrap();
    s.run("shape.rectangle", &json!({"x": 10, "y": 10, "width": 20, "height": 20})).unwrap();
    let res = s.run("color.cluster", &json!({"max_delta_e": 5.0, "action": "preview"})).unwrap();
    assert_eq!(res["modified_node_count"], 0);
}

#[test]
fn test_color_cluster_rejects_invalid_floats() {
    let mut s = Session::headless();
    s.run("file.new", &json!({"width": 200, "height": 200})).unwrap();
    assert!(s.run("color.cluster", &json!({"max_delta_e": "NaN", "action": "preview"})).is_err());
    assert!(s.run("color.cluster", &json!({"max_delta_e": f64::INFINITY, "action": "preview"})).is_err());
}

#[test]
fn test_color_cluster_handles_missing_node_ids() {
    let mut s = Session::headless();
    s.run("file.new", &json!({"width": 200, "height": 200})).unwrap();
    let res = s.run("color.cluster", &json!({"node_ids": [999999], "max_delta_e": 5.0, "action": "preview"})).unwrap();
    assert_eq!(res["modified_node_count"], 0);
}

#[test]
fn test_color_cluster_skips_empty_color_nodes() {
    let mut s = Session::headless();
    s.run("file.new", &json!({"width": 200, "height": 200})).unwrap();
    let r1 = s.run("shape.rectangle", &json!({"x": 10, "y": 10, "width": 20, "height": 20})).unwrap()["id"].as_u64().unwrap();
    s.run("paint.setFill", &json!({"ids": [r1], "none": true})).unwrap();

    let res = s.run("color.cluster", &json!({"node_ids": [r1], "max_delta_e": 5.0, "action": "preview"})).unwrap();
    assert_eq!(res["modified_node_count"], 0);
    assert!(res["clusters"].as_array().unwrap().is_empty());
}

#[test]
fn test_color_cluster_atomic_rollback_on_mid_batch_failure() {
    let mut s = Session::headless();
    s.run("file.new", &json!({"width": 200, "height": 200})).unwrap();
    let r1 = s.run("shape.rectangle", &json!({"x": 10, "y": 10, "width": 20, "height": 20})).unwrap()["id"].as_u64().unwrap();
    let r2 = s.run("shape.rectangle", &json!({"x": 40, "y": 40, "width": 20, "height": 20})).unwrap()["id"].as_u64().unwrap();

    let pre_summary = s.run("document.semantic_summary", &json!({})).unwrap();

    let err = s.run("color.cluster", &json!({"node_ids": [r1, r2], "max_delta_e": 10.0, "action": "invalid_mode_rollback"}));
    assert!(err.is_err());

    let post_summary = s.run("document.semantic_summary", &json!({})).unwrap();
    assert_eq!(pre_summary, post_summary);
}

#[test]
fn test_color_cluster_group_creates_groups_atomically_with_undo() {
    let mut s = Session::headless();
    s.run("file.new", &json!({"width": 200, "height": 200})).unwrap();
    let r1 = s.run("shape.rectangle", &json!({"x": 10, "y": 10, "width": 20, "height": 20})).unwrap()["id"].as_u64().unwrap();
    let r2 = s.run("shape.rectangle", &json!({"x": 40, "y": 40, "width": 20, "height": 20})).unwrap()["id"].as_u64().unwrap();

    let res = s.run("color.cluster", &json!({"node_ids": [r1, r2], "max_delta_e": 10.0, "action": "group"})).unwrap();
    assert_eq!(res["modified_node_count"], 2);

    // Single undo restores initial ungrouped layout
    s.run("edit.undo", &json!({})).unwrap();
    let summary = s.run("document.semantic_summary", &json!({})).unwrap();
    assert_eq!(summary["group_count"], 0);
    assert_eq!(summary["loose_path_count"], 2);
}
