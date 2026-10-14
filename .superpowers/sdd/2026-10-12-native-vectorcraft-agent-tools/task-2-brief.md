# Task 2 Brief: Layer 1 Color Clustering Primitives (`crates/color`)

## Goal
Implement `cluster_colors_by_delta_e` in `crates/color/src/recolor.rs` using CIEDE2000 color distance math.

## Files
- Modify: `crates/color/src/recolor.rs`
- Test: `crates/color/src/recolor.rs` (inline `#[cfg(test)]` module)

## Requirements & TDD Sequence

### Step 1: Write failing unit tests
Add to the test module in `crates/color/src/recolor.rs`:
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

### Step 2: Run test to verify it fails
```powershell
cargo test -p vectorcraft_color test_cluster_colors_by_delta_e_threshold
```
Expected: FAIL (unresolved function `cluster_colors_by_delta_e`).

### Step 3: Implement `cluster_colors_by_delta_e` in `crates/color/src/recolor.rs`
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

### Step 4: Run tests to verify they pass
```powershell
cargo test -p vectorcraft_color test_cluster_colors
```
Expected: PASS.

### Step 5: Commit
```powershell
git add crates/color/src/recolor.rs
git commit -m "feat(color): implement cluster_colors_by_delta_e with CIEDE2000"
```

## Global Constraints
- **Zero-Panic Invariant**: No `unwrap()`, `expect()`, `panic!`, `todo!`, `unimplemented!` in runtime code.
- Validate all float arguments with `.is_finite()`.
- Clean-room invariant: Strictly use open CIE math (`delta_e2000`).

## Report Contract
Write report to: `s:\Github\vectorcraft\.superpowers\sdd\2026-10-12-native-vectorcraft-agent-tools\task-2-report.md`.
Return: Status (DONE / DONE_WITH_CONCERNS / BLOCKED), one-line test summary, and any concerns.
