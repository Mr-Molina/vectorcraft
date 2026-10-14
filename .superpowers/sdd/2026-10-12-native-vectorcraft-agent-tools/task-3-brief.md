# Task 3 Brief: Layer 5 Engine Command: `document.semantic_summary`

## Goal
Implement the `document.semantic_summary` query command in `crates/engine/src/inspect.rs` and register it in `crates/engine/src/cmd/edit.rs`.

## Files
- Modify: `crates/engine/src/inspect.rs`
- Modify: `crates/engine/src/cmd/edit.rs` (register command in `specs()`)
- Test: `crates/engine/tests/semantic_summary_test.rs`

## Requirements & TDD Sequence

### Step 1: Write integration tests
Create `crates/engine/tests/semantic_summary_test.rs`:
```rust
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
    assert!(summary["palette"].as_array().unwrap().len() >= 1);
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
```

### Step 2: Run test to verify it fails
```powershell
cargo test -p vectorcraft-engine --test semantic_summary_test
```
Expected: FAIL (unknown command `document.semantic_summary`).

### Step 3: Implement `semantic_summary` in `crates/engine/src/inspect.rs` and register in `crates/engine/src/cmd/edit.rs`
In `crates/engine/src/inspect.rs`:
```rust
pub fn semantic_summary(s: &Session, p: &Value) -> Result<Value, crate::EngineError> {
    let doc_handle = s.doc()?;
    let doc = &doc_handle.doc;
    let max_palette = p.get("max_palette_entries").and_then(Value::as_u64).unwrap_or(32) as usize;
    
    let artboard_rect = doc.artboards.first().map(|a| a.rect);
    let mut bg_card_id = None;
    let mut total_nodes = 0;
    let mut group_count = 0;
    let mut loose_paths = 0;
    let mut palette_map: std::collections::BTreeMap<String, (usize, Vec<u64>)> = std::collections::BTreeMap::new();
    
    for layer in &doc.layers {
        for (idx, child) in layer.children.iter().enumerate() {
            total_nodes += 1;
            if child.is_container() {
                group_count += 1;
            } else {
                loose_paths += 1;
            }
            if idx == 0 && bg_card_id.is_none() {
                if let (Some(ab), Some(gb)) = (artboard_rect, child.geometric_bounds()) {
                    let dw = (ab.width() - gb.width()).abs();
                    let dh = (ab.height() - gb.height()).abs();
                    if dw <= ab.width() * 0.02 && dh <= ab.height() * 0.02 {
                        bg_card_id = Some(child.id.0);
                    }
                }
            }
            if let Some(fill) = child.appearance.fill_paint() {
                let hex = fill.label();
                let entry = palette_map.entry(hex).or_insert((0, Vec::new()));
                entry.0 += 1;
                if entry.1.len() < 5 {
                    entry.1.push(child.id.0);
                }
            }
        }
    }
    
    let palette: Vec<Value> = palette_map.into_iter().take(max_palette).map(|(hex, (count, ids))| {
        json!({ "hex": hex, "count": count, "sample_node_ids": ids })
    }).collect();

    Ok(json!({
        "artboard": artboard_rect.map(|r| json!({"width": r.width(), "height": r.height()})).unwrap_or(Value::Null),
        "background_card_id": bg_card_id,
        "total_nodes": total_nodes,
        "group_count": group_count,
        "loose_path_count": loose_paths,
        "palette": palette
    }))
}
```

In `crates/engine/src/cmd/edit.rs`:
Add command spec in `specs()`:
```rust
cmd!(
    query "document.semantic_summary",
    "Semantic Document Summary",
    [],
    None,
    "{selection_only?: bool, max_palette_entries?: int} -> token-efficient summary",
    has_doc,
    doc_semantic_summary
),
```
And add wrapper function:
```rust
fn doc_semantic_summary(s: &mut Session, p: &Value) -> Result<Value> {
    crate::inspect::semantic_summary(s, p)
}
```

### Step 4: Run tests to verify they pass
```powershell
cargo test -p vectorcraft-engine --test semantic_summary_test
```
Expected: PASS.

### Step 5: Commit
```powershell
git add crates/engine/src/inspect.rs crates/engine/src/cmd/edit.rs crates/engine/tests/semantic_summary_test.rs
git commit -m "feat(engine): add document.semantic_summary query command"
```

## Global Constraints
- **Zero-Panic Invariant**: No `unwrap()`, `expect()`, `panic!`, `todo!`, `unimplemented!` in runtime code.
- Clean-room: Clean public SVG/geometry logic only.

## Report Contract
Write report to: `s:\Github\vectorcraft\.superpowers\sdd\2026-10-12-native-vectorcraft-agent-tools\task-3-report.md`.
Return: Status (DONE / DONE_WITH_CONCERNS / BLOCKED), one-line test summary, and any concerns.
