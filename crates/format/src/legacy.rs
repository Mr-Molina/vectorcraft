//! Writing for older readers: format v1's anchor form.

use serde_json::{Value, json};

pub(crate) const MAX_LEGACY_DEPTH: usize = 64;

/// Rewrite every anchor in a serialized document (`anchors` arrays of path data) the way format v1
/// wrote it: `{p: {x, y}, in: {x, y}, out: {x, y}, kind}`, every member present (v1 readers need
/// them all; v2 writes `{p: [x, y], in?, out?, kind?}`).
pub(crate) fn anchors_to_v1(v: &mut Value) {
    anchors_to_v1_depth(v, 0);
}

fn anchors_to_v1_depth(v: &mut Value, depth: usize) {
    if depth >= MAX_LEGACY_DEPTH {
        return;
    }
    match v {
        Value::Object(o) => {
            for (k, x) in o.iter_mut() {
                match x {
                    Value::Array(anchors) if k == "anchors" => anchors.iter_mut().for_each(anchor_to_v1),
                    _ => anchors_to_v1_depth(x, depth + 1),
                }
            }
        }
        Value::Array(a) => {
            for x in a.iter_mut() {
                anchors_to_v1_depth(x, depth + 1);
            }
        }
        _ => {}
    }
}

pub(crate) fn point(v: &Value) -> Option<Value> {
    if let Some([x, y]) = v.as_array().map(Vec::as_slice)
        && x.is_number()
        && y.is_number()
    {
        let x = x.as_f64()?;
        let y = y.as_f64()?;
        return Some(json!({ "x": x, "y": y }));
    }
    if v.get("x").is_some_and(Value::is_number) && v.get("y").is_some_and(Value::is_number) {
        let x = v.get("x")?.as_f64()?;
        let y = v.get("y")?.as_f64()?;
        return Some(json!({ "x": x, "y": y }));
    }
    None
}

fn anchor_to_v1(a: &mut Value) {
    let Some(o) = a.as_object() else { return };
    // Not a v2 anchor (already a map, or something else called `anchors`): leave it alone.
    let Some(p) = o.get("p").and_then(point) else { return };
    let handle = |k: &str| o.get(k).and_then(point).unwrap_or_else(|| p.clone());
    let kind = o.get("kind").cloned().unwrap_or_else(|| json!("Corner"));
    *a = json!({ "p": p, "in": handle("in"), "out": handle("out"), "kind": kind });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchors_are_written_the_v1_way() {
        let mut v = json!({"layers": [{"path": {"subpaths": [{"anchors": [{"p": [1.0, 2.0]}, {"p": [3.0, 4.0], "out": [5.0, 4.0], "kind": "Smooth"}], "closed": true}]}}], "effect": {"anchors": true}});
        anchors_to_v1(&mut v);
        let a = &v["layers"][0]["path"]["subpaths"][0]["anchors"];
        assert_eq!(a[0], json!({"p": {"x": 1.0, "y": 2.0}, "in": {"x": 1.0, "y": 2.0}, "out": {"x": 1.0, "y": 2.0}, "kind": "Corner"}));
        assert_eq!(a[1], json!({"p": {"x": 3.0, "y": 4.0}, "in": {"x": 3.0, "y": 4.0}, "out": {"x": 5.0, "y": 4.0}, "kind": "Smooth"}));
        assert_eq!(v["effect"]["anchors"], json!(true));
    }

    #[test]
    fn point_validation() {
        assert_eq!(point(&json!([1.5, 2.5])), Some(json!({"x": 1.5, "y": 2.5})));
        assert_eq!(point(&json!({"x": 3.0, "y": 4.0})), Some(json!({"x": 3.0, "y": 4.0})));
        assert_eq!(point(&json!([1.0])), None);
        assert_eq!(point(&json!(["a", 2.0])), None);
        assert_eq!(point(&json!({"x": "a", "y": 2.0})), None);
        assert_eq!(point(&json!({"x": 1.0})), None);
    }

    #[test]
    fn recursion_depth_limit() {
        let mut deep = json!({"anchors": [{"p": [1.0, 2.0]}]});
        for _ in 0..70 {
            deep = json!({"nested": deep});
        }
        anchors_to_v1(&mut deep);
        // The deep anchor shouldn't be touched because it exceeds MAX_LEGACY_DEPTH
        let mut curr = &deep;
        for _ in 0..70 {
            curr = &curr["nested"];
        }
        assert_eq!(curr["anchors"][0]["p"], json!([1.0, 2.0]));
    }
}
