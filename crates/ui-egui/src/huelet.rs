//! Native Huelet colour wheel. Tool colours remain the engine's encoded display RGB;
//! Lab/CMYK use PhotoCraft's existing conversions and current colour-management context.
use egui::{Color32, Key, Mesh, Pos2, Rect, Sense, Stroke, StrokeKind, pos2, vec2};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{
    PhotocraftApp,
    color_picker_ui::{hex, hsv_to_rgb, parse_hex, rgb_to_hsv},
    theme::Tokens,
    widgets,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Shape {
    #[default]
    Triangle,
    Square,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Model {
    #[default]
    Hsv,
    Rgb,
    Lab,
    Cmyk,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Harmony {
    #[default]
    Free,
    Analogous,
    Complementary,
    Split,
    Triad,
    Tetradic,
    Square,
}
impl Harmony {
    fn offsets(self, angle: f32) -> Vec<f32> {
        match self {
            Self::Free => vec![0.0],
            Self::Analogous => vec![0.0, -angle, angle],
            Self::Complementary => vec![0.0, 180.0],
            Self::Split => vec![0.0, 180.0 - angle, 180.0 + angle],
            Self::Triad => vec![0.0, 120.0, 240.0],
            Self::Tetradic => vec![0.0, angle, 180.0, 180.0 + angle],
            Self::Square => vec![0.0, 90.0, 180.0, 270.0],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Scheme {
    pub mode: Harmony,
    pub base: f32,
    pub offsets: Vec<f32>,
    pub active: usize,
    pub linked: bool,
    pub angle: f32,
}
impl Default for Scheme {
    fn default() -> Self {
        Self { mode: Harmony::Free, base: 0.0, offsets: vec![0.0], active: 0, linked: true, angle: 30.0 }
    }
}
impl Scheme {
    fn hues(&self) -> Vec<f32> {
        self.offsets.iter().map(|offset| hue(self.base + offset)).collect()
    }
    fn reset(&mut self, h: f32) {
        self.base = h;
        self.offsets = self.mode.offsets(self.angle);
        self.active = 0;
    }
    fn move_hue(&mut self, h: f32) {
        let offset = self.offsets.get(self.active).copied().unwrap_or(0.0);
        if self.linked {
            self.base = hue(h - offset);
        } else if self.active == 0 {
            let delta = h - self.base;
            for offset in self.offsets.iter_mut().skip(1) {
                *offset = hue(*offset - delta);
            }
            self.base = h;
        } else if let Some(offset) = self.offsets.get_mut(self.active) {
            *offset = hue(h - self.base);
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Editor {
    pub memory: [f32; 2],
    pub scheme: Scheme,
    #[serde(skip)]
    pub seen: Option<[f32; 4]>,
    #[serde(skip)]
    pub channel_edit: Option<(Model, [f32; 4])>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
enum Drag {
    Ring,
    Field,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub background: bool,
    pub shape: Shape,
    pub model: Model,
    pub editors: [Editor; 2],
    pub favorites: Vec<String>,
    pub recent: Vec<String>,
    pub show_harmony: bool,
    pub show_favorites: bool,
    pub show_recent: bool,
    #[serde(skip)]
    drag: Option<Drag>,
    #[serde(skip)]
    pending_recent: Option<[f32; 3]>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            background: false,
            shape: Shape::Triangle,
            model: Model::Hsv,
            editors: Default::default(),
            favorites: ["#EE704C", "#F6CC83", "#A9BCAF", "#7E91B2", "#C1A4BF", "#ECE6DA"].map(str::to_owned).to_vec(),
            recent: Vec::new(),
            show_harmony: true,
            show_favorites: true,
            show_recent: true,
            drag: None,
            pending_recent: None,
        }
    }
}
fn hue(h: f32) -> f32 {
    if h.is_finite() { h.rem_euclid(360.0) } else { 0.0 }
}
fn unit(v: f32) -> f32 {
    if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 }
}
impl State {
    pub(crate) fn sanitize(&mut self) {
        for editor in &mut self.editors {
            editor.memory = [hue(editor.memory[0]), unit(editor.memory[1])];
            let scheme = &mut editor.scheme;
            scheme.base = hue(scheme.base);
            scheme.angle = if scheme.angle.is_finite() { scheme.angle.clamp(5.0, 175.0) } else { 30.0 };
            if scheme.offsets.len() != scheme.mode.offsets(scheme.angle).len() || scheme.offsets.iter().any(|v| !v.is_finite()) {
                scheme.reset(editor.memory[0]);
            }
            scheme.active = scheme.active.min(scheme.offsets.len().saturating_sub(1));
        }
        for (list, cap) in [(&mut self.favorites, 60), (&mut self.recent, 20)] {
            *list = list.iter().filter_map(|s| parse_hex(s).map(hex)).take(cap).fold(Vec::new(), |mut out, s| {
                if !out.contains(&s) {
                    out.push(s);
                }
                out
            });
        }
    }
    fn editor(&mut self) -> &mut Editor {
        if self.background { &mut self.editors[1] } else { &mut self.editors[0] }
    }
}

const PREF: &str = "huelet";
pub fn restore(app: &mut PhotocraftApp) {
    if let Some(value) = app.session.prefs().dialogs.get(PREF)
        && let Ok(mut state) = serde_json::from_value::<State>(value.clone())
    {
        state.sanitize();
        app.ui.huelet = state;
    }
}
pub fn persist(app: &mut PhotocraftApp, ctx: &egui::Context) {
    if ctx.input(|i| i.pointer.any_down()) {
        return;
    }
    if !app.session.prefs().dialogs.contains_key(PREF) && app.ui.huelet == State::default() {
        return;
    }
    if let Ok(value) = serde_json::to_value(&app.ui.huelet)
        && app.session.prefs().dialogs.get(PREF) != Some(&value)
    {
        app.session.prefs.edit(|p| {
            p.dialogs.insert(PREF.into(), value);
        });
    }
}

fn color(app: &PhotocraftApp, background: bool) -> [f32; 4] {
    if background { app.session.tools.background } else { app.session.tools.foreground }
}
fn write_color(app: &mut PhotocraftApp, background: bool, rgb: [f32; 3]) -> bool {
    let old = color(app, background);
    let [r, g, b] = rgb.map(unit);
    let new = [r, g, b, old[3]];
    if new == old {
        return false;
    }
    let params = if background { json!({"background":new}) } else { json!({"foreground":new}) };
    match app.run("tools.setColors", params) {
        Ok(_) => {
            if !background {
                crate::type_tool::foreground_changed(app);
            }
            true
        }
        Err(error) => {
            app.ui.status = error;
            false
        }
    }
}
fn read_hsv(editor: &mut Editor, c: [f32; 4]) -> [f32; 3] {
    let mut hsv = rgb_to_hsv([c[0], c[1], c[2]].map(unit));
    if hsv[1] < 1e-6 {
        hsv[0] = editor.memory[0];
    }
    if hsv[2] < 1e-6 {
        hsv[1] = editor.memory[1];
    }
    if editor.seen != Some(c) {
        if editor.seen.is_some() {
            editor.scheme.reset(hsv[0]);
        } else {
            // A restored scheme has no runtime `seen` color yet. Keep its custom spacing.
            editor.scheme.move_hue(hsv[0]);
        }
        editor.channel_edit = None;
        editor.seen = Some(c);
    }
    editor.memory = [hsv[0], hsv[1]];
    hsv
}
fn commit(app: &mut PhotocraftApp, state: &mut State, hsv: [f32; 3]) {
    state.editor().scheme.move_hue(hsv[0]);
    let rgb = hsv_to_rgb(hsv[0], hsv[1], hsv[2]);
    write_color(app, state.background, rgb);
    let c = color(app, state.background);
    let editor = state.editor();
    editor.memory = [hsv[0], hsv[1]];
    editor.seen = Some(c);
}
fn recent(state: &mut State, rgb: [f32; 3]) {
    let s = hex(rgb);
    state.recent.retain(|v| v != &s);
    state.recent.insert(0, s);
    state.recent.truncate(20);
}
fn c32(rgb: [f32; 3]) -> Color32 {
    let [r, g, b] = rgb.map(|v| (unit(v) * 255.0).round() as u8);
    Color32::from_rgb(r, g, b)
}
fn ring_point(center: Pos2, radius: f32, h: f32) -> Pos2 {
    let a = (h - 90.0).to_radians();
    center + vec2(a.cos(), a.sin()) * radius
}
fn triangle(center: Pos2, radius: f32) -> [Pos2; 3] {
    [0.0_f32, 240.0, 120.0].map(|a| center + vec2(a.to_radians().cos(), a.to_radians().sin()) * radius)
}
fn weights(p: Pos2, [a, b, c]: [Pos2; 3]) -> [f32; 3] {
    let denom = (b.y - c.y) * (a.x - c.x) + (c.x - b.x) * (a.y - c.y);
    if denom.abs() < f32::EPSILON {
        return [0.0, 0.0, 1.0];
    }
    let u = ((b.y - c.y) * (p.x - c.x) + (c.x - b.x) * (p.y - c.y)) / denom;
    let v = ((c.y - a.y) * (p.x - c.x) + (a.x - c.x) * (p.y - c.y)) / denom;
    [u, v, 1.0 - u - v]
}
fn triangle_sv(p: Pos2, vertices: [Pos2; 3], previous_s: f32) -> [f32; 2] {
    let mut w = weights(p, vertices);
    if w.iter().any(|v| *v < 0.0) {
        let [a, b, c] = vertices;
        let mut best = a;
        let mut distance = f32::INFINITY;
        for (a, b) in [(a, b), (b, c), (c, a)] {
            let edge = b - a;
            let t = ((p - a).dot(edge) / edge.length_sq().max(f32::EPSILON)).clamp(0.0, 1.0);
            let point = a + edge * t;
            let d = point.distance_sq(p);
            if d < distance {
                distance = d;
                best = point;
            }
        }
        w = weights(best, vertices);
    }
    let [pure, white, _] = w.map(unit);
    let v = unit(pure + white);
    [if v < 1e-6 { previous_s } else { unit(pure / v) }, v]
}
fn triangle_pos([s, v]: [f32; 2], [pure, white, black]: [Pos2; 3]) -> Pos2 {
    pos2(pure.x * s * v + white.x * (1.0 - s) * v + black.x * (1.0 - v), pure.y * s * v + white.y * (1.0 - s) * v + black.y * (1.0 - v))
}

fn wheel(ui: &mut egui::Ui, state: &mut State, hsv: &mut [f32; 3]) -> bool {
    let t = Tokens::get(ui.ctx());
    let size = ui.available_width().clamp(80.0, 340.0);
    let (rect, response) = ui.allocate_exact_size(vec2(size, size), Sense::click_and_drag());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "Huelet color wheel"));
    let center = rect.center();
    let outer = size * 0.46;
    let inner = size * 0.385;
    let field_radius = size * 0.35;
    let vertices = triangle(center, field_radius);
    let square = Rect::from_center_size(center, vec2(1.35 * field_radius, 1.35 * field_radius));
    let mut ring = Mesh::default();
    for i in 0..=120 {
        let h = i as f32 * 3.0;
        let color = c32(hsv_to_rgb(h, 1.0, 1.0));
        ring.colored_vertex(ring_point(center, outer, h), color);
        ring.colored_vertex(ring_point(center, inner, h), color);
        if i < 120 {
            let a = i * 2;
            ring.add_triangle(a, a + 1, a + 3);
            ring.add_triangle(a, a + 3, a + 2);
        }
    }
    ui.painter().add(ring);
    let mut field = Mesh::default();
    match state.shape {
        Shape::Triangle => {
            let [pure, white, black] = vertices;
            field.colored_vertex(pure, c32(hsv_to_rgb(hsv[0], 1.0, 1.0)));
            field.colored_vertex(white, c32([1.0; 3]));
            field.colored_vertex(black, c32([0.0; 3]));
            field.add_triangle(0, 1, 2);
        }
        Shape::Square => {
            for y in 0..=16 {
                for x in 0..=16 {
                    let s = x as f32 / 16.0;
                    let v = 1.0 - y as f32 / 16.0;
                    field.colored_vertex(pos2(square.left() + s * square.width(), square.top() + (1.0 - v) * square.height()), c32(hsv_to_rgb(hsv[0], s, v)));
                    if y < 16 && x < 16 {
                        let a = y * 17 + x;
                        field.add_triangle(a, a + 1, a + 18);
                        field.add_triangle(a, a + 18, a + 17);
                    }
                }
            }
        }
    }
    ui.painter().add(field);
    let mut changed = false;
    if response.drag_started() || response.clicked() {
        response.request_focus();
        if let Some(p) = response.interact_pointer_pos() {
            let d = p.distance(center);
            state.drag = if d >= inner && d <= outer + 6.0 {
                Some(Drag::Ring)
            } else if match state.shape {
                Shape::Triangle => weights(p, vertices).iter().all(|w| *w >= 0.0),
                Shape::Square => square.contains(p),
            } {
                Some(Drag::Field)
            } else {
                None
            };
            if state.drag == Some(Drag::Ring) {
                let hues = state.editor().scheme.hues();
                if let Some((index, _)) = hues
                    .iter()
                    .enumerate()
                    .map(|(i, h)| (i, p.distance(ring_point(center, (inner + outer) * 0.5, *h))))
                    .filter(|(_, d)| *d < 12.0)
                    .min_by(|a, b| a.1.total_cmp(&b.1))
                {
                    state.editor().scheme.active = index;
                }
            }
        }
    }
    if (response.dragged() || response.clicked())
        && let Some(p) = response.interact_pointer_pos()
    {
        match state.drag {
            Some(Drag::Ring) => {
                hsv[0] = hue((p.y - center.y).atan2(p.x - center.x).to_degrees() + 90.0);
                state.editor().scheme.move_hue(hsv[0]);
                changed = true;
            }
            Some(Drag::Field) => {
                let [s, v] = match state.shape {
                    Shape::Triangle => triangle_sv(p, vertices, hsv[1]),
                    Shape::Square => [unit((p.x - square.left()) / square.width()), unit(1.0 - (p.y - square.top()) / square.height())],
                };
                hsv[1] = s;
                hsv[2] = v;
                changed = true;
            }
            None => {}
        }
    }
    if !ui.input(|i| i.pointer.primary_down()) {
        state.drag = None;
    }
    if response.has_focus() {
        ui.input(|i| {
            let step = if i.modifiers.shift { 10.0 } else { 1.0 };
            let dh = if i.key_pressed(Key::ArrowRight) {
                step
            } else if i.key_pressed(Key::ArrowLeft) {
                -step
            } else {
                0.0
            };
            let dv = if i.key_pressed(Key::ArrowUp) {
                step / 100.0
            } else if i.key_pressed(Key::ArrowDown) {
                -step / 100.0
            } else {
                0.0
            };
            if dh != 0.0 || dv != 0.0 {
                hsv[0] = hue(hsv[0] + dh);
                if i.modifiers.alt {
                    hsv[1] = unit(hsv[1] + dv);
                } else {
                    hsv[2] = unit(hsv[2] + dv);
                }
                state.editor().scheme.move_hue(hsv[0]);
                changed = true;
            }
        });
    }
    let scheme = &state.editor().scheme;
    for (index, h) in scheme.hues().into_iter().enumerate() {
        let p = ring_point(center, (inner + outer) * 0.5, h);
        ui.painter().circle_filled(p, if index == scheme.active { 6.0 } else { 4.0 }, c32(hsv_to_rgb(h, hsv[1], hsv[2])));
        ui.painter().circle_stroke(p, if index == scheme.active { 7.0 } else { 5.0 }, Stroke::new(1.5, t.text));
    }
    let marker = match state.shape {
        Shape::Triangle => triangle_pos([hsv[1], hsv[2]], vertices),
        Shape::Square => pos2(square.left() + hsv[1] * square.width(), square.top() + (1.0 - hsv[2]) * square.height()),
    };
    ui.painter().circle_stroke(marker, 5.0, Stroke::new(2.0, t.text));
    ui.painter().circle_stroke(marker, 7.0, Stroke::new(1.0, t.card));
    changed
}

fn channels(model: Model, hsv: [f32; 3], rgb: [f32; 3]) -> [f32; 4] {
    match model {
        Model::Hsv => [hsv[0], hsv[1] * 100.0, hsv[2] * 100.0, 0.0],
        Model::Rgb => [rgb[0] * 255.0, rgb[1] * 255.0, rgb[2] * 255.0, 0.0],
        Model::Lab => {
            let [l, a, b] = photocraft_color::convert::srgb_to_lab(rgb);
            [l, a, b, 0.0]
        }
        Model::Cmyk => photocraft_color::convert::rgb_to_cmyk(rgb).map(|v| v * 100.0),
    }
}
fn channel_rgb(model: Model, values: [f32; 4]) -> [f32; 3] {
    let [a, b, c, _] = values;
    match model {
        Model::Hsv => hsv_to_rgb(a, b / 100.0, c / 100.0),
        Model::Rgb => [a / 255.0, b / 255.0, c / 255.0],
        Model::Lab => photocraft_color::convert::lab_to_srgb([a, b, c]),
        Model::Cmyk => photocraft_color::convert::cmyk_to_rgb(values.map(|v| v / 100.0)),
    }
    .map(unit)
}
fn sliders(ui: &mut egui::Ui, state: &mut State, hsv: [f32; 3], rgb: [f32; 3]) -> Option<[f32; 3]> {
    ui.horizontal_wrapped(|ui| {
        for (model, label) in [(Model::Hsv, "HSV"), (Model::Rgb, "RGB"), (Model::Lab, "LAB"), (Model::Cmyk, "CMYK")] {
            if ui.selectable_value(&mut state.model, model, label).changed() {
                state.editor().channel_edit = None;
            }
        }
    });
    let model = state.model;
    let mut values = state.editor().channel_edit.filter(|(m, _)| *m == model).map(|(_, v)| v).unwrap_or_else(|| channels(model, hsv, rgb));
    let rows: &[(&str, f32, f32)] = match model {
        Model::Hsv => &[("Hue", 0.0, 360.0), ("Saturation", 0.0, 100.0), ("Brightness", 0.0, 100.0)],
        Model::Rgb => &[("R", 0.0, 255.0), ("G", 0.0, 255.0), ("B", 0.0, 255.0)],
        Model::Lab => &[("L", 0.0, 100.0), ("a", -128.0, 127.0), ("b", -128.0, 127.0)],
        Model::Cmyk => &[("C", 0.0, 100.0), ("M", 0.0, 100.0), ("Y", 0.0, 100.0), ("K", 0.0, 100.0)],
    };
    let mut changed = false;
    for (index, (label, min, max)) in rows.iter().enumerate() {
        let Some(value) = values.get_mut(index) else { continue };
        let mut stops = Vec::new();
        for k in 0..=16 {
            let mut v = state.editor().channel_edit.filter(|(m, _)| *m == model).map(|(_, v)| v).unwrap_or_else(|| channels(model, hsv, rgb));
            if let Some(slot) = v.get_mut(index) {
                *slot = min + (max - min) * k as f32 / 16.0;
            }
            stops.push(c32(channel_rgb(model, v)));
        }
        changed |= widgets::slider_row(ui, label, value, *min..=*max, "", Some(&stops)).changed();
    }
    if changed {
        state.editor().channel_edit = Some((model, values));
        Some(channel_rgb(model, values))
    } else {
        None
    }
}
fn swatch(ui: &mut egui::Ui, value: &str, selected: bool) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let (rect, response) = ui.allocate_exact_size(vec2(25.0, 25.0), Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, value));
    ui.painter().rect_filled(rect.shrink(2.0), t.radius_sm, parse_hex(value).map(c32).unwrap_or(t.field));
    ui.painter().rect_stroke(
        rect.shrink(1.0),
        t.radius_sm,
        Stroke::new(if selected { 2.0 } else { 1.0 }, if selected { t.accent } else { t.field_border }),
        StrokeKind::Inside,
    );
    response.on_hover_text(value)
}
fn palette(ui: &mut egui::Ui, colors: &mut Vec<String>, removable: bool) -> Option<[f32; 3]> {
    let mut chosen = None;
    let mut remove = None;
    ui.horizontal_wrapped(|ui| {
        for (index, value) in colors.iter().enumerate() {
            let response = swatch(ui, value, false);
            if response.clicked() {
                response.request_focus();
                chosen = parse_hex(value);
            }
            if removable {
                response.context_menu(|ui| {
                    if ui.button(tl!("Delete")).clicked() {
                        remove = Some(index);
                        ui.close();
                    }
                });
                if response.has_focus() && ui.input(|i| i.key_pressed(Key::Delete)) {
                    remove = Some(index);
                }
            }
        }
    });
    if let Some(index) = remove {
        colors.remove(index);
    }
    chosen
}

pub fn show(app: &mut PhotocraftApp, ui: &mut egui::Ui) {
    let mut state = app.ui.huelet.clone();
    state.sanitize();
    ui.spacing_mut().item_spacing = vec2(5.0, 5.0);
    ui.horizontal_wrapped(|ui| {
        let fg = hex([app.session.tools.foreground[0], app.session.tools.foreground[1], app.session.tools.foreground[2]]);
        let bg = hex([app.session.tools.background[0], app.session.tools.background[1], app.session.tools.background[2]]);
        if swatch(ui, &fg, !state.background).on_hover_text(tl!("Foreground Color")).clicked() {
            state.background = false;
        }
        if swatch(ui, &bg, state.background).on_hover_text(tl!("Background Color")).clicked() {
            state.background = true;
        }
        if ui.button(tl!("Switch Colors")).clicked() {
            if let Err(error) = app.run("tools.swapColors", json!({})) {
                app.ui.status = error;
            }
            state.editors.swap(0, 1);
        }
        ui.menu_button(tl!("Panel Options"), |ui| {
            ui.checkbox(&mut state.show_harmony, tl!("Color Harmony"));
            ui.checkbox(&mut state.show_favorites, tl!("Swatches"));
            ui.checkbox(&mut state.show_recent, tl!("Recent Colors"));
        });
    });
    let initial_color = color(app, state.background);
    let mut hsv = read_hsv(state.editor(), initial_color);
    let before = hsv;
    if wheel(ui, &mut state, &mut hsv) {
        state.editor().channel_edit = None;
        commit(app, &mut state, hsv);
    }
    ui.horizontal(|ui| {
        ui.selectable_value(&mut state.shape, Shape::Triangle, tl!("Triangle"));
        ui.selectable_value(&mut state.shape, Shape::Square, tl!("Square"));
    });
    let c = color(app, state.background);
    let rgb = [c[0], c[1], c[2]];
    ui.horizontal_wrapped(|ui| {
        let label = ui.label("HEX");
        let id = ui.id().with(("huelet-hex", state.background));
        let mut value: String = ui.data(|d| d.get_temp(id)).unwrap_or_else(|| hex(rgb));
        let response = ui.add(egui::TextEdit::singleline(&mut value).id(id).desired_width(76.0)).labelled_by(label.id);
        if response.changed()
            && let Some(new) = parse_hex(&value)
        {
            write_color(app, state.background, new);
            let c = color(app, state.background);
            hsv = read_hsv(state.editor(), c);
        }
        if !response.has_focus() {
            let c = color(app, state.background);
            value = hex([c[0], c[1], c[2]]);
        }
        ui.data_mut(|d| d.insert_temp(id, value));
        if ui.button(tl!("Copy")).clicked() {
            ui.ctx().copy_text(hex(rgb));
        }
        if ui.button(tl!("Add Swatch")).clicked() {
            let value = hex(rgb);
            if !state.favorites.contains(&value) && state.favorites.len() < 60 {
                state.favorites.push(value);
            }
        }
    });
    let c = color(app, state.background);
    let rgb = [c[0], c[1], c[2]];
    if let Some(rgb) = sliders(ui, &mut state, hsv, rgb) {
        let mut next = rgb_to_hsv(rgb);
        if state.model == Model::Hsv
            && let Some((_, values)) = state.editor().channel_edit
        {
            next = [hue(values[0]), unit(values[1] / 100.0), unit(values[2] / 100.0)];
        } else if next[1] < 1e-6 {
            next[0] = hsv[0];
            if next[2] < 1e-6 {
                next[1] = hsv[1];
            }
        }
        commit(app, &mut state, next);
        hsv = next;
    }
    let mut chosen = None;
    if state.show_harmony {
        ui.separator();
        ui.label(tl!("Color Harmony"));
        let scheme = &mut state.editor().scheme;
        let old = scheme.mode;
        widgets::dropdown(
            ui,
            "huelet-harmony",
            &mut scheme.mode,
            &[
                (Harmony::Free, "Free"),
                (Harmony::Analogous, "Analogous"),
                (Harmony::Complementary, "Complementary"),
                (Harmony::Split, "Split Complementary"),
                (Harmony::Triad, "Triadic"),
                (Harmony::Tetradic, "Tetradic"),
                (Harmony::Square, "Square"),
            ],
            ui.available_width().min(220.0),
        );
        if scheme.mode != old {
            scheme.angle = if scheme.mode == Harmony::Tetradic { 60.0 } else { 30.0 };
            scheme.reset(hsv[0]);
        }
        if scheme.mode != Harmony::Free {
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut scheme.linked, tl!("Linked"));
                if ui.button(tl!("Reset")).clicked() {
                    scheme.reset(hsv[0]);
                }
            });
            if matches!(scheme.mode, Harmony::Analogous | Harmony::Split | Harmony::Tetradic) {
                let max = if scheme.mode == Harmony::Tetradic { 175.0 } else { 90.0 };
                if widgets::slider_row(ui, tl!("Angle"), &mut scheme.angle, 5.0..=max, "°", None).changed() {
                    scheme.offsets = scheme.mode.offsets(scheme.angle);
                    scheme.base = hue(hsv[0] - scheme.offsets.get(scheme.active).copied().unwrap_or(0.0));
                }
            }
            ui.horizontal_wrapped(|ui| {
                for (index, h) in scheme.hues().into_iter().enumerate() {
                    if swatch(ui, &hex(hsv_to_rgb(h, hsv[1], hsv[2])), index == scheme.active).clicked() {
                        scheme.active = index;
                        hsv[0] = h;
                    }
                }
            });
        }
    }
    if state.show_favorites {
        ui.separator();
        ui.label(tl!("Swatches"));
        chosen = palette(ui, &mut state.favorites, true);
    }
    if state.show_recent {
        ui.separator();
        ui.label(tl!("Recent Colors"));
        chosen = palette(ui, &mut state.recent, false).or(chosen);
    }
    if let Some(rgb) = chosen {
        write_color(app, state.background, rgb);
        let c = color(app, state.background);
        read_hsv(state.editor(), c);
    } else if hsv != before {
        commit(app, &mut state, hsv);
    }
    let c = color(app, state.background);
    if c != initial_color {
        state.pending_recent = Some([c[0], c[1], c[2]]);
    }
    if !ui.input(|i| i.pointer.any_down())
        && let Some(rgb) = state.pending_recent.take()
    {
        recent(&mut state, rgb);
    }
    app.ui.huelet = state;
    persist(app, ui.ctx());
}

#[cfg(test)]
#[path = "huelet_tests.rs"]
mod tests;
