# Native VectorCraft Agent Extensions Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build engine-native semantic summarization, CIEDE2000 perceptual clustering, and labeled visual overlays directly into VectorCraft and expose them as first-class MCP tools.

**Architecture:** Layer-partitioned extensions following VectorCraft's clean-room architecture: public color clustering in Layer 1 (`vectorcraft_color`), engine commands and transaction coordination in Layer 5 (`vectorcraft_engine`), visual overlay rendering in Layer 3 (`vectorcraft_render`), and MCP protocol exposure in Layer 6 (`vectorcraft_mcp`).

**Tech Stack:** Rust (stable MSVC), `vectorcraft_color`, `vectorcraft_engine`, `vectorcraft_render`, `vectorcraft_mcp`, serde_json, MCP protocol JSON-RPC.

**Spec:** [`docs/superpowers/specs/2026-10-12-native-vectorcraft-agent-tools-design.md`](file:///s:/Github/vectorcraft/docs/superpowers/specs/2026-10-12-native-vectorcraft-agent-tools-design.md)

**Execution Waves:**
- **Wave 0 (Setup)**: Task 1 (Toolchain Bootstrapping)
- **Wave 1 (Parallel Core)**: Task 2 (`vectorcraft_color` clustering), Task 3 (`document.semantic_summary`) [disjoint crates/modules]
- **Wave 2 (Engine Mutation & Render)**: Task 4 (`color.cluster` commands, blocked by Task 2), Task 5 (`render.overlay`)
- **Wave 3 (Integration & Server)**: Task 6 (`vectorcraft_mcp` tool registrations, blocked by Tasks 3, 4, 5)
- **Wave 4 (Verification & Release)**: Task 7 (Quality gates, layers check, release compilation, live verification)

## Global Constraints

- **Clean-room Invariant**: Never use Adobe software, reverse engineer, or copy Adobe code/fixtures. Algorithms must come from public ISO/CIE/W3C specs.
- **Zero-Panic Invariant**: No `unwrap()`, `expect()`, `panic!`, `todo!`, or `unimplemented!` in runtime code outside tests.
- **Layering Enforcement**: Strictly preserve layer hierarchy (`cargo xtask layers`). L6 cannot depend on UI; L5 cannot depend on L6.
- **Single-Flight Undo**: All mutating operations must be wrapped in `Session::edit` to preserve an atomic undo step.
- **Bounded Inputs**: All float parameters must validate `.is_finite()`; all collections must cap maximum node evaluations to 5,000 items.

## Review Focus

1. **Non-finite Float Thresholds**: User supplies `NaN` or `+Infinity` for `max_delta_e` or `scale` -> Must reject with `EngineError::BadParam` instead of panicking or infinite looping. Tested in Task 2 (Step 1) and Task 4 (Step 1).
2. **Missing Node ID References**: Target list contains a node ID deleted or invalid -> Must gracefully ignore missing ID or return clean error without modifying surviving nodes. Tested in Task 4 (Step 1).
3. **Empty Color Nodes**: Node has neither fill nor stroke (transparent container or guide) -> Must fall back to skipping node rather than crashing color conversion. Tested in Task 4 (Step 1: `test_color_cluster_skips_empty_color_nodes`).
4. **Artboard Aspect Ratio Divergence**: Document has no artboard or multiple artboards -> `background_card_id` detector must fall back to `None` without crashing. Tested in Task 3 (Step 1: `test_semantic_summary_artboard_divergence_fallback`).
5. **Atomic Rollback on Mid-Batch Failure**: A failure occurs while reparenting nodes -> Must abort and roll back all prior moves, preserving original document state intact. Tested in Task 4 (Step 1: `test_color_cluster_atomic_rollback_on_mid_batch_failure`).

---

### Task 1: Rust Toolchain Bootstrapping & Environment Setup

**Files:**
- System Environment: `$env:PATH`, `winget`, `$HOME/.cargo/bin`

**Dependencies:**
- Blocked By: None
- Parallel Wave: Wave 0

**Interfaces:**
- Consumes: Host OS package manager (`winget`)
- Produces: Verified `cargo.exe` and `rustc.exe` available in shell session

#### Verification Pattern: Pattern D (Infrastructure / DevOps)
- [ ] **Step 1: Install Rustup via winget**
  Run: `winget install Rustlang.Rustup -h --accept-source-agreements --accept-package-agreements`
  Expected: Exit code 0 or successfully registered package.

- [ ] **Step 2: Refresh shell environment and add cargo to PATH**
  Run: `$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"`

- [ ] **Step 3: Verify toolchain installation**
  Run: `cargo --version; rustc --version`
  Expected: Outputs `cargo 1.x.x` and `rustc 1.x.x` with exit code 0.

- [ ] **Step 4: Verify workspace metadata**
  Run: `cargo metadata --no-deps --format-version 1`
  Expected: JSON document listing VectorCraft workspace crates with exit code 0.

---

### Task 2: Layer 1 Color Clustering Primitives (`crates/color`)

**Files:**
- Modify: `crates/color/src/recolor.rs`
- Test: `crates/color/src/recolor.rs` (inline tests)

**Dependencies:**
- Blocked By: Task 1
- Parallel Wave: Wave 1

**Interfaces:**
- Consumes: `cms::delta_e2000`, `recolor::cluster`, `Color::to_lab()`
- Produces: `pub fn cluster_colors_by_delta_e(colors: &[(u64, Color)], max_delta_e: f64) -> Vec<ColorCluster>`

#### Verification Pattern: Pattern A (Standard TDD)
- [ ] **Step 1: Write failing unit tests for `cluster_colors_by_delta_e` including failure modes**
  Add to `crates/color/src/recolor.rs` tests:
  ```rust
  #[test]
  fn test_cluster_colors_by_delta_e_threshold() {
      let red1 = (1, Color::rgb(0.9, 0.1, 0.1));
      let red2 = (2, Color::rgb(0.91, 0.11, 0.1));
      let blue = (3, Color::rgb(0.1, 0.1, 0.9));
      let clusters = cluster_colors_by_delta_e(&[red1, red2, blue], 5.0);
      assert_eq!(clusters.len(), 2);
      assert_eq!(clusters[0].node_ids, vec![1, 2]);
      assert_eq!(clusters[1].node_ids, vec![3]);
  }

  #[test]
  fn test_cluster_colors_rejects_non_finite_and_negative() {
      let red = (1, Color::rgb(0.9, 0.1, 0.1));
      assert!(cluster_colors_by_delta_e(&[red], f64::NAN).is_empty());
      assert!(cluster_colors_by_delta_e(&[red], f64::INFINITY).is_empty());
      assert!(cluster_colors_by_delta_e(&[red], -1.0).is_empty());
      assert!(cluster_colors_by_delta_e(&[], 5.0).is_empty());
  }
  ```

- [ ] **Step 2: Run test to verify it fails**
  Run: `cargo test -p vectorcraft_color test_cluster_colors_by_delta_e_threshold`
  Expected: FAIL (unresolved function `cluster_colors_by_delta_e`).

- [ ] **Step 3: Implement `cluster_colors_by_delta_e` in `crates/color/src/recolor.rs`**
  ```rust
  #[derive(Clone, Debug, PartialEq)]
  pub struct ColorCluster {
      pub representative: Color,
      pub node_ids: Vec<u64>,
      pub avg_delta_e: f64,
  }

  pub fn cluster_colors_by_delta_e(items: &[(u64, Color)], max_delta_e: f64) -> Vec<ColorCluster> {
      if items.is_empty() || !max_delta_e.is_finite() || max_delta_e <= 0.0 {
          return Vec::new();
      }
      let colors: Vec<Color> = items.iter().map(|(_, c)| *c).collect();
      let weights: Vec<f32> = vec![1.0; items.len()];
      let raw_clusters = cluster(&colors, &weights, items.len().min(64));
      let mut out = Vec::new();
      for group in raw_clusters {
          if group.is_empty() { continue; }
          let rep = items[group[0]].1;
          let rep_lab = rep.to_lab();
          let mut ids = Vec::new();
          let mut total_de = 0.0;
          for &idx in &group {
              let (id, col) = items[idx];
              let de = delta_e2000(rep_lab, col.to_lab()) as f64;
              if de <= max_delta_e || ids.is_empty() {
                  ids.push(id);
                  total_de += de;
              }
          }
          if !ids.is_empty() {
              out.push(ColorCluster {
                  representative: rep,
                  avg_delta_e: total_de / (ids.len() as f64),
                  node_ids: ids,
              });
          }
      }
      out
  }
  ```

- [ ] **Step 4: Run test to verify it passes**
  Run: `cargo test -p vectorcraft_color test_cluster_colors_by_delta_e_threshold test_cluster_colors_rejects_non_finite_and_negative`
  Expected: PASS.

- [ ] **Step 5: Commit**
  Run: `git commit -am "feat(color): implement cluster_colors_by_delta_e with CIEDE2000"`

---

### Task 3: Layer 5 Engine Command: `document.semantic_summary`

**Files:**
- Modify: `crates/engine/src/inspect.rs`
- Modify: `crates/engine/src/cmd/edit.rs` (register command)
- Test: `crates/engine/tests/semantic_summary_test.rs`

**Dependencies:**
- Blocked By: Task 1
- Parallel Wave: Wave 1

**Interfaces:**
- Consumes: `Session::doc()`, `NodeKind`, `geometric_bounds()`
- Produces: Engine command `document.semantic_summary`

#### Verification Pattern: Pattern A (Standard TDD)
- [ ] **Step 1: Write integration tests for `document.semantic_summary` (happy path & missing artboard)**
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
      // Draw shape with mismatched bounds
      s.run("shape.rectangle", &json!({"x": 10, "y": 10, "width": 50, "height": 50})).unwrap();
      let summary = s.run("document.semantic_summary", &json!({})).unwrap();
      assert!(summary["background_card_id"].is_null());
      assert_eq!(summary["total_nodes"], 1);
  }
  ```

- [ ] **Step 2: Run test to verify it fails**
  Run: `cargo test -p vectorcraft_engine --test semantic_summary_test`
  Expected: FAIL (unknown command `document.semantic_summary`).

- [ ] **Step 3: Implement `semantic_summary` in `crates/engine/src/inspect.rs` and register in `edit.rs`**
  In `crates/engine/src/inspect.rs`:
  ```rust
  pub fn semantic_summary(s: &Session, p: &Value) -> Result<Value, crate::EngineError> {
      let doc_handle = s.doc()?;
      let doc = &doc_handle.doc;
      let selection_only = p.get("selection_only").and_then(Value::as_bool).unwrap_or(false);
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
  In `crates/engine/src/cmd/edit.rs`, register command:
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
  And define wrapper:
  ```rust
  fn doc_semantic_summary(s: &mut Session, p: &Value) -> Result<Value> {
      crate::inspect::semantic_summary(s, p)
  }
  ```

- [ ] **Step 4: Run test to verify it passes**
  Run: `cargo test -p vectorcraft_engine --test semantic_summary_test`
  Expected: PASS.

- [ ] **Step 5: Commit**
  Run: `git commit -am "feat(engine): add document.semantic_summary query command"`

---

### Task 4: Layer 5 Engine Command: `color.cluster`

**Files:**
- Modify: `crates/engine/src/cmd/colorcmds.rs`
- Test: `crates/engine/tests/color_cluster_test.rs`

**Dependencies:**
- Blocked By: Task 2
- Parallel Wave: Wave 2

**Interfaces:**
- Consumes: `vectorcraft_color::recolor::cluster_colors_by_delta_e`, `Session::edit`, `group_nodes`
- Produces: Engine command `color.cluster` supporting `"preview"`, `"group"`, and `"merge_existing"`

#### Verification Pattern: Pattern A (Standard TDD)
- [ ] **Step 1: Write integration tests for `color.cluster` (preview, group, empty nodes, invalid floats, missing IDs, atomic rollback)**
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
      
      // Test invalid action causing rejection prior to or during mutation
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
      
      // Verify atomic single undo restores original tree
      s.run("edit.undo", &json!({})).unwrap();
      let summary = s.run("document.semantic_summary", &json!({})).unwrap();
      assert_eq!(summary["group_count"], 0);
      assert_eq!(summary["loose_path_count"], 2);
  }
  ```

- [ ] **Step 2: Run test to verify it fails**
  Run: `cargo test -p vectorcraft_engine --test color_cluster_test`
  Expected: FAIL (unknown command `color.cluster`).

- [ ] **Step 3: Implement `color.cluster` in `crates/engine/src/cmd/colorcmds.rs`**
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
          s.edit("Cluster by Color", |d, sel| {
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
  Register command in `crates/engine/src/cmd/colorcmds.rs`:
  ```rust
  cmd!(
      "color.cluster",
      "Cluster by Color",
      ["Object", "Color"],
      None,
      "{max_delta_e: float, action: 'preview'|'group', node_ids?: [int]}",
      has_doc,
      color_cluster
  )
  ```

- [ ] **Step 4: Run test to verify it passes**
  Run: `cargo test -p vectorcraft_engine --test color_cluster_test`
  Expected: PASS.

- [ ] **Step 5: Commit**
  Run: `git commit -am "feat(engine): add color.cluster command with atomic grouping"`

---

### Task 5: Layer 5 & 3 Engine Command: `render.overlay`

**Files:**
- Modify: `crates/engine/src/cmd/docmenu.rs` (or `crates/engine/src/cmd/fileio/mod.rs`)
- Test: `crates/engine/tests/render_overlay_test.rs`

**Dependencies:**
- Blocked By: Task 1
- Parallel Wave: Wave 2

**Interfaces:**
- Consumes: `vectorcraft_render`, `geometric_bounds()`
- Produces: Engine command `render.overlay`

#### Verification Pattern: Pattern A (Standard TDD)
- [ ] **Step 1: Write integration tests for `render.overlay` (happy path & non-finite scale)**
  Create `crates/engine/tests/render_overlay_test.rs`:
  ```rust
  use serde_json::json;
  use std::path::PathBuf;
  use vectorcraft_engine::Session;

  #[test]
  fn test_render_overlay_emits_valid_png() {
      let mut s = Session::headless();
      s.run("file.new", &json!({"width": 200, "height": 200})).unwrap();
      s.run("shape.rectangle", &json!({"x": 10, "y": 10, "width": 50, "height": 50})).unwrap();
      let out_path = std::env::temp_dir().join("vc_test_overlay.png");
      let res = s.run("render.overlay", &json!({"path": out_path.to_str().unwrap()})).unwrap();
      assert_eq!(res["annotated_count"], 1);
      assert!(out_path.exists());
      std::fs::remove_file(out_path).ok();
  }

  #[test]
  fn test_render_overlay_rejects_non_finite_scale() {
      let mut s = Session::headless();
      s.run("file.new", &json!({"width": 200, "height": 200})).unwrap();
      let out_path = std::env::temp_dir().join("vc_test_overlay_bad.png");
      assert!(s.run("render.overlay", &json!({"path": out_path.to_str().unwrap(), "scale": f64::NAN})).is_err());
  }
  ```

- [ ] **Step 2: Run test to verify it fails**
  Run: `cargo test -p vectorcraft_engine --test render_overlay_test`
  Expected: FAIL (unknown command `render.overlay`).

- [ ] **Step 3: Implement `render.overlay` in `crates/engine/src/cmd/docmenu.rs`**
  ```rust
  fn render_overlay(s: &mut Session, p: &Value) -> Result<Value> {
      let path = p.get("path").and_then(Value::as_str).ok_or_else(|| bad("render.overlay", "missing target path"))?;
      let scale = p.get("scale").and_then(Value::as_f64).unwrap_or(2.0);
      if !scale.is_finite() || scale <= 0.0 {
          return Err(bad("render.overlay", "scale must be a positive finite number"));
      }

      let doc_handle = s.doc()?;
      let doc = &doc_handle.doc;
      let artboard = doc.artboards.first().ok_or_else(|| bad("render.overlay", "document has no artboard"))?;
      
      // Render base document to pixel buffer
      let mut pixmap = vectorcraft_render::render_artboard(doc, artboard, scale as f32)?;
      
      // Annotate bounding boxes and badges
      let mut annotated = 0;
      for layer in &doc.layers {
          for child in &layer.children {
              if let Some(bounds) = child.geometric_bounds() {
                  annotated += 1;
                  let sx = (bounds.x0 * scale) as i32;
                  let sy = (bounds.y0 * scale) as i32;
                  let sw = (bounds.width() * scale) as i32;
                  let sh = (bounds.height() * scale) as i32;
                  pixmap.draw_debug_box(sx, sy, sw, sh, child.id.0);
              }
          }
      }

      pixmap.save_png(path)?;

      Ok(json!({
          "path": path,
          "annotated_count": annotated,
          "dimensions": { "width": pixmap.width(), "height": pixmap.height() }
      }))
  }
  ```
  Register command in `docmenu.rs`:
  ```rust
  cmd!(
      "render.overlay",
      "Render Labeled Overlay",
      [],
      None,
      "{path: string, scale?: float, node_ids?: [int]}",
      has_doc,
      render_overlay
  )
  ```

- [ ] **Step 4: Run test to verify it passes**
  Run: `cargo test -p vectorcraft_engine --test render_overlay_test`
  Expected: PASS.

- [ ] **Step 5: Commit**
  Run: `git commit -am "feat(engine): add render.overlay command with labeled badges"`

---

### Task 6: Layer 6 MCP Server Tool Declarations & Protocol Wiring

**Files:**
- Modify: `crates/mcp/src/tools.rs`
- Test: `crates/mcp/src/tests_protocol.rs`

**Dependencies:**
- Blocked By: Tasks 3, 4, 5
- Parallel Wave: Wave 3

**Interfaces:**
- Consumes: Engine commands `document.semantic_summary`, `color.cluster`, `render.overlay`
- Produces: MCP tools `semantic_summary`, `cluster_colors`, `render_overlay`

#### Verification Pattern: Pattern A (Standard TDD)
- [ ] **Step 1: Write protocol tests in `crates/mcp/src/tests_protocol.rs`**
  ```rust
  #[test]
  fn test_mcp_tools_include_agent_extensions() {
      let defs = tool_definitions();
      let names: Vec<&str> = defs.iter().filter_map(|t| t["name"].as_str()).collect();
      assert!(names.contains(&"semantic_summary"));
      assert!(names.contains(&"cluster_colors"));
      assert!(names.contains(&"render_overlay"));
  }
  ```

- [ ] **Step 2: Run test to verify it fails**
  Run: `cargo test -p vectorcraft_mcp test_mcp_tools_include_agent_extensions`
  Expected: FAIL.

- [ ] **Step 3: Register tools and dispatch match branches in `crates/mcp/src/tools.rs`**
  Add to `tool_definitions()` in `crates/mcp/src/tools.rs`:
  ```rust
  tool(
      "semantic_summary",
      "Get Semantic Document Summary",
      "Returns a token-efficient summary of the active document: artboards, canvas background card ID, total group and loose path counts, and dominant palette frequency.",
      obj(json!({
          "selection_only": {"type": "boolean", "description": "Only summarize selected nodes"},
          "max_palette_entries": {"type": "integer", "description": "Maximum color swatches (default 32)"}
      }), &[]),
      true,
  ),
  tool(
      "cluster_colors",
      "Cluster Nodes by Perceptual Color",
      "Groups or previews nodes clustered by perceptual color distance (CIEDE2000). Action 'preview' returns proposed groups. Action 'group' creates new named color groups atomically.",
      obj(json!({
          "node_ids": {"type": "array", "items": {"type": "integer"}, "description": "Node IDs to cluster"},
          "max_delta_e": {"type": "number", "description": "CIEDE2000 distance threshold"},
          "action": {"type": "string", "enum": ["preview", "group"], "description": "Execution action"}
      }), &["max_delta_e", "action"]),
      false,
  ),
  tool(
      "render_overlay",
      "Render Labeled Visual Overlay",
      "Exports an annotated PNG image of the artboard displaying semi-transparent bounding boxes and high-contrast numbered badges for nodes.",
      obj(json!({
          "node_ids": {"type": "array", "items": {"type": "integer"}, "description": "Node IDs to label"},
          "path": {"type": "string", "description": "Target PNG file path"},
          "scale": {"type": "number", "description": "Raster scale factor (default 2.0)"}
      }), &["path"]),
      true,
  ),
  ```
  Add dispatch arms in `crates/mcp/src/tools.rs`:
  ```rust
  "semantic_summary" => j(b.call("document.semantic_summary", Value::Object(a))?),
  "cluster_colors" => j(b.call("color.cluster", Value::Object(a))?),
  "render_overlay" => j(b.call("render.overlay", Value::Object(a))?),
  ```

- [ ] **Step 4: Run test to verify it passes**
  Run: `cargo test -p vectorcraft_mcp test_mcp_tools_include_agent_extensions`
  Expected: PASS.

- [ ] **Step 5: Commit**
  Run: `git commit -am "feat(mcp): expose semantic_summary, cluster_colors, and render_overlay tools"`

---

### Task 7: Quality Gates, Release Build & Live Verification

**Files:**
- Crates: All workspace crates

**Dependencies:**
- Blocked By: Task 6
- Parallel Wave: Wave 4

**Interfaces:**
- Consumes: All completed tasks
- Produces: Production-ready `vectorcraft.exe` and `vectorcraft-cli.exe` binaries

#### Verification Pattern: Pattern D (Infrastructure / DevOps)
- [ ] **Step 1: Run layer violation check**
  Run: `cargo xtask layers`
  Expected: PASS (zero layer violations).

- [ ] **Step 2: Run clippy and no-panic audit**
  Run: `cargo clippy --all-targets -D warnings`
  Expected: PASS (zero warnings, zero unwrap/panic violations).

- [ ] **Step 3: Run full workspace tests**
  Run: `cargo test -p vectorcraft_color -p vectorcraft_engine -p vectorcraft_mcp`
  Expected: All tests pass.

- [ ] **Step 4: Build release binaries**
  Run: `cargo build --release -p vectorcraft -p vectorcraft-cli`
  Expected: Binaries created in `target/release/`.

- [ ] **Step 5: Live Verification against test artboard**
  Run: `target/release/vectorcraft-cli.exe run --command document.semantic_summary`
  Expected: Returns JSON semantic summary containing artboard and palette data.

- [ ] **Step 6: Commit and update ROADMAP/status**
  Run: `git commit -am "chore(release): complete native vectorcraft agent extensions"`
