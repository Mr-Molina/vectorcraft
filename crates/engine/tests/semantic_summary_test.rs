//! Integration tests for `document.semantic_summary`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use serde_json::{Value, json};
use vectorcraft_doc::{Node, NodeId};
use vectorcraft_engine::{EngineError, Session};

fn summary(s: &mut Session, p: Value) -> Value {
    s.run("document.semantic_summary", &p).unwrap()
}

/// A rectangle with fill `color` (nothing selected when the paint changes, so no other object
/// is recoloured). Returns its id.
fn rect(s: &mut Session, color: &str, x: f64) -> u64 {
    s.run("select.set", &json!({"ids": []})).unwrap();
    s.run("paint.setFill", &json!({"color": color})).unwrap();
    s.run("shape.rectangle", &json!({"x": x, "y": 10, "width": 20, "height": 20})).unwrap()["id"].as_u64().unwrap()
}

fn fill_label(s: &mut Session, id: u64) -> Value {
    s.run("document.node", &json!({"id": id, "summary": true})).unwrap()["fill"].clone()
}

#[test]
fn semantic_summary_basic_structure() {
    let mut s = Session::headless();
    s.run("file.new", &json!({"width": 500, "height": 500})).unwrap();
    s.run("shape.rectangle", &json!({"x": 10, "y": 10, "width": 50, "height": 50})).unwrap();
    let summary = summary(&mut s, json!({}));
    assert_eq!(summary["artboardCount"], 1);
    assert_eq!(summary.get("layerCount").unwrap_or(&json!(1)), 1);
    assert_eq!(summary["totalNodes"], 1);
    assert_eq!(summary["groupCount"], 0);
    assert_eq!(summary["loosePathCount"], 1);
    assert_eq!(summary["hiddenCount"], 0);
}

#[test]
fn semantic_summary_empty_document() {
    let mut s = Session::headless();
    s.run("file.new", &json!({"width": 500, "height": 500})).unwrap();
    let summary = summary(&mut s, json!({}));
    assert_eq!(summary["artboardCount"], 1);
    assert_eq!(summary.get("layerCount").unwrap_or(&json!(1)), 1);
    assert_eq!(summary["totalNodes"], 0);
    assert_eq!(summary["groupCount"], 0);
    assert_eq!(summary["loosePathCount"], 0);
    assert_eq!(summary["hiddenCount"], 0);
    assert_eq!(summary["palette"], json!([]));
}

#[test]
fn test_semantic_summary_detects_background_and_palette() {
    let mut s = Session::headless();
    s.run("file.new", &json!({"width": 500, "height": 500})).unwrap();
    s.run("shape.rectangle", &json!({"x": 0, "y": 0, "width": 500, "height": 500})).unwrap();
    s.run("shape.ellipse", &json!({"x": 100, "y": 100, "width": 50, "height": 50})).unwrap();

    let summary = summary(&mut s, json!({}));
    assert_eq!(summary["totalNodes"], 2);
    assert_eq!(summary["loosePathCount"], 2);
    assert_eq!(summary["groupCount"], 0);
    assert_eq!(summary["hiddenCount"], 0);
    assert_eq!(summary["artboardCount"], 1);
    assert_eq!(summary["artboard"], json!({"width": 500.0, "height": 500.0}));
    assert_eq!(summary["depthCapped"], false);
    assert!(summary["backgroundCardId"].is_number());
    assert!(!summary["palette"].as_array().unwrap().is_empty());
}

#[test]
fn test_semantic_summary_artboard_divergence_fallback() {
    let mut s = Session::headless();
    s.run("file.new", &json!({"width": 300, "height": 300})).unwrap();
    s.run("shape.rectangle", &json!({"x": 10, "y": 10, "width": 50, "height": 50})).unwrap();
    let summary = summary(&mut s, json!({}));
    assert!(summary["backgroundCardId"].is_null());
    assert_eq!(summary["totalNodes"], 1);
}

#[test]
fn test_semantic_summary_selection_only() {
    let mut s = Session::headless();
    s.run("file.new", &json!({"width": 400, "height": 400})).unwrap();
    let _r1 = s.run("shape.rectangle", &json!({"x": 10, "y": 10, "width": 50, "height": 50})).unwrap();
    let r2 = s.run("shape.ellipse", &json!({"x": 100, "y": 100, "width": 50, "height": 50})).unwrap();
    s.run("select.set", &json!({"ids": [r2["id"]]})).unwrap();

    let summary = summary(&mut s, json!({"selectionOnly": true}));
    assert_eq!(summary["totalNodes"], 1);
    assert_eq!(summary["loosePathCount"], 1);
}

#[test]
fn test_semantic_summary_selection_only_walks_selected_subtree() {
    let mut s = Session::headless();
    s.run("file.new", &json!({"width": 400, "height": 400})).unwrap();
    let a = rect(&mut s, "#ff0000", 10.0);
    let b = rect(&mut s, "#ff0000", 40.0);
    let _c = rect(&mut s, "#0000ff", 70.0);
    s.run("select.set", &json!({"ids": [a, b]})).unwrap();
    let g = s.run("object.group", &json!({})).unwrap()["id"].as_u64().unwrap();
    s.run("select.set", &json!({"ids": [g]})).unwrap();

    let sel = summary(&mut s, json!({"selectionOnly": true}));
    assert_eq!(sel["groupCount"], 1);
    assert_eq!(sel["totalNodes"], 3);
    // Inside the selected group: not loose.
    assert_eq!(sel["loosePathCount"], 0);
    let pal = sel["palette"].as_array().unwrap();
    assert_eq!(pal.len(), 1, "{sel}");
    assert_eq!(pal[0]["count"], 2);

    let all = summary(&mut s, json!({}));
    assert_eq!(all["totalNodes"], 4);
    assert_eq!(all["loosePathCount"], 1);
}

#[test]
fn test_semantic_summary_counts_nested_groups() {
    let mut s = Session::headless();
    s.run("file.new", &json!({"width": 400, "height": 400})).unwrap();
    let r = rect(&mut s, "#ff0000", 10.0);
    s.run("select.set", &json!({"ids": [r]})).unwrap();
    let g1 = s.run("object.group", &json!({})).unwrap()["id"].as_u64().unwrap();
    s.run("select.set", &json!({"ids": [g1]})).unwrap();
    s.run("object.group", &json!({})).unwrap();

    let sum = summary(&mut s, json!({}));
    assert_eq!(sum["groupCount"], 2, "{sum}");
    assert_eq!(sum["loosePathCount"], 0, "a path inside a group is not loose: {sum}");
    assert_eq!(sum["totalNodes"], 3);
    // The nested path's fill is found.
    let pal = sum["palette"].as_array().unwrap();
    assert_eq!(pal.len(), 1);
    assert_eq!(pal[0]["sampleIds"], json!([r]));
}

#[test]
fn test_semantic_summary_palette_order_and_truncation() {
    let mut s = Session::headless();
    s.run("file.new", &json!({"width": 400, "height": 400})).unwrap();
    let mut x = 0.0;
    let mut next = |s: &mut Session, c: &str| {
        x += 25.0;
        rect(s, c, x)
    };
    let red = next(&mut s, "#ff0000");
    next(&mut s, "#ff0000");
    let blue = next(&mut s, "#0000ff");
    let green = next(&mut s, "#00ff00");
    next(&mut s, "#ff0000");
    next(&mut s, "#00ff00");
    let (red_l, green_l, blue_l) = (fill_label(&mut s, red), fill_label(&mut s, green), fill_label(&mut s, blue));
    assert!(red_l != green_l && green_l != blue_l && red_l != blue_l);

    let sum = summary(&mut s, json!({}));
    let pal = sum["palette"].as_array().unwrap();
    let got: Vec<(Value, Value)> = pal.iter().map(|e| (e["hex"].clone(), e["count"].clone())).collect();
    assert_eq!(got, vec![(red_l.clone(), json!(3)), (green_l.clone(), json!(2)), (blue_l, json!(1))]);

    let sum = summary(&mut s, json!({"maxPalette": 2}));
    let pal = sum["palette"].as_array().unwrap();
    assert_eq!(pal.len(), 2);
    assert_eq!(pal[0]["hex"], red_l);
    assert_eq!(pal[1]["hex"], green_l);

    assert!(summary(&mut s, json!({"maxPalette": 0}))["palette"].as_array().unwrap().is_empty());
    // Above the cap is clamped, not an error.
    assert_eq!(summary(&mut s, json!({"maxPalette": 100000}))["palette"].as_array().unwrap().len(), 3);
}

#[test]
fn test_semantic_summary_palette_ties_by_hex() {
    let mut s = Session::headless();
    s.run("file.new", &json!({"width": 400, "height": 400})).unwrap();
    rect(&mut s, "#00ff00", 10.0);
    rect(&mut s, "#ff0000", 40.0);
    rect(&mut s, "#0000ff", 70.0);
    let sum = summary(&mut s, json!({}));
    let hexes: Vec<String> = sum["palette"].as_array().unwrap().iter().map(|e| e["hex"].as_str().unwrap().to_string()).collect();
    let mut sorted = hexes.clone();
    sorted.sort();
    assert_eq!(hexes, sorted);
}

#[test]
fn test_semantic_summary_sample_ids_capped() {
    let mut s = Session::headless();
    s.run("file.new", &json!({"width": 400, "height": 400})).unwrap();
    let ids: Vec<u64> = (0..8).map(|i| rect(&mut s, "#ff0000", 10.0 + 30.0 * f64::from(i))).collect();
    let sum = summary(&mut s, json!({}));
    let e = &sum["palette"][0];
    assert_eq!(e["count"], 8);
    assert_eq!(e["sampleIds"], json!(ids[..5]));
}

#[test]
fn test_semantic_summary_hidden_object_excluded() {
    let mut s = Session::headless();
    s.run("file.new", &json!({"width": 400, "height": 400})).unwrap();
    rect(&mut s, "#ff0000", 10.0);
    let b = rect(&mut s, "#0000ff", 40.0);
    s.run("select.set", &json!({"ids": [b]})).unwrap();
    s.run("object.hide", &json!({})).unwrap();

    let sum = summary(&mut s, json!({}));
    assert_eq!(sum["hiddenCount"], 1, "{sum}");
    assert_eq!(sum["totalNodes"], 1);
    assert_eq!(sum["loosePathCount"], 1);
    assert_eq!(sum["palette"].as_array().unwrap().len(), 1);
}

#[test]
fn test_semantic_summary_hidden_layer_excluded() {
    let mut s = Session::headless();
    s.run("file.new", &json!({"width": 400, "height": 400})).unwrap();
    let r = rect(&mut s, "#ff0000", 10.0);
    s.run("select.set", &json!({"ids": [r]})).unwrap();
    s.run("object.group", &json!({})).unwrap();
    let st = s.doc_mut().unwrap();
    let doc = Arc::make_mut(&mut st.doc);
    let layer = doc.layers[0].id;
    doc.node_mut(layer).unwrap().visible = false;

    let sum = summary(&mut s, json!({}));
    assert_eq!(sum["hiddenCount"], 1, "the hidden layer counts once, its subtree is skipped: {sum}");
    assert_eq!(sum["totalNodes"], 0);
    assert_eq!(sum["groupCount"], 0);
    assert_eq!(sum["loosePathCount"], 0);
    assert!(sum["palette"].as_array().unwrap().is_empty());
}

#[test]
fn test_semantic_summary_malformed_params() {
    let mut s = Session::headless();
    s.run("file.new", &json!({"width": 400, "height": 400})).unwrap();
    for p in [
        json!({"selectionOnly": "yes"}),
        json!({"selectionOnly": 1}),
        json!({"maxPalette": -1}),
        json!({"maxPalette": 2.5}),
        json!({"maxPalette": "8"}),
    ] {
        let r = s.run("document.semantic_summary", &p);
        assert!(matches!(r, Err(EngineError::BadParams { .. })), "{p}: {r:?}");
    }
    // Explicit nulls mean "absent".
    s.run("document.semantic_summary", &json!({"selectionOnly": null, "maxPalette": null})).unwrap();
}

#[test]
fn test_semantic_summary_deep_nesting_is_capped() {
    let mut s = Session::headless();
    s.run("file.new", &json!({"width": 400, "height": 400})).unwrap();
    rect(&mut s, "#ff0000", 10.0);
    {
        let st = s.doc_mut().unwrap();
        let doc = Arc::make_mut(&mut st.doc);
        let mut node: Arc<Node> = {
            let layer = Arc::make_mut(&mut doc.layers[0]);
            layer.children_mut().unwrap().pop().unwrap()
        };
        for _ in 0..2000 {
            let id: NodeId = doc.alloc_id();
            node = Arc::new(Node::group(id, vec![node]));
        }
        Arc::make_mut(&mut doc.layers[0]).children_mut().unwrap().push(node);
    }
    let sum = summary(&mut s, json!({}));
    assert_eq!(sum["depthCapped"], true, "{sum}");
    // Groups at depths 1..=512 under the layer are walked.
    assert_eq!(sum["groupCount"], 512);
    assert_eq!(sum["loosePathCount"], 0);
}

#[test]
fn test_semantic_summary_is_a_query() {
    let mut s = Session::headless();
    s.run("file.new", &json!({"width": 400, "height": 400})).unwrap();
    rect(&mut s, "#ff0000", 10.0);
    let (undo, rev) = {
        let st = s.doc().unwrap();
        (st.history.undo.len(), st.revision)
    };
    summary(&mut s, json!({}));
    summary(&mut s, json!({"selectionOnly": true, "maxPalette": 3}));
    let st = s.doc().unwrap();
    assert_eq!(st.history.undo.len(), undo);
    assert_eq!(st.revision, rev);
}
