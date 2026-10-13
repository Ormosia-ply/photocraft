//! Brush-tip rotation shortcuts: arrows turn by 1°, Shift+arrows by 15°.
//! These change the session brush through `tools.setBrush`, so the settings panel,
//! previews, cursor and subsequent strokes all use the same angle.

use serde_json::{Value, json};

use crate::commands::{CommandSpec, always};
use crate::{EngineError, Result, Session};

/// Whether this command rotates the brush tip (used for tool-specific UI bindings).
pub fn is_rotation_command(id: &str) -> bool {
    matches!(id, "tools.rotateBrushCounterClockwise" | "tools.rotateBrushClockwise" | "tools.rotateBrushCounterClockwise15" | "tools.rotateBrushClockwise15")
}

fn rotate(s: &mut Session, params: &Value, id: &str, delta: f64) -> Result<Value> {
    let bad = |msg: &str| EngineError::BadParams { cmd: id.into(), msg: msg.into() };
    if !params.is_null() && !params.as_object().is_some_and(|p| p.is_empty()) {
        return Err(bad("expected an empty object or null"));
    }
    let angle = s.tools.brush.angle;
    if !angle.is_finite() {
        return Err(bad("the current brush angle must be finite"));
    }
    // Brush angles are counterclockwise in tip space (Y up). Wrap instead of clamping
    // at ±180°, and calculate in f64 so every finite f32 angle can be reduced safely.
    let angle = ((f64::from(angle).rem_euclid(360.0) + delta + 180.0).rem_euclid(360.0) - 180.0) as f32;
    crate::brush_cmds::set_brush(s, &json!({"brush": {"angle": angle}}))
}

macro_rules! key_cmd {
    ($id:literal, $label:literal, $shortcut:literal, $delta:literal) => {
        CommandSpec {
            id: $id,
            label: $label,
            menu: &[],
            shortcut: Some($shortcut),
            params: "{}",
            enabled: always,
            run: |s, p| rotate(s, p, $id, $delta),
            journal: true,
        }
    };
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        key_cmd!("tools.rotateBrushCounterClockwise", "Rotate Brush Tip Counterclockwise", "ArrowLeft", 1.0),
        key_cmd!("tools.rotateBrushClockwise", "Rotate Brush Tip Clockwise", "ArrowRight", -1.0),
        key_cmd!("tools.rotateBrushCounterClockwise15", "Rotate Brush Tip Counterclockwise by 15°", "Shift+ArrowLeft", 15.0),
        key_cmd!("tools.rotateBrushClockwise15", "Rotate Brush Tip Clockwise by 15°", "Shift+ArrowRight", -15.0),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_commands_wrap_and_journal_without_changing_other_brush_fields() {
        let mut s = Session::new();
        s.execute("tools.setBrush", json!({"angle": 179, "roundness": 0.25, "size": 80})).unwrap();
        let before = s.tools.brush.clone();
        for (id, expected) in [
            ("tools.rotateBrushCounterClockwise15", -166.0),
            ("tools.rotateBrushClockwise15", 179.0),
            ("tools.rotateBrushCounterClockwise", -180.0),
            ("tools.rotateBrushClockwise", 179.0),
        ] {
            s.execute(id, json!({})).unwrap();
            assert_eq!(s.tools.brush.angle, expected);
            let mut expected_brush = before.clone();
            expected_brush.angle = expected;
            assert_eq!(s.tools.brush, expected_brush);
            assert_eq!(s.journal.last().map(|(id, _)| id.as_str()), Some(id));
        }
        s.execute("tools.rotateBrushClockwise", Value::Null).unwrap();
        assert_eq!(s.tools.brush.angle, 178.0);
    }

    #[test]
    fn bad_params_and_nonfinite_state_fail_without_changing_the_brush() {
        for spec in specs() {
            let mut s = Session::new();
            let before = s.tools.brush.clone();
            for params in [json!(true), json!(42), json!("angle"), json!([]), json!({"angle": "bad"})] {
                assert!(s.execute(spec.id, params).is_err());
                assert_eq!(s.tools.brush, before);
                assert!(s.journal.is_empty());
            }
            for angle in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                s.tools.brush.angle = angle;
                assert!(s.execute(spec.id, json!({})).is_err());
                assert_eq!(s.tools.brush.angle.to_bits(), angle.to_bits());
            }
        }
    }

    #[test]
    fn repeated_turns_and_extreme_finite_angles_stay_in_range() {
        let mut s = Session::new();
        for _ in 0..360 {
            s.execute("tools.rotateBrushClockwise", json!({})).unwrap();
        }
        assert_eq!(s.tools.brush.angle, 0.0);
        for angle in [f32::MAX, f32::MIN, -180.0, 180.0, 359.5] {
            for spec in specs() {
                s.tools.brush.angle = angle;
                s.execute(spec.id, json!({})).unwrap();
                assert!((-180.0..=180.0).contains(&s.tools.brush.angle));
            }
        }
    }

    #[test]
    fn rotating_changes_the_painted_tip_at_every_depth() {
        for depth in [8, 16, 32] {
            let mut s = Session::new();
            s.execute("file.new", json!({"width": 80, "height": 40, "depth": depth, "background": "transparent"})).unwrap();
            s.execute(
                "tools.setBrush",
                json!({"size": 30, "roundness": 0.2, "hardness": 1, "pressureSize": false,
                    "shapeDynamics": {"enabled": false}, "transfer": {"enabled": false}, "smoothing": {"amount": 0}}),
            )
            .unwrap();
            s.execute("paint.stroke", json!({"points": [[20, 20], [21, 20]]})).unwrap();
            for _ in 0..6 {
                s.execute("tools.rotateBrushCounterClockwise15", json!({})).unwrap();
            }
            s.execute("paint.stroke", json!({"points": [[60, 20], [61, 20]]})).unwrap();
            let st = s.active().unwrap();
            let surface = st.doc.layer(st.active_layer.unwrap()).unwrap().surface().unwrap();
            assert!(surface.rgba(30, 20)[3] > 0.9 && surface.rgba(20, 30)[3] < 0.01, "{depth}-bit horizontal tip");
            assert!(surface.rgba(60, 30)[3] > 0.9 && surface.rgba(70, 20)[3] < 0.01, "{depth}-bit rotated tip");
        }
    }
}
