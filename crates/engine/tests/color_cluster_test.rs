//! Integration tests for `color.cluster` (read-only preview) and `edit.colors.groupByColor`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::{Value, json};
use vectorcraft_doc::NodeId;
use vectorcraft_engine::Session;

fn session() -> Session {
    let mut s = Session::headless();
    s.run("file.new", &json!({"width": 400, "height": 400})).unwrap();
    s
}

fn rect(s: &mut Session, x: f64, color: &str) -> u64 {
    let id = s.run("shape.rectangle", &json!({"x": x, "y": 10, "width": 10, "height": 10})).unwrap()["id"].as_u64().unwrap();
    s.run("paint.setFill", &json!({"ids": [id], "color": color})).unwrap();
    id
}

fn doc_json(s: &Session) -> String {
    serde_json::to_string(&*s.active().unwrap().doc).unwrap()
}

fn undo_len(s: &Session) -> usize {
    s.active().unwrap().history.undo.len()
}

fn ids_of(res: &Value) -> Vec<Vec<u64>> {
    res["clusters"].as_array().unwrap().iter().map(|c| c["ids"].as_array().unwrap().iter().map(|i| i.as_u64().unwrap()).collect()).collect()
}

fn skipped(res: &Value) -> Vec<(u64, String)> {
    res["skipped"].as_array().unwrap().iter().map(|e| (e["id"].as_u64().unwrap(), e["reason"].as_str().unwrap().to_string())).collect()
}

#[test]
fn preview_is_read_only_and_reports_clusters() {
    let mut s = session();
    let r1 = rect(&mut s, 10.0, "#ff0000");
    let r2 = rect(&mut s, 30.0, "#ff0101");
    let b = rect(&mut s, 50.0, "#0000ff");
    let (before, undo) = (doc_json(&s), undo_len(&s));
    let res = s.run("color.cluster", &json!({"maxDeltaE": 5.0, "ids": [b, r2, r1]})).unwrap();
    assert_eq!(doc_json(&s), before, "preview must not change the document");
    assert_eq!(undo_len(&s), undo, "preview must not add an undo step");
    assert_eq!(ids_of(&res), vec![vec![r1, r2], vec![b]], "near colours cluster, far ones don't; paint order");
    let c = &res["clusters"];
    assert_eq!(c[0]["hex"], "#ff0000");
    assert_eq!(c[0]["name"], "Color #ff0000");
    assert!(c[0]["avgDeltaE"].as_f64().unwrap() > 0.0);
    assert_eq!(c[1]["avgDeltaE"].as_f64().unwrap(), 0.0, "a single member has no spread");
    assert!(skipped(&res).is_empty());
}

#[test]
fn threshold_zero_is_exact_match() {
    let mut s = session();
    let a = rect(&mut s, 10.0, "#ff0000");
    let b = rect(&mut s, 30.0, "#fe0101");
    let c = rect(&mut s, 50.0, "#ff0000");
    let res = s.run("color.cluster", &json!({"maxDeltaE": 0, "ids": [a, b, c]})).unwrap();
    assert_eq!(ids_of(&res), vec![vec![a, c], vec![b]]);
    assert_eq!(res["clusters"][0]["avgDeltaE"].as_f64().unwrap(), 0.0);
}

#[test]
fn permutations_of_ids_give_identical_clusters() {
    let mut s = session();
    let colors = ["#ff0000", "#f80404", "#f00808", "#e80c0c", "#e01010", "#d81414", "#0000ff", "#0404f4"];
    let ids: Vec<u64> = colors.iter().enumerate().map(|(i, c)| rect(&mut s, 10.0 + 20.0 * i as f64, c)).collect();
    let base = s.run("color.cluster", &json!({"maxDeltaE": 3.0, "ids": ids})).unwrap();
    let mut rev = ids.clone();
    rev.reverse();
    let mut rot = ids.clone();
    rot.rotate_left(3);
    let mut swapped = ids.clone();
    swapped.swap(0, 5);
    swapped.swap(2, 7);
    for p in [rev, rot, swapped] {
        assert_eq!(s.run("color.cluster", &json!({"maxDeltaE": 3.0, "ids": p})).unwrap(), base, "{p:?}");
    }
}

#[test]
fn malformed_params_are_rejected() {
    let mut s = session();
    let r = rect(&mut s, 10.0, "#ff0000");
    for cmd in ["color.cluster", "edit.colors.groupByColor"] {
        for ids in [json!("5"), json!(["a"]), json!([-1]), json!([1.5]), json!({"0": r})] {
            assert!(s.run(cmd, &json!({"maxDeltaE": 5, "ids": ids})).is_err(), "{cmd} ids {ids}");
        }
        // NaN can't be written in JSON: serde_json makes it null, and a raw `NaN` doesn't parse.
        let nan = json!({"maxDeltaE": f64::NAN, "ids": [r]});
        assert!(nan["maxDeltaE"].is_null());
        assert!(serde_json::from_str::<Value>(r#"{"maxDeltaE": NaN}"#).is_err());
        for p in [
            nan,
            json!({"ids": [r]}),
            json!({"maxDeltaE": -1, "ids": [r]}),
            json!({"maxDeltaE": "NaN", "ids": [r]}),
            json!({"maxDeltaE": 1e9, "ids": [r]}),
            json!({"maxDeltaE": 200.001, "ids": [r]}),
        ] {
            assert!(s.run(cmd, &p).is_err(), "{cmd} {p}");
        }
    }
    assert!(s.run("color.cluster", &json!({"maxDeltaE": 200, "ids": [r]})).is_ok());
    assert!(s.run("color.cluster", &json!({"maxDeltaE": 0, "ids": [r]})).is_ok());
    let too_many: Vec<u64> = (0..100_001).collect();
    assert!(s.run("color.cluster", &json!({"maxDeltaE": 5, "ids": too_many})).is_err());
}

#[test]
fn default_scope_is_the_selection() {
    let mut s = session();
    let r1 = rect(&mut s, 10.0, "#ff0000");
    let r2 = rect(&mut s, 30.0, "#fe0101");
    let _other = rect(&mut s, 50.0, "#ff0000");
    s.run("select.set", &json!({"ids": [r1, r2]})).unwrap();
    let res = s.run("color.cluster", &json!({"maxDeltaE": 5})).unwrap();
    assert_eq!(ids_of(&res), vec![vec![r1, r2]]);
}

#[test]
fn duplicates_and_descendants_of_listed_objects_collapse() {
    let mut s = session();
    let r1 = rect(&mut s, 10.0, "#ff0000");
    let r2 = rect(&mut s, 30.0, "#ff0000");
    let res = s.run("color.cluster", &json!({"maxDeltaE": 5, "ids": [r1, r1, r2, r2]})).unwrap();
    assert_eq!(ids_of(&res), vec![vec![r1, r2]]);
    s.run("select.set", &json!({"ids": [r1, r2]})).unwrap();
    let g = s.run("object.group", &json!({})).unwrap()["id"].as_u64().unwrap();
    let res = s.run("color.cluster", &json!({"maxDeltaE": 5, "ids": [r1, g, r2]})).unwrap();
    assert!(ids_of(&res).is_empty(), "the group carries its members");
    assert_eq!(skipped(&res), vec![(g, "no fill".to_string())]);
}

#[test]
fn locked_gradient_and_missing_objects_are_skipped() {
    let mut s = session();
    let r1 = rect(&mut s, 10.0, "#ff0000");
    let locked = rect(&mut s, 30.0, "#ff0000");
    let grad = rect(&mut s, 50.0, "#ff0000");
    s.run("paint.setFill", &json!({"ids": [grad], "gradient": {}})).unwrap();
    s.run("select.set", &json!({"ids": [locked]})).unwrap();
    s.run("object.lock", &json!({})).unwrap();
    let res = s.run("color.cluster", &json!({"maxDeltaE": 5, "ids": [r1, locked, grad, 999_999]})).unwrap();
    assert_eq!(ids_of(&res), vec![vec![r1]]);
    let sk = skipped(&res);
    assert!(sk.contains(&(locked, "locked".into())), "{sk:?}");
    assert!(sk.contains(&(grad, "gradient".into())), "{sk:?}");
    assert!(sk.contains(&(999_999, "not found".into())), "{sk:?}");
    // Grouping leaves the locked object where it was.
    let r3 = rect(&mut s, 70.0, "#ff0000");
    let res = s.run("edit.colors.groupByColor", &json!({"maxDeltaE": 5, "ids": [r1, locked, r3]})).unwrap();
    assert_eq!(res["groups"].as_array().unwrap().len(), 1);
    let d = &s.active().unwrap().doc;
    let layer = d.position(NodeId(locked)).unwrap().0;
    assert!(d.node(layer.unwrap()).unwrap().is_layer(), "the locked object stays in its layer");
}

#[test]
fn group_by_color_groups_per_parent_and_undoes_in_one_step() {
    let mut s = session();
    let a = rect(&mut s, 10.0, "#ff0000");
    let blue = rect(&mut s, 30.0, "#0000ff");
    let c = rect(&mut s, 50.0, "#ff0101");
    let green = rect(&mut s, 70.0, "#00ff00");
    let layer1 = s.active().unwrap().doc.position(NodeId(a)).unwrap().0.unwrap();
    let layer2 = s.run("layer.new", &json!({"name": "Two"})).unwrap()["id"].as_u64().unwrap();
    let e = rect(&mut s, 90.0, "#ff0000");
    let f = rect(&mut s, 110.0, "#ff0101");
    assert_eq!(s.active().unwrap().doc.position(NodeId(e)).unwrap().0, Some(NodeId(layer2)));

    let ids = [f, green, a, e, blue, c];
    let preview = s.run("color.cluster", &json!({"maxDeltaE": 5, "ids": ids})).unwrap();
    let before = doc_json(&s);
    let undo = undo_len(&s);
    let res = s.run("edit.colors.groupByColor", &json!({"maxDeltaE": 5, "ids": ids})).unwrap();
    assert_eq!(res["clusters"], preview["clusters"], "preview and grouping agree, names included");
    assert_eq!(undo_len(&s), undo + 1, "one undo step");
    let groups: Vec<u64> = res["groups"].as_array().unwrap().iter().map(|g| g.as_u64().unwrap()).collect();
    assert_eq!(groups.len(), 2, "only clusters of two or more become groups: {res}");

    let st = s.active().unwrap();
    let d = &st.doc;
    let mut sel = st.selection.objects.iter().map(|i| i.0).collect::<Vec<_>>();
    sel.sort_unstable();
    let mut want = groups.clone();
    want.sort_unstable();
    assert_eq!(sel, want, "the new groups are selected");
    let kids = |g: u64| d.node(NodeId(g)).unwrap().children().unwrap().iter().map(|n| n.id.0).collect::<Vec<_>>();
    let parent = |id: u64| d.position(NodeId(id)).unwrap().0.unwrap().0;
    assert_eq!(kids(groups[0]), vec![a, c], "members keep their paint order");
    assert_eq!(kids(groups[1]), vec![e, f]);
    assert_eq!(parent(groups[0]), layer1.0, "never moved across layers");
    assert_eq!(parent(groups[1]), layer2);
    assert_eq!(d.node(NodeId(groups[0])).unwrap().name.as_deref(), Some("Color #ff0000"));
    assert_eq!(d.node(NodeId(groups[1])).unwrap().name.as_deref(), Some("Color #ff0000 2"), "names are unique");
    // The singletons stay where they were.
    assert_eq!(parent(blue), layer1.0);
    assert_eq!(parent(green), layer1.0);
    // The group sits where its front-most member was: above blue, below green.
    let idx = |id: u64| d.position(NodeId(id)).unwrap().1;
    assert!(idx(blue) < idx(groups[0]) && idx(groups[0]) < idx(green));

    let after = doc_json(&s);
    s.run("edit.undo", &json!({})).unwrap();
    assert_eq!(doc_json(&s), before, "one undo restores the exact document");
    s.run("edit.redo", &json!({})).unwrap();
    assert_eq!(doc_json(&s), after, "redo regroups");
}

#[test]
fn nothing_to_group_adds_no_undo_step() {
    let mut s = session();
    let a = rect(&mut s, 10.0, "#ff0000");
    let b = rect(&mut s, 30.0, "#0000ff");
    let undo = undo_len(&s);
    let res = s.run("edit.colors.groupByColor", &json!({"maxDeltaE": 5, "ids": [a, b]})).unwrap();
    assert_eq!(res["groups"], json!([]));
    assert_eq!(undo_len(&s), undo);
}

#[test]
fn group_by_color_takes_ids_without_a_selection() {
    let mut s = session();
    let a = rect(&mut s, 10.0, "#ff0000");
    let b = rect(&mut s, 30.0, "#ff0101");
    s.run("select.none", &json!({})).unwrap();
    assert!(s.active().unwrap().selection.objects.is_empty());
    let err = s.run("edit.colors.groupByColor", &json!({"maxDeltaE": 5})).unwrap_err();
    assert!(err.to_string().contains("ids"), "a clear message: {err}");
    let res = s.run("edit.colors.groupByColor", &json!({"maxDeltaE": 5, "ids": [a, b]})).unwrap();
    let groups = res["groups"].as_array().unwrap();
    assert_eq!(groups.len(), 1, "{res}");
    let g = NodeId(groups[0].as_u64().unwrap());
    let kids: Vec<u64> = s.active().unwrap().doc.node(g).unwrap().children().unwrap().iter().map(|n| n.id.0).collect();
    assert_eq!(kids, vec![a, b]);
}

#[test]
fn art_nested_inside_a_live_blend_is_skipped() {
    let mut s = session();
    let r1 = rect(&mut s, 10.0, "#ff0000");
    let r2 = rect(&mut s, 30.0, "#ff0000");
    s.run("select.set", &json!({"ids": [r1, r2]})).unwrap();
    let ga = s.run("object.group", &json!({})).unwrap()["id"].as_u64().unwrap();
    let r3 = rect(&mut s, 200.0, "#ff0000");
    let r4 = rect(&mut s, 220.0, "#ff0000");
    s.run("select.set", &json!({"ids": [r3, r4]})).unwrap();
    let gb = s.run("object.group", &json!({})).unwrap()["id"].as_u64().unwrap();
    s.run("object.blend.make", &json!({"ids": [ga, gb], "steps": 2})).unwrap();
    // r1 sits in a group (ga) inside the blend: not directly under it.
    let d = &s.active().unwrap().doc;
    assert_eq!(d.position(NodeId(r1)).unwrap().0, Some(NodeId(ga)));
    let res = s.run("color.cluster", &json!({"maxDeltaE": 5, "ids": [r1, r2, r3]})).unwrap();
    assert!(ids_of(&res).is_empty(), "{res}");
    let sk = skipped(&res);
    for id in [r1, r2, r3] {
        assert!(sk.contains(&(id, "inside a live object".into())), "{sk:?}");
    }
    let undo = undo_len(&s);
    let res = s.run("edit.colors.groupByColor", &json!({"maxDeltaE": 5, "ids": [r1, r2, r3, r4]})).unwrap();
    assert_eq!(res["groups"], json!([]));
    assert_eq!(undo_len(&s), undo);
}
