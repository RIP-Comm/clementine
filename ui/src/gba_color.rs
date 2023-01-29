use egui::Color32;

pub struct GbaColor(pub emu::render::color::Color);

impl From<GbaColor> for Color32 {
    fn from(gba_color: GbaColor) -> Self {
        Self::from_rgb(
            gba_color.0.red() << 3,
            gba_color.0.green() << 3,
            gba_color.0.blue() << 3,
        )
    }
}

impl From<Color32> for GbaColor {
    fn from(color_u32: Color32) -> Self {
        Self(emu::render::color::Color::from_rgb(
            color_u32.r() >> 3,
            color_u32.g() >> 3,
            color_u32.b() >> 3,
        ))
    }
}
