# Task 4 Brief: Layer 5 Engine Command: `color.cluster`

## Goal
Implement the `color.cluster` engine command in `crates/engine/src/cmd/colorcmds.rs` supporting perceptual CIEDE2000 grouping, dry-run `"preview"`, and atomic `"group"` transactions with single-step undo.

## Files
- Modify: `crates/engine/src/cmd/colorcmds.rs`
- Test: `crates/engine/tests/color_cluster_test.rs`

## Requirements & TDD Sequence

### Step 1: Write integration tests
Create `crates/engine/tests/color_cluster_test.rs`:
```rust
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
    s.run("paint.setFill", &json!({"ids": [r1], "paint": "none"})).unwrap();
    
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
```

### Step 2: Run test to verify it fails
```powershell
cargo test -p vectorcraft-engine --test color_cluster_test
```
Expected: FAIL (unknown command `color.cluster`).

### Step 3: Implement `color.cluster` in `crates/engine/src/cmd/colorcmds.rs`
In `crates/engine/src/cmd/colorcmds.rs`:
```rust
fn color_cluster(s: &mut Session, p: &Value) -> Result<Value> {
    let max_delta_e = p.get("max_delta_e").and_then(Value::as_f64).ok_or_else(|| bad("color.cluster", "missing or invalid max_delta_e"))?;
    if !max_delta_e.is_finite() || max_delta_e <= 0.0 {
        return Err(bad("color.cluster", "max_delta_e must be a positive finite number"));
    }
    let action = p.get("action").and_then(Value::as_str).unwrap_or("preview");
    
    let doc_handle = s.doc()?;
    let doc = &doc_handle.doc;
    let target_ids: Vec<NodeId> = if let Some(arr) = p.get("node_ids").and_then(Value::as_array) {
        arr.iter().filter_map(Value::as_u64).map(NodeId).collect()
    } else {
        doc.layers.iter().flat_map(|l| l.children.iter().map(|c| c.id)).collect()
    };

    let mut items: Vec<(u64, Color)> = Vec::new();
    for &id in &target_ids {
        if let Some(node) = doc.node(id) {
            if let Some(fill) = node.appearance.fill_paint() {
                if let Some(col) = fill.to_color() {
                    items.push((id.0, col));
                }
            }
        }
    }

    let clusters = vectorcraft_color::recolor::cluster_colors_by_delta_e(&items, max_delta_e);
    let cluster_json: Vec<Value> = clusters.iter().map(|c| {
        let [r, g, b] = c.representative.to_rgb8();
        json!({
            "group_name": format!("Color #{r:02x}{g:02x}{b:02x}"),
            "hex": format!("#{r:02x}{g:02x}{b:02x}"),
            "node_ids": c.node_ids,
            "avg_delta_e": c.avg_delta_e
        })
    }).collect();

    if action == "preview" {
        return Ok(json!({
            "clusters": cluster_json,
            "modified_node_count": 0
        }));
    }

    if action == "group" {
        let mut modified = 0;
        s.edit("Cluster by Color", |d, _sel| {
            for c in &clusters {
                let [r, g, b] = c.representative.to_rgb8();
                let group_name = format!("Color #{r:02x}{g:02x}{b:02x}");
                let gid = d.alloc_id();
                let mut gnode = Node::group(gid, vec![]);
                gnode.name = Some(group_name);
                if let Some(first_id) = c.node_ids.first() {
                    if let Some((par, idx, _)) = d.position(NodeId(*first_id)) {
                        d.insert(par, idx + 1, gnode)?;
                        for &nid in &c.node_ids {
                            d.move_node(NodeId(nid), Some(gid), usize::MAX)?;
                            modified += 1;
                        }
                    }
                }
            }
            Ok(())
        })?;
        return Ok(json!({
            "clusters": cluster_json,
            "modified_node_count": modified
        }));
    }

    Err(bad("color.cluster", "action must be 'preview' or 'group'"))
}
```

Register command in `specs()` in `crates/engine/src/cmd/colorcmds.rs`:
```rust
cmd!(
    "color.cluster",
    "Cluster by Color",
    ["Object", "Color"],
    None,
    "{max_delta_e: float, action: 'preview'|'group', node_ids?: [int]}",
    has_doc,
    color_cluster
),
```

### Step 4: Run tests to verify they pass
```powershell
cargo test -p vectorcraft-engine --test color_cluster_test
```
Expected: PASS.

### Step 5: Commit
```powershell
git add crates/engine/src/cmd/colorcmds.rs crates/engine/tests/color_cluster_test.rs
git commit -m "feat(engine): add color.cluster command with atomic grouping"
```

## Global Constraints
- **Zero-Panic Invariant**: No `unwrap()`, `expect()`, `panic!`, `todo!`, `unimplemented!` in runtime code outside tests.
- **Float Validation**: Validates `!max_delta_e.is_finite() || max_delta_e <= 0.0`.
- **Single-Flight Undo**: `s.edit("Cluster by Color", ...)` encapsulates all node mutations.

## Report Contract
Write report to: `s:\Github\vectorcraft\.superpowers\sdd\2026-10-12-native-vectorcraft-agent-tools\task-4-report.md`.
Return: Status (DONE / DONE_WITH_CONCERNS / BLOCKED), one-line test summary, and any concerns.
