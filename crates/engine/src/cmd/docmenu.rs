//! Select menu long tail (Same → text/symbol attributes, Object → Direction Handles / text kinds /
//! brush strokes, Save/recall selections), View → Guides, and File → Close All / Document Color
//! Mode / File Info.

use std::collections::BTreeSet;
use std::sync::Arc;

use serde_json::{Value, json};
use vectorcraft_doc::{CharStyle, Guide, Node, NodeId, NodeKind, SavedSelection, TextKind};
use vectorcraft_geom::Vec2;

use super::edit::selected_roots;
use super::*;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!("select.same.symbolInstance", "Symbol Instance", ["Select", "Same"], None, "{} → {count}", has_selection, |s, _| same(
            s,
            "select.same.symbolInstance",
            |a, b| match (&a.kind, &b.kind) {
                (NodeKind::SymbolInstance { symbol: x, .. }, NodeKind::SymbolInstance { symbol: y, .. }) => x == y,
                _ => false,
            }
        )),
        cmd!("select.same.fontFamily", "Font Family", ["Select", "Same"], None, "{} → {count}", has_selection, |s, _| same_text(
            s,
            "select.same.fontFamily",
            |a, b| a.font_family == b.font_family
        )),
        cmd!("select.same.fontFamilyStyle", "Font Family & Style", ["Select", "Same"], None, "{} → {count}", has_selection, |s, _| same_text(
            s,
            "select.same.fontFamilyStyle",
            |a, b| a.font_family == b.font_family && a.font_style == b.font_style
        )),
        cmd!("select.same.fontFamilyStyleSize", "Font Family, Style & Size", ["Select", "Same"], None, "{} → {count}", has_selection, |s, _| {
            same_text(s, "select.same.fontFamilyStyleSize", |a, b| {
                a.font_family == b.font_family && a.font_style == b.font_style && (a.size - b.size).abs() < 1e-6
            })
        }),
        cmd!("select.same.fontSize", "Font Size", ["Select", "Same"], None, "{} → {count}", has_selection, |s, _| same_text(
            s,
            "select.same.fontSize",
            |a, b| (a.size - b.size).abs() < 1e-6
        )),
        cmd!("select.same.textFillColor", "Text Fill Color", ["Select", "Same"], None, "{} → {count}", has_selection, |s, _| same_text(
            s,
            "select.same.textFillColor",
            |a, b| a.fill == b.fill
        )),
        cmd!("select.same.textStrokeColor", "Text Stroke Color", ["Select", "Same"], None, "{} → {count}", has_selection, |s, _| same_text(
            s,
            "select.same.textStrokeColor",
            |a, b| a.stroke == b.stroke
        )),
        cmd!(
            "select.object.directionHandles",
            "Direction Handles",
            ["Select", "Object"],
            None,
            "{} direct-select every anchor (showing all handles) of the selected paths → {anchors}",
            has_selection,
            direction_handles
        ),
        cmd!("select.object.pointText", "Point Text Objects", ["Select", "Object"], None, "{} → {count}", has_doc, |s, _| by_kind(
            s,
            |n| matches!(
                &n.kind,
                NodeKind::Text(t) if matches!(t.kind, TextKind::Point)
            )
        )),
        cmd!("select.object.areaText", "Area Text Objects", ["Select", "Object"], None, "{} → {count}", has_doc, |s, _| by_kind(s, |n| matches!(
            &n.kind,
            NodeKind::Text(t) if matches!(t.kind, TextKind::Area { .. })
        ))),
        cmd!("select.object.brushStrokes", "Brush Strokes", ["Select", "Object"], None, "{} → {count}", has_doc, |s, _| by_kind(s, |n| n
            .appearance
            .stroke()
            .is_some_and(|st| st.brush.is_some()))),
        cmd!(
            "select.object.bristleBrushStrokes",
            "Bristle Brush Strokes",
            ["Select", "Object"],
            None,
            "{} objects whose brush name contains \"bristle\" → {count}",
            has_doc,
            |s, _| by_kind(s, |n| n
                .appearance
                .stroke()
                .and_then(|st| st.brush.as_deref())
                .is_some_and(|b| b.to_ascii_lowercase().contains("bristle")))
        ),
        cmd!(
            "select.save",
            "Save Selection…",
            ["Select"],
            None,
            "{name?} save the current selection under a name, in the document (default \"Selection N\"; an existing name is replaced; at most 25, names up to 255 characters) → {name}",
            has_selection,
            save_selection
        ),
        cmd!(
            "select.recall",
            "Recall Selection",
            ["Select"],
            None,
            "{name} select the objects of a saved selection (those since deleted are left out) → {count}",
            has_saved_selection,
            recall_selection
        ),
        cmd!(
            "select.editSaved",
            "Edit Selection…",
            ["Select"],
            None,
            "{name, newName?: rename, delete?: bool} | {edits: [{name, newName?, delete?}…]} rename or delete saved selections, all in one undo step; names are those before the edit and must stay unique",
            has_saved_selection,
            edit_saved
        ),
        cmd!(query "select.savedList", "Saved Selections", [], None, "{} → [name…] for the active document", has_doc, |s, _| {
            Ok(json!(s.doc()?.doc.saved_selections.iter().map(|x| x.name.clone()).collect::<Vec<_>>()))
        }),
        cmd!(
            "view.guides.make",
            "Make Guides",
            ["View", "Guides"],
            Some("Cmd+5"),
            "{} turn the selected paths into guides → {count}",
            has_selection,
            make_guides
        ),
        cmd!(
            "view.guides.release",
            "Release Guides",
            ["View", "Guides"],
            Some("Cmd+Alt+5"),
            "{} turn the selected guides (or, with none selected, all guide paths) back into paths → {count}",
            has_doc,
            release_guides
        ),
        cmd!(
            query "view.guides.lock",
            "Lock Guides",
            ["View", "Guides"],
            Some("Cmd+Alt+;"),
            "{locked?: bool} toggle (or set) the session's guide lock → {locked}",
            has_doc,
            lock_guides
        ),
        cmd!(
            "view.guides.clear",
            "Clear Guides",
            ["View", "Guides"],
            None,
            "{} delete all ruler guides and guide paths → {count}",
            has_doc,
            clear_guides
        ),
        cmd!(
            "guide.add",
            "Add Guide",
            [],
            None,
            "{vertical: bool, pos: pt (x for vertical, y for horizontal), artboard?: index (an artboard guide: it runs across that artboard only and moves, is copied and is deleted with it; default a canvas guide, across the whole canvas)} → {index}",
            has_doc,
            guide_add
        ),
        cmd!(
            query "guide.list",
            "Guides",
            [],
            None,
            "{} → [{index, vertical, pos, selected, artboard?: index (an artboard guide's)}…] the ruler guides",
            has_doc,
            guide_list
        ),
        cmd!(
            "guide.select",
            "Select Guides",
            [],
            None,
            "{indexes: [index…], toggle?: bool} select ruler guides on their own (deselecting the art); toggle adds or removes them instead → {selected: [index…]}",
            guides_unlocked,
            guide_select
        ),
        cmd!(
            "guide.remove",
            "Remove Guide",
            [],
            None,
            "{index?} delete ruler guide `index`, else the selected ones (as Delete / edit.clear does) → {count}",
            guides_unlocked,
            guide_remove
        ),
        cmd!(
            "guide.move",
            "Move Guide",
            [],
            None,
            "{index, pos: pt} put ruler guide `index` at `pos` | {dx?, dy?: pt, copy?: bool} move the selected guides (vertical ones by dx, horizontal ones by dy); copy leaves them and selects the moved copies",
            guides_unlocked,
            guide_move
        ),
        cmd!("file.closeAll", "Close All", ["File"], Some("Cmd+Alt+W"), "{} → {closed}", has_doc, close_all),
        cmd!(
            "file.documentColorMode",
            "Document Color Mode",
            ["File", "Document Color Mode"],
            None,
            "{mode: \"cmyk\"|\"rgb\", convert?: true (convert every colour of the art, symbols, pattern tiles and swatches through the colour settings; swatch links kept; Gray colours stay Gray, on the black plate), intent?, grays?: \"profile\" (default: RGB greys separate through the CMYK profile like any colour, into four-colour greys and a rich black) | \"black\" (to CMYK, RGB greys with R = G = B go on the black plate only, K = their grey value)} → {changed}",
            has_doc,
            super::colormgmt::convert_mode
        ),
        cmd!(
            "file.info",
            "File Info…",
            ["File"],
            Some("Cmd+Alt+Shift+I"),
            "{title?, author?, authorTitle?, description?, keywords?: [string…]|\"a, b\" (each once), rating?: 0–5, copyrightStatus?: \"unknown\"|\"copyrighted\"|\"publicDomain\", copyrightNotice?, copyrightUrl?} set the File Info in one undo step (SVG with metadata, PDF and PNG exports carry it); no params → {title, author, authorTitle, description, keywords, rating, copyrightStatus, copyrightNotice, copyrightUrl, created, modified (ISO 8601 UTC or null; read-only), colorMode, units, artboards, objects}",
            has_doc,
            super::fileinfo::file_info
        ),
        cmd!(
            query "render.overlay",
            "Render Labeled Overlay",
            [],
            None,
            "{artboard?: int (default 0), scale?: float (default 1), ids?: [int], ungroupedOnly?: bool, path?: string} → {pngBase64: string, width: int, height: int, annotatedCount: int, path: string|null} the artboard rendered with each object's world bounds boxed and labelled by id (ids absent: the top-level objects of every visible layer and sublayer; ungroupedOnly: only loose objects — directly in a layer and not groups); written to path only when given",
            has_doc,
            render_overlay
        ),
    ]
}

impl Session {
    /// View → Guides → Lock Guides state (canvas guide dragging should honour it).
    pub fn guides_locked(&self) -> bool {
        self.menu.guides_locked
    }
}

// ---------- Select ----------

fn candidates(s: &Session, f: impl Fn(&Node) -> bool) -> Result<Vec<NodeId>> {
    let st = s.doc()?;
    let mut ids = vec![];
    for l in st.doc.layers.iter().filter(|l| l.visible && !l.locked) {
        l.walk(&mut |n| {
            if !n.is_container() && n.visible && !n.locked && f(n) {
                ids.push(n.id);
            }
        });
    }
    Ok(ids)
}

fn finish_same(s: &mut Session, cmd: &str, ids: Vec<NodeId>) -> Result<Value> {
    let n = ids.len();
    s.select(|_, sel| sel.set(ids))?;
    s.doc_mut()?.last_selection_cmd = Some((cmd.to_string(), json!({})));
    Ok(json!({ "count": n }))
}

fn same(s: &mut Session, cmd: &str, eq: fn(&Node, &Node) -> bool) -> Result<Value> {
    let st = s.doc()?;
    let r = st.selection.objects.first().and_then(|id| st.doc.node(*id)).cloned().ok_or_else(|| bad(cmd, "nothing selected"))?;
    let ids = candidates(s, |n| eq(n, &r))?;
    finish_same(s, cmd, ids)
}

fn first_text_style(s: &Session) -> Option<CharStyle> {
    let st = s.active()?;
    let mut out = None;
    for id in &st.selection.objects {
        st.doc.node(*id)?.walk(&mut |n| {
            if out.is_none()
                && let NodeKind::Text(t) = &n.kind
            {
                out = Some(t.first_style());
            }
        });
        if out.is_some() {
            break;
        }
    }
    out
}

fn same_text(s: &mut Session, cmd: &str, eq: fn(&CharStyle, &CharStyle) -> bool) -> Result<Value> {
    let r = first_text_style(s).ok_or_else(|| bad(cmd, "select a text object"))?;
    let ids = candidates(s, |n| match &n.kind {
        NodeKind::Text(t) => t.runs.iter().any(|run| eq(&run.style, &r)),
        _ => false,
    })?;
    finish_same(s, cmd, ids)
}

fn by_kind(s: &mut Session, f: fn(&Node) -> bool) -> Result<Value> {
    let ids = candidates(s, f)?;
    let n = ids.len();
    s.select(|_, sel| sel.set(ids))?;
    Ok(json!({ "count": n }))
}

fn direction_handles(s: &mut Session, _: &Value) -> Result<Value> {
    let roots = selected_roots(s)?;
    let d = &s.doc()?.doc;
    let mut items: Vec<(NodeId, BTreeSet<(usize, usize)>)> = vec![];
    for r in &roots {
        if let Some(n) = d.node(*r) {
            n.walk(&mut |c| {
                if let Some(p) = c.path_data() {
                    items.push((c.id, p.anchors().map(|(si, ai, _)| (si, ai)).collect()));
                }
            });
        }
    }
    let total: usize = items.iter().map(|x| x.1.len()).sum();
    s.select(|_, sel| {
        sel.clear();
        for (id, refs) in items {
            sel.add(id);
            sel.anchors.insert(id, refs);
        }
    })?;
    Ok(json!({ "anchors": total }))
}

fn save_selection(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "select.save";
    let st = s.doc()?;
    let ids = st.selection.objects.clone();
    let saved = &st.doc.saved_selections;
    let name = str_param(p, "name").and_then(SavedSelection::clean_name).unwrap_or_else(|| SavedSelection::default_name(saved));
    let existing = saved.iter().position(|x| x.name == name);
    if existing.is_none() && saved.len() >= SavedSelection::MAX {
        return Err(bad(C, format!("a document keeps at most {} saved selections", SavedSelection::MAX)));
    }
    s.edit("Save Selection", |d, _| {
        let entry = SavedSelection { name: name.clone(), objects: ids };
        match existing.and_then(|i| d.saved_selections.get_mut(i)) {
            Some(x) => *x = entry,
            None => d.saved_selections.push(entry),
        }
        Ok(())
    })?;
    Ok(json!({ "name": name }))
}

fn recall_selection(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "select.recall";
    let name = str_param(p, "name").ok_or_else(|| bad(C, "missing name"))?;
    let ids = s.doc()?.doc.saved_selections.iter().find(|x| x.name == name).map(|x| x.objects.clone());
    let ids = ids.ok_or_else(|| bad(C, format!("no saved selection `{name}`")))?;
    s.select(|d, sel| sel.set(ids.into_iter().filter(|id| d.node(*id).is_some())))?;
    Ok(json!({ "count": s.doc()?.selection.len() }))
}

/// Recall and Edit Selection… need a document with at least one saved selection.
fn has_saved_selection(s: &Session) -> std::result::Result<(), String> {
    let st = s.active().ok_or("no document open")?;
    if st.doc.saved_selections.is_empty() { Err("no saved selections".into()) } else { Ok(()) }
}

fn edit_saved(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "select.editSaved";
    let edits = match p.get("edits").and_then(Value::as_array) {
        Some(list) => list.as_slice(),
        None => std::slice::from_ref(p),
    };
    let before = s.doc()?.doc.saved_selections.clone();
    // Edits name the selections as they are now, so they don't depend on each other's order.
    let mut after: Vec<Option<SavedSelection>> = before.iter().cloned().map(Some).collect();
    // Files may hold any number of saved selections: keep this linear.
    let mut index = std::collections::HashMap::with_capacity(before.len());
    for (i, x) in before.iter().enumerate() {
        index.entry(x.name.as_str()).or_insert(i);
    }
    for e in edits {
        let name = str_param(e, "name").ok_or_else(|| bad(C, "missing name"))?;
        let i = *index.get(name).ok_or_else(|| bad(C, format!("no saved selection `{name}`")))?;
        let Some(slot) = after.get_mut(i) else { continue };
        if bool_or(e, "delete", false) {
            *slot = None;
        } else if let Some(n) = str_param(e, "newName").and_then(SavedSelection::clean_name)
            && let Some(x) = slot
        {
            x.name = n;
        }
    }
    let after: Vec<SavedSelection> = after.into_iter().flatten().collect();
    let mut names = std::collections::HashSet::with_capacity(after.len());
    for x in &after {
        if !names.insert(x.name.as_str()) {
            return Err(bad(C, format!("two saved selections would be named `{}`", x.name)));
        }
    }
    if after != before {
        s.edit("Edit Selection", |d, _| {
            d.saved_selections = after;
            Ok(())
        })?;
    }
    ok()
}

// ---------- Guides ----------

fn guides_unlocked(s: &Session) -> std::result::Result<(), String> {
    has_doc(s)?;
    if s.menu.guides_locked { Err("guides are locked".into()) } else { Ok(()) }
}

fn set_guide_flag(n: &mut Node, on: bool, count: &mut usize) {
    match &mut n.kind {
        NodeKind::Path { guide, clipping: false, .. } => {
            if *guide != on {
                *guide = on;
                *count += 1;
            }
        }
        _ => {
            if let Some(ch) = n.children_mut() {
                for c in ch.iter_mut() {
                    set_guide_flag(Arc::make_mut(c), on, count);
                }
            }
        }
    }
}

fn make_guides(s: &mut Session, _: &Value) -> Result<Value> {
    let roots = selected_roots(s)?;
    let n = s.edit("Make Guides", |d, sel| {
        let mut n = 0;
        for r in &roots {
            if let Some(node) = d.node_mut(*r) {
                set_guide_flag(node, true, &mut n);
            }
        }
        if n == 0 {
            return Err(EngineError::Other("Make Guides: select paths".into()));
        }
        sel.clear();
        Ok(n)
    })?;
    Ok(json!({ "count": n }))
}

fn guide_paths(d: &vectorcraft_doc::Document) -> Vec<NodeId> {
    let mut v = vec![];
    d.walk(|n| {
        if matches!(n.kind, NodeKind::Path { guide: true, .. }) {
            v.push(n.id);
        }
    });
    v
}

fn release_guides(s: &mut Session, _: &Value) -> Result<Value> {
    let st = s.doc()?;
    let all = guide_paths(&st.doc);
    let sel: Vec<NodeId> = all.iter().copied().filter(|id| st.selection.contains(*id)).collect();
    let targets: Vec<NodeId> = if sel.is_empty() { all.into_iter().filter(|id| st.doc.is_editable(*id)).collect() } else { sel };
    if targets.is_empty() {
        return Err(EngineError::Other("Release Guides: no guides".into()));
    }
    let n = targets.len();
    s.edit("Release Guides", |d, sel| {
        for id in &targets {
            if let Some(NodeKind::Path { guide, .. }) = d.node_mut(*id).map(|n| &mut n.kind) {
                *guide = false;
            }
        }
        sel.set(targets.iter().copied());
        Ok(())
    })?;
    Ok(json!({ "count": n }))
}

fn lock_guides(s: &mut Session, p: &Value) -> Result<Value> {
    let v = p.get("locked").and_then(Value::as_bool).unwrap_or(!s.menu.guides_locked);
    s.menu.guides_locked = v;
    // Locked guides can't stay selected.
    if v && !s.doc()?.selection.guides.is_empty() {
        s.select(|_, sel| sel.guides.clear())?;
    }
    Ok(json!({ "locked": v }))
}

fn clear_guides(s: &mut Session, _: &Value) -> Result<Value> {
    let st = s.doc()?;
    let paths = guide_paths(&st.doc);
    let n = paths.len() + st.doc.guides.len();
    if n == 0 {
        return Ok(json!({ "count": 0 }));
    }
    s.edit("Clear Guides", |d, _| {
        d.guides.clear();
        for id in &paths {
            let _ = d.remove(*id);
        }
        Ok(())
    })?;
    Ok(json!({ "count": n }))
}

fn guide_add(s: &mut Session, p: &Value) -> Result<Value> {
    let pos = f64_req(p, "pos", "guide.add")?;
    let vertical = bool_or(p, "vertical", false);
    let artboard = match p.get("artboard").filter(|v| !v.is_null()) {
        Some(v) => {
            let boards = &s.doc()?.doc.artboards;
            let ab = v.as_u64().and_then(|i| boards.get(usize::try_from(i).ok()?)).ok_or_else(|| EngineError::Other("no such artboard".into()))?;
            Some(ab.id)
        }
        None => None,
    };
    let i = s.edit("New Guide", |d, _| {
        d.guides.push(Guide { artboard, ..Guide::new(vertical, pos) });
        Ok(d.guides.len() - 1)
    })?;
    Ok(json!({ "index": i }))
}

fn guide_list(s: &mut Session, _: &Value) -> Result<Value> {
    let st = s.doc()?;
    let row = |(i, g): (usize, &Guide)| {
        let mut r = json!({ "index": i, "vertical": g.vertical, "pos": g.pos, "selected": st.selection.guides.contains(&i) });
        if let Some(ab) = st.doc.artboards.iter().position(|a| Some(a.id) == g.artboard) {
            r["artboard"] = json!(ab);
        }
        r
    };
    Ok(Value::Array(st.doc.guides.iter().enumerate().map(row).collect()))
}

/// Ruler guide `index` of the active document (`index` validated).
fn guide_index(s: &Session, v: Option<&Value>, cmd: &str) -> Result<usize> {
    let i = v.and_then(Value::as_u64).and_then(|i| usize::try_from(i).ok()).ok_or_else(|| bad(cmd, "an index must be a whole number"))?;
    if i >= s.doc()?.doc.guides.len() {
        return Err(bad(cmd, format!("no guide {i}")));
    }
    Ok(i)
}

fn guide_select(s: &mut Session, p: &Value) -> Result<Value> {
    let list = p.get("indexes").and_then(Value::as_array).ok_or_else(|| bad("guide.select", "missing indexes"))?;
    let picked = list.iter().map(|v| guide_index(s, Some(v), "guide.select")).collect::<Result<Vec<_>>>()?;
    if bool_or(p, "toggle", false) {
        let mut now = s.doc()?.selection.guides.clone();
        for i in picked {
            if let Some(k) = now.iter().position(|g| *g == i) {
                now.remove(k);
            } else {
                now.push(i);
            }
        }
        s.select(|_, sel| sel.set_guides(now))?;
    } else {
        s.select(|_, sel| sel.set_guides(picked))?;
    }
    Ok(json!({ "selected": s.doc()?.selection.guides }))
}

/// Delete ruler guide `index`, else the selected guides, in one undo step → {count}.
pub(crate) fn guide_remove(s: &mut Session, p: &Value) -> Result<Value> {
    let gone: Vec<usize> = match p.get("index") {
        Some(v) => vec![guide_index(s, Some(v), "guide.remove")?],
        None => s.doc()?.selection.guides.clone(),
    };
    if gone.is_empty() {
        return Err(bad("guide.remove", "no guide selected"));
    }
    let label = if gone.len() == 1 { "Delete Guide" } else { "Delete Guides" };
    s.edit(label, |d, sel| {
        d.retain_guides(sel, |i, _| !gone.contains(&i));
        Ok(())
    })?;
    Ok(json!({ "count": gone.len() }))
}

/// Put ruler guide `index` at `pos`, else move (or copy) the selected guides, in one undo step.
pub(crate) fn guide_move(s: &mut Session, p: &Value) -> Result<Value> {
    if let Some(v) = p.get("index") {
        let i = guide_index(s, Some(v), "guide.move")?;
        let pos = f64_req(p, "pos", "guide.move")?;
        s.edit("Move Guide", |d, _| {
            if let Some(g) = d.guides.get_mut(i) {
                g.pos = pos;
            }
            Ok(())
        })?;
        return ok();
    }
    let moving = s.doc()?.selection.guides.clone();
    if moving.is_empty() {
        return Err(bad("guide.move", "no guide selected (or give an index)"));
    }
    let (dx, dy, copy) = (f64_or(p, "dx", 0.0), f64_or(p, "dy", 0.0), bool_or(p, "copy", false));
    s.edit(if copy { "Copy Guide" } else { "Move Guide" }, |d, sel| {
        let mut copies = vec![];
        for &i in &moving {
            let Some(g) = d.guides.get_mut(i) else { continue };
            let moved = g.moved(Vec2::new(dx, dy));
            if copy {
                copies.push(moved);
            } else {
                *g = moved;
            }
        }
        if copy {
            let first = d.guides.len();
            d.guides.extend(copies);
            sel.set_guides(first..d.guides.len());
        }
        Ok(())
    })?;
    ok()
}

// ---------- File ----------

fn close_all(s: &mut Session, _: &Value) -> Result<Value> {
    let n = s.documents().len();
    while !s.documents().is_empty() {
        s.close_document(s.documents().len() - 1);
    }
    Ok(json!({ "closed": n }))
}

// ---------- Render Overlay ----------

// 3x5 bitmap glyphs for decimal digits '0'..='9'.
// Each row has 3 bits (bits 2, 1, 0 from left to right).
const DIGIT_GLYPHS_3X5: [[u8; 5]; 10] = [
    [0b111, 0b101, 0b101, 0b101, 0b111], // 0
    [0b010, 0b110, 0b010, 0b010, 0b111], // 1
    [0b111, 0b001, 0b111, 0b100, 0b111], // 2
    [0b111, 0b001, 0b111, 0b001, 0b111], // 3
    [0b101, 0b101, 0b111, 0b001, 0b001], // 4
    [0b111, 0b100, 0b111, 0b001, 0b111], // 5
    [0b111, 0b100, 0b111, 0b101, 0b111], // 6
    [0b111, 0b001, 0b001, 0b001, 0b001], // 7
    [0b111, 0b101, 0b111, 0b101, 0b111], // 8
    [0b111, 0b101, 0b111, 0b001, 0b111], // 9
];

fn blend_pixel(img: &mut image::RgbaImage, x: i32, y: i32, color: [u8; 4]) {
    if x < 0 || y < 0 {
        return;
    }
    let (ux, uy) = (x as u32, y as u32);
    if ux >= img.width() || uy >= img.height() {
        return;
    }
    let px = img.get_pixel_mut(ux, uy);
    let sa = color[3] as u32;
    if sa == 0 {
        return;
    }
    if sa == 255 {
        px.0 = color;
        return;
    }
    let inv = 255 - sa;
    let r = (color[0] as u32 * sa + px.0[0] as u32 * inv + 127) / 255;
    let g = (color[1] as u32 * sa + px.0[1] as u32 * inv + 127) / 255;
    let b = (color[2] as u32 * sa + px.0[2] as u32 * inv + 127) / 255;
    let a = sa + (px.0[3] as u32 * inv + 127) / 255;
    px.0 = [r.min(255) as u8, g.min(255) as u8, b.min(255) as u8, a.min(255) as u8];
}

/// Most ids `render.overlay` accepts in one call (untrusted input).
const OVERLAY_MAX_IDS: usize = 100_000;
/// Most bounding box targets `render.overlay` will annotate to prevent slow pixel tint loops.
const OVERLAY_MAX_TARGETS: usize = 2048;

/// `p[key]`, treating JSON `null` as absent.
fn overlay_param<'a>(p: &'a Value, key: &str) -> Option<&'a Value> {
    p.get(key).filter(|v| !v.is_null())
}

/// The top-level objects (children of layers and sublayers, not of groups) of `layer`, skipping
/// hidden objects and hidden sublayers.
fn overlay_layer_art(layer: &Node, depth: usize, out: &mut Vec<NodeId>) {
    if depth >= 512 {
        return;
    }
    for c in layer.children().into_iter().flatten().filter(|c| c.visible) {
        if c.is_layer() {
            overlay_layer_art(c, depth + 1, out);
        } else {
            out.push(c.id);
        }
    }
}

pub(crate) fn render_overlay(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "render.overlay";
    let path = match overlay_param(p, "path") {
        Some(v) => {
            let s = v.as_str().filter(|s| !s.is_empty()).ok_or_else(|| bad(C, "path must be a non-empty string"))?;
            if s.contains('\0') || std::path::Path::new(s).components().any(|c| matches!(c, std::path::Component::ParentDir)) {
                return Err(bad(C, "path cannot contain parent directory components or null bytes"));
            }
            Some(s)
        }
        None => None,
    };
    let scale = match overlay_param(p, "scale") {
        Some(v) => v.as_f64().filter(|sc| sc.is_finite() && *sc > 0.0).ok_or_else(|| bad(C, "scale must be a positive finite number"))?,
        None => 1.0,
    };
    let artboard_index = match overlay_param(p, "artboard") {
        Some(v) => v.as_u64().ok_or_else(|| bad(C, "artboard must be a non-negative integer"))?,
        None => 0,
    };
    let ungrouped_only = match overlay_param(p, "ungroupedOnly") {
        Some(v) => v.as_bool().ok_or_else(|| bad(C, "ungroupedOnly must be true or false"))?,
        None => false,
    };
    let ids: Option<Vec<NodeId>> = match overlay_param(p, "ids") {
        Some(v) => {
            let arr = v.as_array().ok_or_else(|| bad(C, "ids must be an array of non-negative integers"))?;
            if arr.len() > OVERLAY_MAX_IDS {
                return Err(bad(C, format!("ids may list at most {OVERLAY_MAX_IDS} objects")));
            }
            let mut seen = std::collections::HashSet::with_capacity(arr.len());
            let mut out = Vec::with_capacity(arr.len());
            for v in arr {
                let id = NodeId(v.as_u64().ok_or_else(|| bad(C, "ids must be an array of non-negative integers"))?);
                if seen.insert(id) {
                    out.push(id);
                }
            }
            Some(out)
        }
        None => None,
    };

    let doc = &s.doc()?.doc;
    let artboard = usize::try_from(artboard_index)
        .ok()
        .and_then(|i| doc.artboards.get(i))
        .ok_or_else(|| bad(C, format!("artboard {artboard_index} is out of range (the document has {})", doc.artboards.len())))?;
    let region = artboard.rect;
    if ![region.x0, region.y0, region.x1, region.y1].iter().all(|v| v.is_finite()) {
        return Err(bad(C, "artboard has non-finite bounds"));
    }
    // Refuse before rendering anything (same limit as File › Export and Rasterize).
    super::menucmds::check_raster_size(region, scale)?;

    // Explicit ids are looked up anywhere in the tree (unknown ids are skipped); otherwise the
    // top-level objects of every visible layer.
    let candidates: Vec<NodeId> = match ids {
        Some(ids) => ids.into_iter().filter(|id| doc.node(*id).is_some()).collect(),
        None => {
            let mut out = Vec::new();
            for layer in doc.layers.iter().filter(|l| l.visible) {
                overlay_layer_art(layer, 0, &mut out);
            }
            out
        }
    };
    let mut targets: Vec<(NodeId, vectorcraft_geom::Rect)> = Vec::with_capacity(candidates.len().min(OVERLAY_MAX_TARGETS));
    for id in candidates {
        if targets.len() >= OVERLAY_MAX_TARGETS {
            break;
        }
        // ungroupedOnly: loose objects only — directly in a layer and not themselves groups.
        let loose = || {
            doc.parent_of(id).and_then(|pid| doc.node(pid)).is_some_and(Node::is_layer)
                && doc.node(id).is_some_and(|n| !matches!(n.kind, NodeKind::Group { .. }))
        };
        if ungrouped_only && !loose() {
            continue;
        }
        // World-space geometric bounds, as reported by document.inspect's selectionBounds.
        if let Some(b) = doc.bounds_of(&[id], false) {
            targets.push((id, b));
        }
    }

    let mut renderer = vectorcraft_render::Renderer::new();
    let rendered = renderer.render_region(doc, region, scale, true);
    let mut img =
        image::RgbaImage::from_raw(rendered.width, rendered.height, rendered.to_straight()).ok_or_else(|| bad(C, "invalid rendered image buffer"))?;

    // At most 65535 per side (checked above), so these always fit.
    let w = i32::try_from(img.width()).unwrap_or(i32::MAX);
    let h = i32::try_from(img.height()).unwrap_or(i32::MAX);

    const TINT: [u8; 4] = [0, 150, 255, 40];
    const BORDER: [u8; 4] = [0, 120, 240, 220];
    const BADGE_BG: [u8; 4] = [20, 20, 30, 230];
    const BADGE_BORDER: [u8; 4] = [0, 120, 240, 240];
    const BADGE_TEXT: [u8; 4] = [255, 255, 255, 255];

    let mut annotated = 0usize;

    for (node_id, bounds) in targets {
        // Pixel coordinates, clamped to just outside the image so far-away art can't make the
        // drawing loops below run for billions of iterations.
        let to_px = |v: f64, origin: f64, max: i32| -> Option<i32> {
            let r = ((v - origin) * scale).round();
            r.is_finite().then(|| r.clamp(-1.0, f64::from(max) + 1.0) as i32)
        };
        let (Some(x0), Some(y0), Some(x1), Some(y1)) =
            (to_px(bounds.x0, region.x0, w), to_px(bounds.y0, region.y0, h), to_px(bounds.x1, region.x0, w), to_px(bounds.y1, region.y0, h))
        else {
            continue;
        };

        let (bx0, bx1) = if x0 <= x1 { (x0, x1) } else { (x1, x0) };
        let (by0, by1) = if y0 <= y1 { (y0, y1) } else { (y1, y0) };

        // A box entirely outside the image gets no badge and doesn't count as annotated.
        if bx1 < 0 || by1 < 0 || bx0 > w - 1 || by0 > h - 1 {
            continue;
        }

        // 1. Semi-transparent bounding box interior tint (`.max(0)`: `clamp` panics if min > max).
        let (last_x, last_y) = ((w - 1).max(0), (h - 1).max(0));
        let fill_x0 = bx0.clamp(0, last_x);
        let fill_x1 = bx1.clamp(0, last_x);
        let fill_y0 = by0.clamp(0, last_y);
        let fill_y1 = by1.clamp(0, last_y);

        for y in fill_y0..=fill_y1 {
            for x in fill_x0..=fill_x1 {
                blend_pixel(&mut img, x, y, TINT);
            }
        }

        // 2. 1px bounding box border
        for x in bx0..=bx1 {
            blend_pixel(&mut img, x, by0, BORDER);
            blend_pixel(&mut img, x, by1, BORDER);
        }
        for y in by0..=by1 {
            blend_pixel(&mut img, bx0, y, BORDER);
            blend_pixel(&mut img, bx1, y, BORDER);
        }

        // 3. Numbered badge displaying node_id
        let text = node_id.0.to_string();
        let num_digits = text.len() as i32;
        // 3x5 font scaled 2x: digit is 6x10 px, spacing 2 px
        let text_w = num_digits * 6 + (num_digits - 1).max(0) * 2;
        let text_h = 10;
        let pad_x = 3;
        let pad_y = 2;
        let badge_w = text_w + 2 * pad_x;
        let badge_h = text_h + 2 * pad_y;

        let badge_x = bx0.clamp(0, (w - badge_w).max(0));
        let badge_y = by0.clamp(0, (h - badge_h).max(0));

        // Draw badge background & border
        for by in badge_y..badge_y + badge_h {
            for bx in badge_x..badge_x + badge_w {
                blend_pixel(&mut img, bx, by, BADGE_BG);
            }
        }
        for bx in badge_x..badge_x + badge_w {
            blend_pixel(&mut img, bx, badge_y, BADGE_BORDER);
            blend_pixel(&mut img, bx, badge_y + badge_h - 1, BADGE_BORDER);
        }
        for by in badge_y..badge_y + badge_h {
            blend_pixel(&mut img, badge_x, by, BADGE_BORDER);
            blend_pixel(&mut img, badge_x + badge_w - 1, by, BADGE_BORDER);
        }

        // Draw badge digits (3x5 scaled 2x)
        let mut cur_x = badge_x + pad_x;
        let cur_y = badge_y + pad_y;
        for c in text.chars() {
            if let Some(digit) = c.to_digit(10)
                && let Some(rows) = DIGIT_GLYPHS_3X5.get(digit as usize)
            {
                for (r_idx, row_bits) in rows.iter().enumerate() {
                    for col in 0..3 {
                        if (row_bits & (1 << (2 - col))) != 0 {
                            let px = cur_x + col * 2;
                            let py = cur_y + r_idx as i32 * 2;
                            for dy in 0..2 {
                                for dx in 0..2 {
                                    blend_pixel(&mut img, px + dx, py + dy, BADGE_TEXT);
                                }
                            }
                        }
                    }
                }
            }
            cur_x += 6 + 2;
        }

        annotated += 1;
    }

    // Same encoder as `Rendered::to_png` (PNG, no metadata), in memory.
    let mut png = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).map_err(|e| bad(C, format!("failed to encode PNG: {e}")))?;
    if let Some(path) = path {
        super::fileio::write_file(path, &png)?;
    }

    Ok(json!({
        "pngBase64": vectorcraft_format::base64_encode(&png),
        "width": img.width(),
        "height": img.height(),
        "annotatedCount": annotated,
        "path": path,
    }))
}
