//! Sound controls widget: master volume and mute for the audio output.

use std::sync::Arc;

use crate::audio::AudioControls;
use crate::ui_traits::UiTool;

pub struct SoundControls {
    /// `None` when no audio device was available at startup.
    controls: Option<Arc<AudioControls>>,
}

impl SoundControls {
    pub const fn new(controls: Option<Arc<AudioControls>>) -> Self {
        Self { controls }
    }
}

impl UiTool for SoundControls {
    fn name(&self) -> &'static str {
        "Sound"
    }

    fn show(&mut self, ctx: &egui::Context, open: &mut bool) {
        egui::Window::new(self.name())
            .default_width(220.0)
            .default_pos(egui::pos2(450.0, 300.0))
            .open(open)
            .show(ctx, |ui| {
                self.ui(ui);
            });
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        let Some(controls) = &self.controls else {
            ui.label("No audio device available.");
            return;
        };

        let mut enabled = !controls.muted();
        if ui.checkbox(&mut enabled, "Enabled").changed() {
            controls.set_muted(!enabled);
        }

        let mut volume = controls.volume();
        ui.add_enabled_ui(enabled, |ui| {
            if ui
                .add(egui::Slider::new(&mut volume, 0.0..=1.0).text("Volume"))
                .changed()
            {
                controls.set_volume(volume);
            }
        });
    }
}
