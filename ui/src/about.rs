use crate::ui_traits::{UiTool, View};

pub struct About {}

impl About {
    pub fn new() -> Self {
        Self {}
    }
}

impl UiTool for About {
    fn name(&self) -> &'static str {
        "About Clementine"
    }

    fn show(&mut self, ctx: &egui::Context, open: &mut bool) {
        egui::Window::new(self.name())
            .default_width(320.0)
            .open(open)
            .show(ctx, |ui| {
                use View as _;
                self.ui(ui);
            });
    }
}

impl View for About {
    fn ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("🍊Clementine");
        ui.label(
            "Clementine is an emulator in early developing phase.\nThe community is working hard to realize this emulator for a pure educational scope.\nFeel free to contribute.".to_string()
        );
    }
}
