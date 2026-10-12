use super::*;
use egui::{Event, Modifiers, PointerButton};
use egui_kittest::{Harness, kittest::Queryable};

#[test]
fn unopened_default_panel_does_not_dirty_preferences() {
    let mut app = app();
    let before = app.session.prefs_to_json();
    let rev = app.session.prefs.rev();
    persist(&mut app, &egui::Context::default());
    assert_eq!(app.session.prefs_to_json(), before);
    assert_eq!(app.session.prefs.rev(), rev);
}

#[test]
fn restored_custom_scheme_survives_the_first_host_sync() {
    let mut editor = Editor {
        scheme: Scheme { mode: Harmony::Triad, base: 30.0, offsets: vec![0.0, 130.0, 235.0], active: 1, linked: true, angle: 30.0 },
        ..Default::default()
    };
    let rgb = hsv_to_rgb(160.0, 0.5, 0.8);
    read_hsv(&mut editor, [rgb[0], rgb[1], rgb[2], 1.0]);
    assert_eq!(editor.scheme.offsets, vec![0.0, 130.0, 235.0]);
    assert_eq!(editor.scheme.active, 1);
}

#[test]
fn native_wheel_selection_is_used_by_brush_at_all_document_depths() {
    for depth in [8, 16, 32] {
        let mut app = app();
        app.run("file.new", json!({"width":64,"height":64,"mode":"rgb","depth":depth,"background":"white"})).unwrap();
        let mut state = State::default();
        commit(&mut app, &mut state, [150.0, 1.0, 1.0]);
        app.run("paint.stroke", json!({"points":[[16,32,1],[48,32,1]],"size":16,"hardness":1,"opacity":1})).unwrap();
        let pixel = app.run("document.pixel", json!({"x":32,"y":32})).unwrap();
        let values = pixel.as_array().unwrap();
        assert!(
            values[0].as_f64().unwrap() < 0.02 && values[1].as_f64().unwrap() > 0.98 && (values[2].as_f64().unwrap() - 0.5).abs() < 0.02,
            "{depth}: {pixel}"
        );
    }
}

#[test]
fn automation_reads_options_and_rejects_invalid_updates_atomically() {
    use crate::control::{ControlRequest, Outcome, handle};
    let mut app = app();
    let ctx = egui::Context::default();
    let call = |app: &mut PhotocraftApp, method: &str, params| {
        let (request, _rx) = ControlRequest::new(method, params);
        match handle(app, &ctx, &request) {
            Outcome::Done(value) => value,
            _ => panic!("expected immediate result"),
        }
    };
    assert_eq!(call(&mut app, "ui.set", json!({"huelet":{"shape":"Square","model":"Rgb","background":true}}))["ok"], true);
    assert_eq!(call(&mut app, "ui.inspect", json!({}))["result"]["huelet"]["shape"], "Square");
    let before = app.ui.huelet.clone();
    assert_eq!(call(&mut app, "ui.set", json!({"huelet":{"shape":"Triangle","misspelled":true}}))["ok"], false);
    assert_eq!(app.ui.huelet, before);
}

fn app() -> PhotocraftApp {
    PhotocraftApp::new(photocraft_engine::Session::new(), Default::default())
}

#[test]
fn shape_changes_do_not_add_recent_colors_and_focused_wheel_accepts_keyboard() {
    let mut h = harness(crate::theme::ThemeKind::ProMedium, 320.0);
    h.get_by_label("Square").click();
    h.run_steps(2);
    assert_eq!(h.state().ui.huelet.shape, Shape::Square);
    assert!(h.state().ui.huelet.recent.is_empty());
    let rect = h.get_by_label("Huelet color wheel").rect();
    click(&mut h, rect.center());
    h.key_press_modifiers(Modifiers::SHIFT, Key::ArrowRight);
    h.run_steps(2);
    assert!((h.state().ui.huelet.editors[0].memory[0] - 10.0).abs() < 1e-4);
    assert!(!h.state().ui.huelet.recent.is_empty());
}
fn harness(kind: crate::theme::ThemeKind, width: f32) -> Harness<'static, PhotocraftApp> {
    let mut ready = false;
    let mut h = Harness::builder().with_size(vec2(width, 950.0)).build_ui_state(
        move |ui, app: &mut PhotocraftApp| {
            if !ready {
                PhotocraftApp::setup_context(ui.ctx(), kind);
                ready = true;
                return;
            }
            show(app, ui);
        },
        app(),
    );
    h.run_steps(3);
    h
}
fn click(h: &mut Harness<'static, PhotocraftApp>, p: Pos2) {
    h.event(Event::PointerMoved(p));
    h.run_steps(1);
    h.event(Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.run_steps(1);
    h.event(Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.run_steps(2);
}
#[test]
fn triangle_round_trips_and_projects_outside_points() {
    let vertices = triangle(pos2(100.0, 100.0), 85.0);
    for s in [0.0, 0.3, 0.8, 1.0] {
        for v in [0.0, 0.4, 1.0] {
            let p = triangle_pos([s, v], vertices);
            let [back_s, back_v] = triangle_sv(p, vertices, s);
            assert!((back_s - s).abs() < 1e-5);
            assert!((back_v - v).abs() < 1e-5);
        }
    }
    for p in [pos2(-900.0, 50.0), pos2(100.0, 2000.0), pos2(9000.0, 9000.0)] {
        let [s, v] = triangle_sv(p, vertices, 0.4);
        assert!((0.0..=1.0).contains(&s) && (0.0..=1.0).contains(&v));
    }
}
#[test]
fn black_and_gray_keep_hue_and_black_saturation() {
    let mut editor = Editor { memory: [217.0, 0.7], ..Default::default() };
    assert_eq!(read_hsv(&mut editor, [0.0, 0.0, 0.0, 0.6]), [217.0, 0.7, 0.0]);
    assert_eq!(read_hsv(&mut editor, [0.4, 0.4, 0.4, 0.6]), [217.0, 0.0, 0.4]);
    read_hsv(&mut editor, [1.0, 0.0, 0.0, 0.6]);
    assert_eq!(editor.scheme.base, 0.0);
}
#[test]
fn linked_and_independent_harmony_keep_other_markers() {
    let mut scheme = Scheme { mode: Harmony::Triad, ..Default::default() };
    scheme.reset(30.0);
    scheme.active = 1;
    scheme.move_hue(170.0);
    assert_eq!(scheme.hues(), vec![50.0, 170.0, 290.0]);
    scheme.linked = false;
    scheme.move_hue(190.0);
    assert_eq!(scheme.hues(), vec![50.0, 190.0, 290.0]);
    scheme.active = 0;
    scheme.move_hue(70.0);
    assert_eq!(scheme.hues(), vec![70.0, 190.0, 290.0]);
}
#[test]
fn color_commands_preserve_alpha_and_do_not_touch_other_target() {
    let mut app = app();
    app.session.tools.foreground = [0.1, 0.2, 0.3, 0.4];
    app.session.tools.background = [0.9, 0.8, 0.7, 0.6];
    assert!(write_color(&mut app, false, [1.0, 0.0, 0.5]));
    assert_eq!(app.session.tools.foreground, [1.0, 0.0, 0.5, 0.4]);
    assert_eq!(app.session.tools.background, [0.9, 0.8, 0.7, 0.6]);
    assert!(write_color(&mut app, true, [0.0, 1.0, 0.0]));
    assert_eq!(app.session.tools.background, [0.0, 1.0, 0.0, 0.6]);
}
#[test]
fn native_panel_is_a_live_menu_and_can_float_redock_and_reopen() {
    let mut app = app();
    let ctx = egui::Context::default();
    assert!(crate::menus::menu_items(&app).iter().any(|i| i.id == "window.panel.huelet" && i.enabled && i.path == ["Window"]));
    crate::menus::invoke(&mut app, &ctx, "window.panel.huelet", json!({})).unwrap();
    assert_eq!(app.ui.dock_tabs.color, 4);
    app.run("window.panel.float", json!({"panel":"huelet","width":340,"height":750})).unwrap();
    assert!(app.ui.docking.as_ref().unwrap().floating.iter().any(|f| f.panels.contains(&"huelet".into())));
    app.run("window.panel.dock", json!({"panel":"huelet","anchor":"layers","zone":"top"})).unwrap();
    app.run("window.panel.close", json!({"panel":"huelet"})).unwrap();
    crate::menus::invoke(&mut app, &ctx, "window.panel.huelet", json!({})).unwrap();
    assert!(app.ui.docking.as_ref().unwrap().contains(&"huelet".into()));
}
#[test]
fn saved_options_and_swatches_survive_restart_and_bad_state_is_bounded() {
    let mut first = app();
    first.ui.huelet.shape = Shape::Square;
    first.ui.huelet.model = Model::Cmyk;
    first.ui.huelet.background = true;
    first.ui.huelet.show_recent = false;
    first.ui.huelet.favorites = vec!["#123456".into()];
    let ctx = egui::Context::default();
    persist(&mut first, &ctx);
    let mut second = app();
    second.session.load_prefs_json(&first.session.prefs_to_json()).unwrap();
    restore(&mut second);
    assert_eq!(second.ui.huelet, first.ui.huelet);
    let mut bad = State::default();
    bad.editors[0].scheme.active = usize::MAX;
    bad.editors[0].scheme.offsets = vec![];
    bad.editors[0].memory = [f32::NAN, f32::INFINITY];
    bad.favorites = vec!["invalid".into(), "#123456".into(), "#123456".into()];
    bad.sanitize();
    assert_eq!(bad.editors[0].scheme.active, 0);
    assert_eq!(bad.editors[0].memory, [0.0, 0.0]);
    assert_eq!(bad.favorites, vec!["#123456"]);
}
#[test]
fn channel_models_use_host_conversions_and_keep_black_cmyk_edits() {
    let rgb = [0.2, 0.4, 0.7];
    let hsv = rgb_to_hsv(rgb);
    for model in [Model::Hsv, Model::Rgb, Model::Lab, Model::Cmyk] {
        let back = channel_rgb(model, channels(model, hsv, rgb));
        for (a, b) in rgb.into_iter().zip(back) {
            // ICC CMYK is gamut-limited; its forward and inverse LUTs are approximate.
            let tolerance = if model == Model::Cmyk { 0.06 } else { 0.015 };
            assert!((a - b).abs() < tolerance, "{model:?}: {back:?}");
        }
    }
    let dark = channel_rgb(Model::Cmyk, [30.0, 40.0, 50.0, 100.0]);
    let light = channel_rgb(Model::Cmyk, [30.0, 40.0, 50.0, 50.0]);
    assert!(dark.into_iter().zip(light).all(|(a, b)| a < b));
}
#[test]
fn real_pointer_and_host_color_updates_work_for_both_shapes_and_targets() {
    for shape in [Shape::Triangle, Shape::Square] {
        for background in [false, true] {
            let mut h = harness(crate::theme::ThemeKind::ProMedium, 320.0);
            h.state_mut().ui.huelet.shape = shape;
            h.state_mut().ui.huelet.background = background;
            let initial = [0.0, 1.0, 0.0, 0.35];
            h.state_mut().session.execute("tools.setColors", if background { json!({"background":initial}) } else { json!({"foreground":initial}) }).unwrap();
            h.run_steps(2);
            assert_eq!(h.state().ui.huelet.editors[usize::from(background)].memory[0], 120.0);
            let rect = h.get_by_label("Huelet color wheel").rect();
            click(&mut h, rect.center());
            let c = color(h.state(), background);
            assert_eq!(c[3], 0.35);
            assert!(c[0] > 0.05 && c[2] > 0.05);
            let p = ring_point(rect.center(), rect.width() * 0.4225, 240.0);
            click(&mut h, p);
            assert!((rgb_to_hsv([color(h.state(), background)[0], color(h.state(), background)[1], color(h.state(), background)[2]])[0] - 240.0).abs() < 1.0);
        }
    }
}
#[test]
fn panel_renders_in_all_themes_at_narrow_and_wide_sizes_without_changing_color() {
    for kind in crate::theme::ThemeKind::ALL {
        for width in [220.0, 340.0] {
            let mut h = harness(kind, width);
            let before = h.state().session.tools.foreground;
            h.run_steps(3);
            assert_eq!(h.state().session.tools.foreground, before);
            let rect = h.get_by_label("Huelet color wheel").rect();
            assert!(rect.width() <= width && rect.is_finite());
            assert!(h.state().ui.huelet.recent.is_empty());
        }
    }
}
