# Huelet native panel

Huelet is built into PhotoCraft's Rust/egui UI. Build and launch the desktop app with
`cargo run --release -p photocraft`, then choose **Window > Huelet**. It is a tab in
the Color group. Drag its tab to float it or dock it next to another panel; resize
the group to show more controls. Short groups scroll vertically.

The panel provides a hue ring, fixed HSV triangle or square, foreground/background
selection and swapping, HEX input and copying, HSV/RGB/Lab/CMYK gradient sliders,
seven harmony modes, linked or independent hue markers, adjustable harmony angles,
favorite swatches and recent colors. Right-click a favorite to delete it, or focus
it and press Delete. Panel Options hides harmony, favorites and recent colors.

The focused wheel accepts Left/Right for hue and Up/Down for value. Alt+Up/Down
changes saturation; Shift increases the step tenfold. Dragging outside the inner
picker stays on the nearest triangle edge or square boundary. Gray retains its
hue, and black retains its hue and saturation.

Colors use the existing `tools.setColors` and `tools.swapColors` commands. Edits
preserve the selected tool color's alpha. Eyedropper picks and edits in other panels
appear on the next frame. The native panel does not need a local control server,
authentication token, Electron, or a second PhotoCraft process.

Lab uses PhotoCraft's D50 conversion, matching its native Color Picker (standalone
Huelet uses D65). CMYK uses the host's active conversion context; absent a CMYK
profile it uses PhotoCraft's built-in coated CMYK profile. The panel edits display RGB tool
colors, rather than directly editing document-channel values or HDR colors.

Shape, channel model, foreground/background target, hue memory, harmony settings,
swatches and visibility are stored in PhotoCraft preferences under `dialogs.huelet`.
This preference is independent of standalone Huelet's Electron settings. Tool
colors themselves stay owned by PhotoCraft. Dock positions use normal workspace
preferences and respect Remember Workspace Changes.

Automation uses the normal panel commands with `panel: "huelet"`, for example
`window.panel.float`, `window.panel.dock`, `window.panel.activate`, and
`window.panel.close`. `window.panel.huelet` opens/selects the panel from the menu.
`ui.inspect` includes `huelet`; `ui.set {"huelet":{"shape":"Square","model":"Rgb"}}`
updates its options. Color commands are the same as the rest of the editor.

Regression checks live in `crates/ui-egui/src/huelet_tests.rs` and cover geometry,
host synchronization, alpha, harmony edits, saved preferences, panel movement,
pointer input and rendering in every built-in theme at narrow and wide sizes.
