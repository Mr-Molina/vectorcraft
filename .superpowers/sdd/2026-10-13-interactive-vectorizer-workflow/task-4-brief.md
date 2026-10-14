# Task 4 Brief: Interactive Panel, Live Drag Loop & Control Bar (`crates/ui-egui`)

## Requirements & Scope
- **Files to Modify:**
  - `crates/ui-egui/src/panels/image_trace.rs`
  - `crates/ui-egui/src/panels/properties.rs`
- **Files for Tests:**
  - `crates/ui-egui/src/panels/image_trace.rs` (under `mod tests`)

## Context & Available Interfaces
- `crates/ui-egui` is Layer 6. It depends on `vectorcraft-doc` (L1), `vectorcraft-engine` (L5), `vectorcraft-render` (L3), `egui`, `serde_json`.
- `vectorcraft_doc::TraceView` is available.
- `selected_preset(app)` retrieves the active preset name of the selected trace object.
- `app.run("imageTrace.setView", json!({ "view": chosen }))` switches view mode.
- `app.run("imageTrace.make", json!({ "preset": preset, "params": st.params, "preview": bool }))` retraces with preview coalescing.
- `widgets::dropdown(ui, id, current, options, width)` renders a dropdown. Note that `dropdown` internally wraps `current` and each option with `tl!()`, so pass raw string labels to `dropdown`.
- `widgets::check(ui, label, value, enabled) -> bool` returns `true` when clicked/toggled.
- `widgets::icon_button(ui, icon, tip, active, size) -> Response` renders icon buttons (icon `"eye"` is available).

## Task Details

### 1. View Modes Catalog & Helpers in `crates/ui-egui/src/panels/image_trace.rs`
```rust
pub(crate) const VIEW_MODES: [(&str, &str); 5] = [
    ("tracingResult", "Tracing Result"),
    ("tracingResultWithOutlines", "Tracing Result with Outlines"),
    ("outlines", "Outlines"),
    ("outlinesWithSource", "Outlines with Source Image"),
    ("sourceImage", "Source Image"),
];

/// The view mode of the one selected Image Trace object.
pub(crate) fn selected_view(app: &VectorcraftApp) -> Option<String> {
    let st = app.session.active()?;
    let [id] = st.selection.objects[..] else { return None };
    let n = st.doc.node(id).filter(|n| is_trace(n))?;
    serde_json::to_value(n.trace_view()).ok()?.as_str().map(ToString::to_string)
}

/// Dropdown to select the active Image Trace View mode.
pub(crate) fn view_dropdown(app: &mut VectorcraftApp, ui: &mut Ui, current: &str, width: f32) {
    let current_label = VIEW_MODES.iter().find(|m| m.0 == current).map_or("Tracing Result", |m| m.1);
    let labels: Vec<&str> = VIEW_MODES.iter().map(|m| m.1).collect();
    if let Some(i) = widgets::dropdown(ui, "trace-view-mode", current_label, &labels, width) {
        let chosen = VIEW_MODES[i].0;
        let _ = app.run("imageTrace.setView", json!({ "view": chosen }));
    }
}
```

### 2. Update `TraceUi` State in `crates/ui-egui/src/panels/image_trace.rs`
```rust
#[derive(Clone)]
struct TraceUi {
    preset: String,
    params: Value,
    info: Option<(u64, u64, u64)>,
    synced: Value,
    view: String,
    preview: bool,
    last_drag_time: f64,
}

impl Default for TraceUi {
    fn default() -> Self {
        Self {
            preset: String::new(),
            params: Value::Null,
            info: None,
            synced: Value::Null,
            view: "tracingResult".into(),
            preview: true,
            last_drag_time: 0.0,
        }
    }
}
```
When updating `TraceUi` from stored metadata (`target(app)`):
```rust
if let Some(t) = stored.filter(|t| *t != st.synced) {
    st.preset = t["preset"].as_str().unwrap_or("Custom").to_string();
    st.params = t["params"].clone();
    if let Some(v) = t.get("view").and_then(Value::as_str) {
        st.view = v.to_string();
    }
    st.synced = t;
}
```

### 3. Update `trace()` Helper and `show()` Panel UI
Update `trace()` to take `preview: bool`:
```rust
fn trace(app: &mut VectorcraftApp, st: &mut TraceUi, preview: bool) {
    let preset = if st.preset == "Custom" { "Default" } else { st.preset.as_str() };
    match app.run("imageTrace.make", json!({ "preset": preset, "params": st.params, "preview": preview })) {
        Ok(r) => st.info = Some((r["paths"].as_u64().unwrap_or(0), r["anchors"].as_u64().unwrap_or(0), r["colors"].as_u64().unwrap_or(0))),
        Err(e) => app.ui.status = e,
    }
}
```

In `show()`:
- Add View row beside or right under Preset row:
```rust
ui.horizontal(|ui| {
    widgets::dim_label(ui, tl!("View:"));
    view_dropdown(app, ui, &st.view, 140.0);
    // Eyeball toggle button for quick source inspection
    let is_source = st.view == "sourceImage";
    if widgets::icon_button(ui, "eye", tl!("View Source Image"), is_source, 22.0).clicked() {
        let new_view = if is_source { "tracingResult" } else { "sourceImage" };
        st.view = new_view.to_string();
        let _ = app.run("imageTrace.setView", json!({ "view": new_view }));
    }
});
```

- In the slider loop (Threshold / Colors / Paths / Corners / Noise):
```rust
let now = ui.input(|i| i.time);
if changed {
    st.params[key] = ...;
    st.preset = "Custom".into();
    if st.preview && is_trace && now - st.last_drag_time >= 0.060 {
        st.last_drag_time = now;
        trace(app, &mut st, true);
    }
}
if release && is_trace {
    trace(app, &mut st, false);
}
```

- In the bottom action buttons area, add the `Preview` checkbox:
```rust
ui.horizontal(|ui| {
    if widgets::check(ui, tl!("Preview"), st.preview, true) {
        st.preview = !st.preview;
    }
    let r = ui.add_enabled_ui(is_trace || is_image, |ui| widgets::flat_button(ui, tl!("Trace"), 80.0)).inner;
    if r.on_disabled_hover_text(tl!("Select an image to trace")).clicked() {
        trace(app, &mut st, false);
    }
    if ui.add_enabled_ui(is_trace, |ui| widgets::flat_button(ui, tl!("Expand"), 80.0)).inner.clicked() {
        app.run("imageTrace.expand", json!({})).ok();
    }
});
```

- For discrete controls (mode dropdown, method, snapCurvesToLines, ignoreWhite):
```rust
if retrace && is_trace {
    trace(app, &mut st, false);
}
```

### 4. Control Bar in `crates/ui-egui/src/panels/image_trace.rs`
In `pub fn control_bar(app: &mut VectorcraftApp, ui: &mut Ui) -> bool`:
Add `View:` dropdown beside `Preset:`:
```rust
pub fn control_bar(app: &mut VectorcraftApp, ui: &mut Ui) -> bool {
    let Some(preset) = selected_preset(app) else { return false };
    widgets::dim_label(ui, tl!("Preset:"));
    preset_dropdown(app, ui, &preset, 130.0);
    let view = selected_view(app).unwrap_or_else(|| "tracingResult".into());
    widgets::dim_label(ui, tl!("View:"));
    view_dropdown(app, ui, &view, 140.0);
    if widgets::icon_button(ui, "image", tl!("Image Trace"), false, 24.0).clicked() {
        app.ui.open_panel = Some("imageTrace".into());
    }
    if widgets::flat_button(ui, tl!("Expand"), 64.0).clicked() {
        app.run("imageTrace.expand", json!({})).ok();
    }
    ui.separator();
    true
}
```

### 5. Properties Panel in `crates/ui-egui/src/panels/properties.rs`
In `fn trace_section(app: &mut VectorcraftApp, ui: &mut Ui, preset: &str)`:
```rust
fn trace_section(app: &mut VectorcraftApp, ui: &mut Ui, preset: &str) {
    section_header(ui, tl!("Image Trace"));
    ui.horizontal(|ui| {
        dim_label(ui, tl!("Preset:"));
        crate::panels::image_trace::preset_dropdown(app, ui, preset, ui.available_width());
    });
    let view = crate::panels::image_trace::selected_view(app).unwrap_or_else(|| "tracingResult".into());
    ui.horizontal(|ui| {
        dim_label(ui, tl!("View:"));
        crate::panels::image_trace::view_dropdown(app, ui, &view, ui.available_width());
    });
    let w = (ui.available_width() - 6.0) / 2.0;
    ui.horizontal(|ui| {
        for (label, id) in [(tl!("Expand"), "imageTrace.expand"), (tl!("Release"), "imageTrace.release")] {
            if widgets::flat_button(ui, label, w).clicked() {
                app.run(id, json!({})).ok();
            }
        }
    });
}
```

### 6. Headless Test in `crates/ui-egui/src/panels/image_trace.rs`
Add `test_image_trace_panel_with_view_modes_headless`:
```rust
#[test]
fn test_image_trace_panel_with_view_modes_headless() {
    let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
    app.session.execute("file.new", &json!({"width": 200, "height": 200})).unwrap();

    let img = image::RgbaImage::from_fn(20, 20, |x, y| image::Rgba([x as u8, y as u8, 0, 255]));
    let mut png = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();

    let _img_id = app.session.edit("img", |d, sel| {
        d.images.insert("img1".into(), vectorcraft_doc::ImageBlob::new("image/png", png));
        let id = d.alloc_id();
        let l = d.default_layer();
        d.insert(
            l,
            0,
            vectorcraft_doc::Node::new(
                id,
                vectorcraft_doc::NodeKind::Image(vectorcraft_doc::ImageObject {
                    key: "img1".into(),
                    width: 20,
                    height: 20,
                    xf: vectorcraft_geom::Affine::IDENTITY,
                    link: None,
                    placement: Default::default(),
                }),
            ),
        )?;
        sel.set([id]);
        Ok(id)
    }).unwrap();

    let trace_res = app.session.execute("imageTrace.make", &json!({ "preset": "Default" })).unwrap();
    let trace_id = vectorcraft_doc::NodeId(trace_res["id"].as_u64().unwrap());
    app.session.select(|_, sel| sel.set([trace_id])).unwrap();

    for _ in 0..3 {
        let ctx = egui::Context::default();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            show(&mut app, ui);
            menu(&mut app, ui);
            control_bar(&mut app, ui);
        });
        out.textures_delta.clear();
    }
}
```

### 7. Quality Gates & Commit
- `cargo test -p vectorcraft-ui-egui -- image_trace`
- `cargo clippy -p vectorcraft-ui-egui --all-targets -- -D warnings`
- `cargo fmt -p vectorcraft-ui-egui --check`
- Commit: `git commit -m "feat(ui): add View mode selector, Preview toggle, and debounced drag loop to Image Trace"`
