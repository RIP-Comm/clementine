use crate::dashboard::Dashboard;

pub struct ClementineApp {
    dashboard: Dashboard,
}

impl ClementineApp {
    pub fn new(cartridge_name: String) -> Self {
        Self {
            dashboard: Dashboard::new(cartridge_name),
        }
    }
}

impl eframe::App for ClementineApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.dashboard.ui(ctx);
    }
}
