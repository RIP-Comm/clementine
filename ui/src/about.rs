use crate::ui_traits::UiTool;

#[derive(Default)]
pub struct About {}

impl UiTool for About {
    fn name(&self) -> &'static str {
        "About Clementine"
    }

    fn show(&mut self, ctx: &egui::Context, open: &mut bool) {
        egui::Window::new(self.name())
            .default_width(300.0)
            .default_pos(egui::pos2(450.0, 10.0))
            .open(open)
            .show(ctx, |ui| {
                self.ui(ui);
            });
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("🍊Clementine");
        ui.label(
            "Clementine is an emulator in early developing phase.\nThe community is working hard to realize this emulator for a pure educational scope.\nFeel free to contribute.".to_string()
        );
    }
}
