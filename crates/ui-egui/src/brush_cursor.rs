//! The painting cursor for brushes with a tip shape: sampled tip outlines and rotated or
//! flattened round tips. Everything else falls back to the plain circle (`tool_cursor`).

use std::sync::Arc;

use egui::{CursorIcon, Painter, Pos2, Vec2};
use photocraft_paint::brush::{BrushSettings, TipShape};

/// Below this on-screen radius an outline is unreadable: show the crosshair instead.
const MIN_RADIUS: f32 = 3.0;

#[derive(Clone)]
struct Cached {
    key: Key,
    rings: Option<Arc<Vec<Vec<[f32; 2]>>>>,
}

/// The brush fields an outline depends on.
#[derive(Clone)]
struct Key {
    tip: TipShape,
    loaded: bool,
    size: f32,
    angle: f32,
    roundness: f32,
    hardness: f32,
    flip_x: bool,
    flip_y: bool,
    full: bool,
}

impl Key {
    fn of(b: &BrushSettings, full: bool) -> Self {
        Self {
            tip: b.tip.clone(),
            loaded: loaded(&b.tip),
            size: b.size,
            angle: b.angle,
            roundness: b.roundness,
            hardness: b.hardness,
            flip_x: b.flip_x,
            flip_y: b.flip_y,
            full,
        }
    }
    fn matches(&self, b: &BrushSettings, full: bool) -> bool {
        self.tip == b.tip
            && self.loaded == loaded(&b.tip)
            && self.size == b.size
            && self.angle == b.angle
            && self.roundness == b.roundness
            && self.hardness == b.hardness
            && self.flip_x == b.flip_x
            && self.flip_y == b.flip_y
            && self.full == full
    }
}

fn loaded(tip: &TipShape) -> bool {
    !matches!(tip, TipShape::Stored(tile) if !tile.is_loaded())
}

/// The brush cursor at `at`. `radius` is the tip's screen radius (size / 2 at the current zoom);
/// `normal` is Preferences › Cursors "Normal Brush Tip" (the 50 % contour, not the full tip).
pub(crate) fn cursor(painter: &Painter, at: Pos2, brush: &BrushSettings, radius: f32, normal: bool, centre: bool) -> CursorIcon {
    if let Some(rings) = rings(painter.ctx(), brush, !normal) {
        let pts: Vec<Vec<Vec2>> = rings.iter().map(|r| r.iter().map(|p| Vec2::new(p[0], p[1]) * radius).collect()).collect();
        let extent = pts.iter().flatten().fold(0.0f32, |m, p| m.max(p.length()));
        if extent < MIN_RADIUS {
            return crate::tool_cursor::crosshair(painter, at, 4.0, 0.0);
        }
        return crate::tool_cursor::outline(painter, at, pts, centre);
    }
    let r = if normal { photocraft_paint::half_coverage_radius(radius, brush.hardness).max(1.0) } else { radius };
    crate::tool_cursor::circle(painter, at, r, centre)
}

/// The tip outline in radius units, cached across frames; `None` when the plain circle says it all
/// (a plain round tip) or the tip has no traceable shape.
fn rings(ctx: &egui::Context, brush: &BrushSettings, full: bool) -> Option<Arc<Vec<Vec<[f32; 2]>>>> {
    if matches!(brush.tip, TipShape::Round) && brush.angle == 0.0 && brush.roundness >= 1.0 {
        return None;
    }
    let id = egui::Id::new("photocraft-tip-outline");
    if let Some(c) = ctx.data(|d| d.get_temp::<Cached>(id))
        && c.key.matches(brush, full)
    {
        return c.rings;
    }
    // Faint or sparse sampled tips may have no 50% contour. Their full footprint still tells
    // the user which brush is selected; falling back to a circle discards that shape entirely.
    let traced = photocraft_paint::outline::tip_outline(brush, full)
        .or_else(|| if full { None } else { photocraft_paint::outline::tip_outline(brush, true) })
        .map(Arc::new);
    ctx.data_mut(|d| d.insert_temp(id, Cached { key: Key::of(brush, full), rings: traced.clone() }));
    traced
}

#[cfg(test)]
mod tests {
    use super::*;
    use photocraft_paint::tile::{GrayTile, StoredTile};

    fn faint_tip() -> GrayTile {
        GrayTile::from_fn(24, 16, |x, y| if (3..8).contains(&x) || (10..14).contains(&y) { 0.25 } else { 0.0 })
    }

    #[test]
    fn faint_sampled_tip_keeps_its_shape_in_normal_cursor_mode() {
        let brush = BrushSettings { size: 80.0, tip: TipShape::Sampled(faint_tip()), ..Default::default() };
        assert!(photocraft_paint::outline::tip_outline(&brush, false).is_none(), "no pixels reach 50% coverage");
        let ctx = egui::Context::default();
        let normal = rings(&ctx, &brush, false).expect("the full footprint replaces the missing 50% contour");
        let full = photocraft_paint::outline::tip_outline(&brush, true).unwrap();
        assert_eq!(*normal, full);
        assert!(Arc::ptr_eq(&normal, &rings(&ctx, &brush, false).unwrap()), "unchanged brushes reuse their outline");
    }

    #[test]
    fn loading_a_stored_tip_invalidates_the_preview_outline() {
        let ctx = egui::Context::default();
        let tile = StoredTile { key: "faint-shaped-tip".into(), width: 24, height: 16, preview: Arc::new(GrayTile::from_fn(4, 4, |_, _| 0.0)), full: None };
        let mut brush = BrushSettings { size: 80.0, tip: TipShape::Stored(tile), ..Default::default() };
        assert!(rings(&ctx, &brush, false).is_none());
        if let TipShape::Stored(tile) = &mut brush.tip {
            tile.full = Some(Arc::new(faint_tip()));
        }
        assert!(rings(&ctx, &brush, false).is_some(), "the loaded tip must replace the cached circular fallback");
    }

    #[test]
    fn resizing_a_flat_tip_recomputes_its_minimum_roundness() {
        let ctx = egui::Context::default();
        let mut brush = BrushSettings { size: 2.0, roundness: 0.01, ..Default::default() };
        let small = rings(&ctx, &brush, true).unwrap();
        brush.size = 100.0;
        let large = rings(&ctx, &brush, true).unwrap();
        assert_ne!(*small, *large);
        assert_eq!(*large, photocraft_paint::outline::tip_outline(&brush, true).unwrap());
    }

    #[test]
    fn builtin_shaped_tips_have_a_cursor_outline() {
        let ctx = egui::Context::default();
        for preset in photocraft_paint::presets::builtin() {
            if !matches!(preset.brush.tip, TipShape::Round) {
                assert!(rings(&ctx, &preset.brush, false).is_some(), "{} must show its tip shape", preset.name);
            }
        }
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn faint_sampled_cursor_uploads_its_outline_instead_of_a_circle() {
        let ctx = egui::Context::default();
        ctx.add_plugin(crate::tool_cursor::CursorLifecycle);
        let mut brush = BrushSettings { size: 80.0, tip: TipShape::Sampled(faint_tip()), ..Default::default() };
        let render = |brush: &BrushSettings| {
            let mut output = ctx.run_ui(Default::default(), |ui| {
                let icon = cursor(ui.painter(), egui::pos2(100.0, 100.0), brush, 40.0, true, false);
                ui.ctx().set_cursor_icon(icon);
            });
            output.textures_delta.clear();
            output.platform_output.cursor_image.unwrap()
        };
        let shaped = render(&brush);
        brush.tip = TipShape::Round;
        let round = render(&brush);
        assert_ne!(shaped.rgba, round.rgba, "the OS must receive the sampled shape, not the circular fallback");
        assert!(shaped.rgba.as_chunks::<4>().0.iter().any(|p| p[3] > 0));
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn selecting_shaped_presets_updates_the_complete_desktop_cursor() {
        use egui_kittest::Harness;
        use serde_json::json;

        let mut app = crate::PhotocraftApp::new(photocraft_engine::Session::new(), Default::default());
        app.run("file.new", json!({"width": 300, "height": 300})).unwrap();
        app.ui.tool = crate::Tool::Brush;
        let mut h = Harness::builder().with_size(egui::vec2(1000.0, 700.0)).build_eframe(move |cc| {
            crate::PhotocraftApp::setup_context(&cc.egui_ctx, Default::default());
            app
        });
        h.run_steps(4);
        h.hover_at(h.state().last_canvas_rect.center());
        h.run_steps(3);
        for preset in photocraft_paint::presets::builtin() {
            if matches!(preset.brush.tip, TipShape::Round) {
                continue;
            }
            h.state_mut().run("tools.setBrush", json!({"preset": preset.name})).unwrap();
            h.run_steps(3);
            let shaped = h.output().platform_output.cursor_image.clone().expect("the selected tip reaches the OS");
            let mut round = h.state().session.tools.brush.clone();
            round.tip = TipShape::Round;
            h.state_mut().run("tools.setBrush", json!({"brush": round})).unwrap();
            h.run_steps(3);
            let circle = h.output().platform_output.cursor_image.as_ref().unwrap();
            assert_ne!(shaped.rgba, circle.rgba, "{} must show its sampled shape", preset.name);
        }
    }

    #[test]
    #[ignore = "writes target/brush-shaped-cursor-preview.png for visual inspection"]
    #[cfg(not(target_arch = "wasm32"))]
    fn write_shaped_cursor_preview() {
        let ctx = egui::Context::default();
        ctx.add_plugin(crate::tool_cursor::CursorLifecycle);
        let brush = BrushSettings { size: 120.0, tip: TipShape::Sampled(faint_tip()), ..Default::default() };
        let mut output = ctx.run_ui(Default::default(), |ui| {
            let icon = cursor(ui.painter(), egui::pos2(100.0, 100.0), &brush, 60.0, true, false);
            ui.ctx().set_cursor_icon(icon);
        });
        output.textures_delta.clear();
        let image = output.platform_output.cursor_image.unwrap();
        let side = usize::from(image.size[0]);
        let mut rgba = vec![255; 600 * 200 * 4];
        for (col, background) in [0_u8, 128, 255].into_iter().enumerate() {
            for y in 0..200 {
                for x in col * 200..(col + 1) * 200 {
                    rgba[(y * 600 + x) * 4..(y * 600 + x) * 4 + 3].fill(background);
                }
            }
            for (i, pixel) in image.rgba.as_chunks::<4>().0.iter().enumerate() {
                let x = col * 200 + 100 - usize::from(image.hotspot[0]) + i % side;
                let y = 100 - usize::from(image.hotspot[1]) + i / side;
                let a = f32::from(pixel[3]) / 255.0;
                let value = (f32::from(pixel[0]) * a + f32::from(background) * (1.0 - a)).round() as u8;
                rgba[(y * 600 + x) * 4..(y * 600 + x) * 4 + 3].fill(value);
            }
        }
        let preview = photocraft_codecs::Image::from_u8(600, 200, photocraft_codecs::ChannelLayout::Rgba, rgba).unwrap();
        let png = photocraft_codecs::encode(&preview, photocraft_codecs::Format::Png, &Default::default()).unwrap();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/brush-shaped-cursor-preview.png");
        std::fs::write(path, png).unwrap();
    }
}
