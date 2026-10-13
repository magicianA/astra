//! Fractional sky label positioning with deferred font-atlas tessellation.

/// Restore a shared fractional translation after a text library truncates its
/// origin. Only vertices emitted by that label are touched.
pub fn restore_fractional_translation(vertices: &mut [[f32; 2]], first: usize, position: [f32; 2]) {
    let fraction = [
        position[0] - position[0].trunc(),
        position[1] - position[1].trunc(),
    ];
    for vertex in vertices.iter_mut().skip(first) {
        vertex[0] += fraction[0];
        vertex[1] += fraction[1];
    }
}

#[cfg(feature = "desktop")]
pub fn draw_moving_text(
    painter: &eframe::egui::Painter,
    position: eframe::egui::Pos2,
    color: eframe::egui::Color32,
    font: eframe::egui::FontId,
    text: &str,
) {
    if text.is_empty() || color.a() == 0 {
        return;
    }
    let galley = painter.layout_no_wrap(text.to_owned(), font, color);
    // Defer tessellation until every label has populated the font atlas. A mesh
    // made here would have stale normalized UVs if later labels grow the atlas.
    // UI layout already aligns static origins; sky origins must stay fractional.
    painter
        .ctx()
        .tessellation_options_mut(|options| options.round_text_to_pixels = false);
    painter.galley(position, galley, color);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fractional_correction_keeps_existing_geometry_and_spacing() {
        let mut vertices = [[10.0, 20.0], [100.0, 101.0], [110.0, 101.0]];
        restore_fractional_translation(&mut vertices, 1, [100.125, 100.75]);
        assert_eq!(vertices[0], [10.0, 20.0]);
        assert_eq!(vertices[1], [100.125, 101.75]);
        assert_eq!(vertices[2], [110.125, 101.75]);
        let len = vertices.len();
        restore_fractional_translation(&mut vertices, len, [-0.25, -0.75]);
    }
    #[cfg(feature = "desktop")]
    #[test]
    fn cjk_font_and_fractional_label_motion_at_retina_density() {
        use eframe::egui::{self, epaint::Primitive};
        let context = egui::Context::default();
        let mut fonts = egui::FontDefinitions::default();
        let bytes = std::fs::read(crate::source_data().join("fonts/NotoSansCJKsc-Regular.otf"))
            .expect("production CJK font");
        fonts.font_data.insert(
            "cjk".into(),
            std::sync::Arc::new(egui::FontData::from_owned(bytes)),
        );
        fonts
            .families
            .get_mut(&egui::FontFamily::Proportional)
            .unwrap()
            .insert(0, "cjk".into());
        context.set_fonts(fonts);
        for density in [1.0, 2.0] {
            context.set_pixels_per_point(density);
            let mut reference = Vec::new();
            for frame in 0..=64 {
                let offset = egui::vec2(frame as f32 * 0.125, frame as f32 * -0.0625);
                let origin = egui::pos2(-0.25, -0.75) + offset;
                let output = context.run(egui::RawInput::default(), |context| {
                    let font = egui::FontId::proportional(15.0);
                    assert!(context.fonts(|fonts| {
                        fonts.has_glyphs(&font, "月球天狼星 Moon Sirius 0123456789")
                    }));
                    let painter = context.layer_painter(egui::LayerId::background());
                    draw_moving_text(
                        &painter,
                        origin,
                        egui::Color32::WHITE,
                        font,
                        "月球 Moon / 天狼星 Sirius 0123456789",
                    );
                });
                let vertices: Vec<_> = context
                    .tessellate(output.shapes, output.pixels_per_point)
                    .iter()
                    .flat_map(|s| match &s.primitive {
                        Primitive::Mesh(mesh) => mesh.vertices.clone(),
                        _ => Vec::new(),
                    })
                    .collect();
                assert!(!vertices.is_empty());
                if frame == 0 {
                    reference = vertices.clone();
                }
                assert_eq!(vertices.len(), reference.len());
                for (vertex, base) in vertices.iter().zip(&reference) {
                    assert!(
                        (vertex.pos.x - base.pos.x - offset.x).abs() < 3e-5
                            && (vertex.pos.y - base.pos.y - offset.y).abs() < 3e-5
                    );
                    assert_eq!(vertex.uv, base.uv);
                }
            }
        }
    }
}
