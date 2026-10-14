//! Agent-friendly document summaries and view-models for panels.

use serde_json::{Value, json};
use vectorcraft_doc::{ArrowAlign, Document, LineCap, LineJoin, Node, NodeKind, StrokeAlign, StrokeLayer, WidthProfile};

use crate::Session;

fn rect_json(r: Option<vectorcraft_geom::Rect>) -> Value {
    match r {
        Some(r) => {
            if !r.x0.is_finite() || !r.y0.is_finite() || !r.x1.is_finite() || !r.y1.is_finite() {
                Value::Null
            } else {
                json!({ "x": r.x0, "y": r.y0, "width": r.width(), "height": r.height() })
            }
        }
        None => Value::Null,
    }
}

/// How much of the subtree a node summary carries.
#[derive(Clone, Copy, Default)]
pub struct SummaryOpts {
    /// Child levels to include (`None`: the whole subtree). `0` is the node alone.
    pub depth: Option<u64>,
    /// Children shown per node (`None`: all, top of the stack first). A level that shows
    /// fewer children than it has reports `childCount`, so truncation is never silent.
    pub child_limit: Option<u64>,
}

/// Compact tree summary of a node (for `document.inspect`): the whole subtree.
pub fn node_summary(n: &Node) -> Value {
    node_summary_opts(n, SummaryOpts::default())
}

/// The same summary with [`SummaryOpts`] applied.
pub fn node_summary_opts(n: &Node, opts: SummaryOpts) -> Value {
    node_summary_opts_depth(n, opts, 0)
}

fn node_summary_opts_depth(n: &Node, opts: SummaryOpts, depth: usize) -> Value {
    if depth >= SEMANTIC_MAX_DEPTH {
        let mut v = json!({
            "id": n.id.0,
            "name": n.display_name(),
            "kind": n.kind_label(),
            "visible": n.visible,
            "locked": n.locked,
            "bounds": rect_json(n.geometric_bounds()),
        });
        if let Some(ch) = n.children() {
            v["childCount"] = json!(ch.len());
        }
        return v;
    }
    let mut v = json!({
        "id": n.id.0,
        "name": n.display_name(),
        "kind": n.kind_label(),
        "visible": n.visible,
        "locked": n.locked,
        "bounds": rect_json(n.geometric_bounds()),
    });
    if n.opacity < 1.0 {
        v["opacity"] = json!(n.opacity);
    }
    // The angle of its rotated bounding box (counter-clockwise degrees).
    if n.bbox_angle != 0.0 {
        v["rotation"] = json!(n.bbox_angle);
    }
    if let NodeKind::Text(t) = &n.kind {
        // Type shows its characters' paint: the first run's, as paint.proxies and the Stroke
        // options read it. Fills and strokes of the type object itself, above or below its
        // characters, come separately so the two stay apart.
        let default = vectorcraft_doc::CharStyle::default();
        let style = t.runs.first().map_or(&default, |r| &r.style);
        v["fill"] = json!(style.fill.label());
        v["stroke"] = json!(style.stroke.label());
        v["strokeWidth"] = json!(if style.stroke.is_none() { 0.0 } else { style.stroke_width });
        if n.appearance.fill().is_some() || n.appearance.stroke().is_some() {
            v["objectFill"] = json!(n.appearance.fill_paint().label());
            v["objectStroke"] = json!(n.appearance.stroke_paint().label());
            v["objectStrokeWidth"] = json!(n.appearance.stroke_width());
        }
    } else if !n.is_container() {
        v["fill"] = json!(n.appearance.fill_paint().label());
        v["stroke"] = json!(n.appearance.stroke_paint().label());
        v["strokeWidth"] = json!(n.appearance.stroke_width());
    }
    if let Some(st) = node_stroke(n) {
        v["strokeOptions"] = stroke_options(&st);
    }
    match &n.kind {
        NodeKind::Layer { color, template, printable, preview, dim_images, clip, .. } => {
            let [r, g, b] = color.rgb();
            v["color"] = json!(format!("#{r:02x}{g:02x}{b:02x}"));
            v["template"] = json!(template);
            v["printable"] = json!(printable);
            v["preview"] = json!(preview);
            v["dimImages"] = json!(dim_images);
            v["clip"] = json!(clip);
        }
        NodeKind::Path { path, .. } => {
            v["anchors"] = json!(path.anchor_count());
            v["closed"] = json!(path.is_closed());
        }
        NodeKind::Text(t) => v["text"] = json!(t.plain_text()),
        _ => {}
    }
    if let Some(ch) = n.children() {
        let total = ch.len();
        let take_n = opts.child_limit.map_or(total, |l| l.min(total as u64) as usize);
        if depth + 1 < SEMANTIC_MAX_DEPTH && opts.depth.is_none_or(|d| d > 0) {
            let next = SummaryOpts { depth: opts.depth.map(|d| d.saturating_sub(1)), child_limit: opts.child_limit };
            v["children"] = Value::Array(ch.iter().rev().take(take_n).map(|c| node_summary_opts_depth(c, next, depth + 1)).collect());
        }
        if take_n < total || v.get("children").is_none() {
            v["childCount"] = json!(total);
        }
    }
    v
}

pub fn document(s: &Session) -> Value {
    document_opts(s, SummaryOpts::default())
}

/// The same summary with [`SummaryOpts`] applied to the layer tree (artboards and
/// the rest always come whole): `depth: 0` is the skeleton — top layers with counts.
pub fn document_opts(s: &Session, opts: SummaryOpts) -> Value {
    let Some(st) = s.active() else { return Value::Null };
    let d = &st.doc;
    json!({
        "title": st.title(),
        "path": st.path,
        "dirty": st.is_dirty(),
        "revision": st.revision,
        "units": d.units.label(),
        "colorMode": format!("{:?}", d.color_mode),
        "artboards": d.artboards.iter().map(|a| json!({"name": a.name, "x": a.rect.x0, "y": a.rect.y0, "width": a.rect.width(), "height": a.rect.height()})).collect::<Vec<_>>(),
        // Top of the stack first, like the Layers panel.
        "layers": d.layers.iter().rev().map(|l| node_summary_opts(l, opts)).collect::<Vec<_>>(),
        "currentLayer": st.active_layer.map(|l| l.0),
        // The rows highlighted in the Layers panel (`layer.setCurrent`, `layer.highlight`).
        "layerRows": st.highlighted_rows().iter().map(|i| i.0).collect::<Vec<_>>(),
        "isolation": st.isolation.map(|l| l.0),
        "selection": st.selection.objects.iter().map(|i| i.0).collect::<Vec<_>>(),
        "selectionBounds": rect_json(d.bounds_of(&st.selection.objects, false)),
        // The angle the selection's bounding box stands at (counter-clockwise degrees).
        "selectionRotation": d.bbox_angle(&st.selection.objects),
        // The layer, group or object targeted through the Layers panel (`layer.target`).
        "target": st.selection.target.map(|t| t.0),
        "history": st.history.undo.iter().map(|h| h.label.clone()).collect::<Vec<_>>(),
        "redo": st.history.redo.iter().rev().map(|h| h.label.clone()).collect::<Vec<_>>(),
        "objects": d.node_count(),
        "tool": s.tool_id(),
        "paint": {"fill": s.paint.fill.label(), "stroke": s.paint.stroke.label(), "strokeWidth": s.paint.stroke_width, "fillActive": s.fill_active, "appearanceItem": s.appearance_item()},
        "pasteRemembersLayers": d.paste_remembers_layers,
    })
}

/// What `document.find` matches: every given filter must hit (AND). All matching is
/// case-insensitive; `name` and `text` are substrings, `kind` is the exact panel label
/// (`Layer`, `Group`, `Path`, `Type`, `Image`, ...).
pub struct FindFilter {
    pub name: Option<String>,
    pub kind: Option<String>,
    pub text: Option<String>,
}

impl FindFilter {
    /// True when no effective filter was given (empty strings count as absent).
    pub fn is_empty(&self) -> bool {
        [self.name.as_ref(), self.kind.as_ref(), self.text.as_ref()].iter().all(|f| f.is_none())
    }

    fn hits(&self, n: &Node) -> bool {
        if let Some(needle) = &self.name
            && !n.display_name().to_lowercase().contains(needle)
        {
            return false;
        }
        if let Some(kind) = &self.kind
            && !n.kind_label().eq_ignore_ascii_case(kind)
        {
            return false;
        }
        if let Some(needle) = &self.text {
            let NodeKind::Text(t) = &n.kind else { return false };
            if !t.plain_text().to_lowercase().contains(needle) {
                return false;
            }
        }
        true
    }
}

/// Search the whole layer tree, top of the stack first like the Layers panel.
/// Returns at most `limit` hits as `{id, name, kind, path}` (`path` is the ancestor
/// chain, layer first) plus the total hit count, so a capped reply stays explicit.
pub fn find_nodes(doc: &Document, filter: &FindFilter, limit: u64) -> (Vec<Value>, u64) {
    use std::sync::Arc;
    let mut out = Vec::new();
    let mut total = 0u64;
    let mut ancestors: Vec<u64> = Vec::new();
    fn walk(nodes: &[Arc<Node>], ancestors: &mut Vec<u64>, depth: usize, filter: &FindFilter, limit: u64, out: &mut Vec<Value>, total: &mut u64) {
        if depth >= SEMANTIC_MAX_DEPTH {
            return;
        }
        for n in nodes.iter().rev() {
            if filter.hits(n) {
                *total += 1;
                if (out.len() as u64) < limit {
                    out.push(json!({"id": n.id.0, "name": n.display_name(), "kind": n.kind_label(), "path": ancestors}));
                }
            }
            if let Some(ch) = n.children() {
                ancestors.push(n.id.0);
                walk(ch, ancestors, depth + 1, filter, limit, out, total);
                ancestors.pop();
            }
        }
    }
    walk(&doc.layers, &mut ancestors, 0, filter, limit, &mut out, &mut total);
    (out, total)
}

/// Cap names as `stroke.set` takes them.
pub const CAPS: [(LineCap, &str); 3] = [(LineCap::Butt, "butt"), (LineCap::Round, "round"), (LineCap::Square, "square")];
/// Join names as `stroke.set` takes them.
pub const JOINS: [(LineJoin, &str); 3] = [(LineJoin::Miter, "miter"), (LineJoin::Round, "round"), (LineJoin::Bevel, "bevel")];
/// Stroke alignment names as `stroke.set` takes them.
pub const ALIGNS: [(StrokeAlign, &str); 3] = [(StrokeAlign::Center, "center"), (StrokeAlign::Inside, "inside"), (StrokeAlign::Outside, "outside")];

fn name_of<T: PartialEq>(table: &[(T, &'static str)], v: T) -> &'static str {
    table.iter().find(|(k, _)| *k == v).map_or("", |(_, n)| n)
}

/// The stroke an object's Stroke panel options describe: type's first run's character stroke,
/// else the topmost stroke of its own (groups and layers too). None without a stroke.
pub fn node_stroke(n: &Node) -> Option<StrokeLayer> {
    match &n.kind {
        NodeKind::Text(t) => Some(t.runs.first().map_or_else(|| vectorcraft_doc::CharStyle::default().stroke_layer(), |r| r.style.stroke_layer())),
        _ => n.appearance.stroke().cloned(),
    }
}

/// A stroke's options in `stroke.set` terms: cap, join, miterLimit, align, dash (null when
/// solid), dashOffset, alignDashes, startArrow/endArrow (null for none), arrowAlign and profile,
/// plus widthPoints: the profile's `[t, left, right]` points (width factors; null when uniform)
/// and gradientMode: how a gradient lies on it (`paint.editGradient` strokeMode).
pub fn stroke_options(st: &StrokeLayer) -> Value {
    json!({
        "cap": name_of(&CAPS, st.cap),
        "join": name_of(&JOINS, st.join),
        "miterLimit": st.miter_limit,
        "align": name_of(&ALIGNS, st.align),
        "dash": st.dash.as_ref().map(|d| &d.pattern),
        "dashOffset": st.dash.as_ref().map_or(0.0, |d| d.offset),
        "alignDashes": st.dash.as_ref().is_some_and(|d| d.align_corners),
        "startArrow": st.start_arrow,
        "endArrow": st.end_arrow,
        "arrowAlign": if st.arrow_align == ArrowAlign::Tip { "tip" } else { "extend" },
        "profile": WidthProfile::id_of(st.profile.as_ref()),
        "widthPoints": st.profile.as_ref().map(|p| &p.points),
        "gradientMode": st.gradient_mode.name(),
    })
}

/// Which Stroke panel values the selected objects don't share (their fields show blank), and
/// whether Align Stroke applies to them (no open path or type among them).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StrokeMixed {
    pub weight: bool,
    pub cap: bool,
    pub join: bool,
    pub miter_limit: bool,
    pub align: bool,
    pub dash: bool,
    pub can_align: bool,
}

/// `document.semantic_summary`: default number of palette entries returned.
pub const SEMANTIC_PALETTE_DEFAULT: u64 = 32;
/// `document.semantic_summary`: most palette entries a caller may ask for (`maxPalette` is capped).
pub const SEMANTIC_PALETTE_MAX: u64 = 256;
/// `document.semantic_summary`: object ids listed per palette entry (`count` still counts all).
pub const SEMANTIC_SAMPLE_IDS: usize = 5;
/// `document.semantic_summary`: deepest container level walked into. A hostile file can nest
/// thousands of groups; the walk uses an explicit stack, and below this depth it stops and
/// reports `depthCapped: true` rather than spending unbounded time and memory.
pub const SEMANTIC_MAX_DEPTH: usize = 512;

fn paint_hex(p: &vectorcraft_color::Paint) -> Option<String> {
    if p.is_none() { None } else { Some(p.label()) }
}

/// A finite number as JSON, else an explicit `null` (no NaN or infinity reaches the reply).
fn finite_json(v: f64) -> Value {
    if v.is_finite() { json!(v) } else { Value::Null }
}

/// Semantic summary of the document for agent consumption (`document.semantic_summary`).
///
/// Params: `selectionOnly?: bool` (walk only the selected objects' subtrees), `maxPalette?:
/// int` (default [`SEMANTIC_PALETTE_DEFAULT`], capped at [`SEMANTIC_PALETTE_MAX`]). A param of
/// the wrong type is a `BadParams` error, never silently reinterpreted.
///
/// The whole layer tree is walked (sublayers, groups, clip groups, compound paths, blends,
/// envelopes, repeats), up to [`SEMANTIC_MAX_DEPTH`] levels. What the keys count, within the
/// walked scope:
/// - `groupCount`: visible groups (`NodeKind::Group`, clip groups included). Layers and
///   sublayers are not groups; compound paths, blends, envelopes and repeats are walked into
///   but are not groups either.
/// - `loosePathCount`: visible paths (`NodeKind::Path`) not inside any group or other
///   non-layer container: direct children of a layer or sublayer. A path inside a group, a
///   compound path, etc. is not loose, at any depth.
/// - `hiddenCount`: objects and layers whose own visibility is off (plus selected roots under a
///   hidden ancestor). Their subtrees are skipped: nothing in them counts anywhere else.
/// - `totalNodes`: every visible non-layer object counted (containers included).
/// - `palette`: fills of the visible objects as `{hex, count, sampleIds}`, most used first
///   (ties by `hex` ascending), at most `maxPalette` entries; `sampleIds` holds at most
///   [`SEMANTIC_SAMPLE_IDS`] ids, bottom of the stack first.
/// - `backgroundCardId`: the bottom object of a visible top-level layer when it is visible, in
///   scope and its bounds are within 2% of the first artboard's size and origin; else `null`.
/// - `artboard`: the first artboard's `{width, height}` (each `null` when not finite), or
///   `null` without artboards; `artboardCount`: how many there are.
/// - `depthCapped`: true when a subtree went deeper than [`SEMANTIC_MAX_DEPTH`] and was not
///   walked below it.
pub fn semantic_summary(s: &Session, p: &Value) -> Result<Value, crate::EngineError> {
    const CMD: &str = "document.semantic_summary";
    let selection_only = match p.get("selectionOnly") {
        None | Some(Value::Null) => false,
        Some(v) => v.as_bool().ok_or_else(|| crate::cmd::bad(CMD, "`selectionOnly` must be a boolean"))?,
    };
    let max_palette = match p.get("maxPalette") {
        None | Some(Value::Null) => SEMANTIC_PALETTE_DEFAULT,
        Some(v) => v.as_u64().ok_or_else(|| crate::cmd::bad(CMD, "`maxPalette` must be a non-negative integer"))?,
    }
    .min(SEMANTIC_PALETTE_MAX) as usize;

    let st = s.doc()?;
    let doc = &st.doc;
    let selected: std::collections::HashSet<vectorcraft_doc::NodeId> = st.selection.objects.iter().copied().collect();

    let mut total_nodes = 0u64;
    let mut group_count = 0u64;
    let mut loose_paths = 0u64;
    let mut hidden_count = 0u64;
    let mut depth_capped = false;
    let mut palette_map: std::collections::HashMap<String, (u64, Vec<u64>)> = std::collections::HashMap::new();

    struct Item<'a> {
        node: &'a Node,
        depth: usize,
        /// Inside a group or other non-layer container.
        in_group: bool,
        /// Inside the walked scope (always, unless `selectionOnly`: inside a selected root).
        counting: bool,
        /// Under a hidden ancestor.
        hidden: bool,
    }
    // Explicit stack (no recursion): children are pushed in reverse, so the bottom of the
    // stack pops first and sample ids come out in stacking order.
    let mut stack: Vec<Item> =
        doc.layers.iter().rev().map(|l| Item { node: l, depth: 0, in_group: false, counting: !selection_only, hidden: false }).collect();
    while let Some(item) = stack.pop() {
        let n = item.node;
        let root = selection_only && !item.counting && selected.contains(&n.id);
        let counting = item.counting || root;
        let hidden = item.hidden || !n.visible;
        if hidden {
            // Count the first hidden node on the way down (or a selected root under a hidden
            // ancestor) once and skip its subtree. Outside the scope the walk goes on below it,
            // hidden, only to find selected roots.
            if counting {
                if !item.hidden || root {
                    hidden_count += 1;
                }
                continue;
            }
        } else if counting && !n.is_layer() {
            total_nodes += 1;
            match &n.kind {
                NodeKind::Group { .. } => group_count += 1,
                NodeKind::Path { .. } if !item.in_group => loose_paths += 1,
                _ => {}
            }
            let fill_opt = paint_hex(&n.appearance.fill_paint()).or_else(|| {
                if let NodeKind::Text(t) = &n.kind {
                    let default_style = vectorcraft_doc::CharStyle::default();
                    let style_fill = &t.runs.first().map_or(&default_style, |r| &r.style).fill;
                    paint_hex(style_fill)
                } else {
                    None
                }
            });
            if let Some(label) = fill_opt {
                let entry = palette_map.entry(label).or_insert((0, Vec::new()));
                entry.0 += 1;
                if entry.1.len() < SEMANTIC_SAMPLE_IDS {
                    entry.1.push(n.id.0);
                }
            }
        }
        if let Some(children) = n.children() {
            if item.depth >= SEMANTIC_MAX_DEPTH {
                depth_capped = true;
                continue;
            }
            let in_group = item.in_group || !n.is_layer();
            stack.extend(children.iter().rev().map(|c| Item { node: c, depth: item.depth + 1, in_group, counting, hidden }));
        }
    }

    let mut palette: Vec<(String, (u64, Vec<u64>))> = palette_map.into_iter().collect();
    palette.sort_by(|a, b| b.1.0.cmp(&a.1.0).then_with(|| a.0.cmp(&b.0)));
    let palette: Vec<Value> =
        palette.into_iter().take(max_palette).map(|(hex, (count, ids))| json!({ "hex": hex, "count": count, "sampleIds": ids })).collect();

    let artboard_rect = doc.artboards.first().map(|a| a.rect);
    let mut bg_card_id = None;
    if let Some(ab) = artboard_rect
        && ab.width() > 0.0
        && ab.height() > 0.0
    {
        for layer in doc.layers.iter().filter(|l| l.visible) {
            let Some(child) = layer.children().and_then(|c| c.first()) else { continue };
            if !child.visible || child.is_layer() || (selection_only && !selected.contains(&child.id)) {
                continue;
            }
            let Some(gb) = child.geometric_bounds() else { continue };
            let dw = (ab.width() - gb.width()).abs();
            let dh = (ab.height() - gb.height()).abs();
            let dx = (ab.x0 - gb.x0).abs();
            let dy = (ab.y0 - gb.y0).abs();
            if dw.is_finite()
                && dh.is_finite()
                && dx.is_finite()
                && dy.is_finite()
                && dw <= ab.width() * 0.02
                && dh <= ab.height() * 0.02
                && dx <= ab.width() * 0.02
                && dy <= ab.height() * 0.02
            {
                bg_card_id = Some(child.id.0);
                break;
            }
        }
    }

    Ok(json!({
        "artboard": match artboard_rect {
            Some(r) => json!({ "width": finite_json(r.width()), "height": finite_json(r.height()) }),
            None => Value::Null,
        },
        "artboardCount": doc.artboards.len(),
        "backgroundCardId": bg_card_id,
        "totalNodes": total_nodes,
        "groupCount": group_count,
        "loosePathCount": loose_paths,
        "hiddenCount": hidden_count,
        "depthCapped": depth_capped,
        "palette": palette,
    }))
}
