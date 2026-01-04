//! Memory Inspector Tool
//!
//! A debug window for viewing and editing GBA memory in real-time.
//! Displays memory as a hex dump with ASCII view and allows editing bytes.

use std::sync::{Arc, Mutex};

use crate::emu_thread::{EmuCommand, EmuHandle};
use crate::ui_traits::UiTool;

/// Number of bytes to display per row.
const BYTES_PER_ROW: u32 = 16;
/// Number of rows to display.
const ROWS_TO_DISPLAY: u32 = 16;
/// Total bytes to fetch.
const BYTES_TO_FETCH: usize = (BYTES_PER_ROW * ROWS_TO_DISPLAY) as usize;

/// Memory regions with their address ranges.
const MEMORY_REGIONS: &[(&str, u32, u32)] = &[
    ("BIOS", 0x0000_0000, 0x0000_3FFF),
    ("EWRAM", 0x0200_0000, 0x0203_FFFF),
    ("IWRAM", 0x0300_0000, 0x0300_7FFF),
    ("I/O", 0x0400_0000, 0x0400_03FF),
    ("Palette", 0x0500_0000, 0x0500_03FF),
    ("VRAM", 0x0600_0000, 0x0601_7FFF),
    ("OAM", 0x0700_0000, 0x0700_03FF),
    ("ROM", 0x0800_0000, 0x09FF_FFFF),
    ("Flash", 0x0E00_0000, 0x0E00_FFFF),
];

/// Debug tool for viewing and editing memory.
pub struct MemoryInspector {
    emu_handle: Arc<Mutex<EmuHandle>>,
    /// Current address to view (start of display).
    address: u32,
    /// Address input string for editing.
    address_input: String,
    /// Selected memory region index.
    selected_region: usize,
    /// Cached memory data.
    memory_data: Vec<u8>,
    /// Address of cached data.
    cached_address: u32,
    /// Whether we're waiting for memory data.
    pending_request: bool,
    /// Byte being edited (offset from address).
    editing_byte: Option<usize>,
    /// Edit input string.
    edit_input: String,
    /// Auto-refresh enabled.
    auto_refresh: bool,
    /// Frames since last refresh.
    refresh_counter: u32,
}

impl MemoryInspector {
    pub fn new(emu_handle: Arc<Mutex<EmuHandle>>) -> Self {
        Self {
            emu_handle,
            address: 0x0200_0000, // Start at EWRAM
            address_input: "02000000".to_string(),
            selected_region: 1, // EWRAM
            memory_data: vec![0; BYTES_TO_FETCH],
            cached_address: 0,
            pending_request: false,
            editing_byte: None,
            edit_input: String::new(),
            auto_refresh: false,
            refresh_counter: 0,
        }
    }

    fn request_memory(&mut self) {
        if let Ok(mut handle) = self.emu_handle.lock() {
            handle.send(EmuCommand::ReadMemory {
                address: self.address,
                length: BYTES_TO_FETCH,
            });
            self.pending_request = true;
        }
    }

    fn check_pending_data(&mut self) {
        if let Ok(mut handle) = self.emu_handle.lock()
            && let Some((addr, data)) = handle.pending_memory_data.take()
        {
            if addr == self.address {
                self.memory_data = data;
                self.cached_address = addr;
                self.pending_request = false;
            } else {
                // Put it back if it's not for us
                handle.pending_memory_data = Some((addr, data));
            }
        }
    }

    fn write_byte(&mut self, offset: usize, value: u8) {
        #[allow(clippy::cast_possible_truncation)] // GBA addresses are 32-bit
        let addr = self.address.wrapping_add(offset as u32);
        if let Ok(mut handle) = self.emu_handle.lock() {
            handle.send(EmuCommand::WriteByte {
                address: addr,
                value,
            });
        }
        // Update local cache
        if offset < self.memory_data.len() {
            self.memory_data[offset] = value;
        }
    }

    fn goto_address(&mut self, addr: u32) {
        self.address = addr & !(BYTES_PER_ROW - 1);
        self.address_input = format!("{:08X}", self.address);
        self.request_memory();
    }

    /// Renders the toolbar with region selector, address input, and refresh controls.
    fn render_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("memory_region")
                .selected_text(MEMORY_REGIONS[self.selected_region].0)
                .show_ui(ui, |ui| {
                    for (i, (name, start, _)) in MEMORY_REGIONS.iter().enumerate() {
                        if ui
                            .selectable_value(&mut self.selected_region, i, *name)
                            .clicked()
                        {
                            self.goto_address(*start);
                        }
                    }
                });

            ui.separator();

            ui.label("Addr:");
            let response = ui.add(
                egui::TextEdit::singleline(&mut self.address_input)
                    .desired_width(70.0)
                    .font(egui::TextStyle::Monospace),
            );
            if response.lost_focus()
                && ui.input(|i| i.key_pressed(egui::Key::Enter))
                && let Ok(addr) = u32::from_str_radix(&self.address_input, 16)
            {
                self.goto_address(addr);
            }

            if ui.button("Go").clicked()
                && let Ok(addr) = u32::from_str_radix(&self.address_input, 16)
            {
                self.goto_address(addr);
            }

            ui.separator();

            if ui.button("Refresh").clicked() {
                self.request_memory();
            }

            ui.checkbox(&mut self.auto_refresh, "Auto");
        });
    }

    /// Renders the navigation buttons for paging through memory.
    fn render_navigation(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("<<").clicked() {
                #[allow(clippy::cast_possible_truncation)] // GBA addresses are 32-bit
                self.goto_address(self.address.saturating_sub(BYTES_TO_FETCH as u32));
            }
            if ui.button("<").clicked() {
                self.goto_address(self.address.saturating_sub(BYTES_PER_ROW));
            }
            ui.label(format!("0x{:08X}", self.address));
            if ui.button(">").clicked() {
                self.goto_address(self.address.wrapping_add(BYTES_PER_ROW));
            }
            if ui.button(">>").clicked() {
                #[allow(clippy::cast_possible_truncation)] // GBA addresses are 32-bit
                self.goto_address(self.address.wrapping_add(BYTES_TO_FETCH as u32));
            }

            if self.pending_request {
                ui.spinner();
            }
        });
    }

    /// Renders a single memory row with hex bytes and ASCII representation.
    fn render_memory_row(&mut self, ui: &mut egui::Ui, row: u32) {
        let row_addr = self.address.wrapping_add(row * BYTES_PER_ROW);
        let row_start = (row * BYTES_PER_ROW) as usize;

        ui.horizontal(|ui| {
            ui.label(format!("{row_addr:08X} "));

            for col in 0..BYTES_PER_ROW as usize {
                let offset = row_start + col;
                let byte = self.memory_data.get(offset).copied().unwrap_or(0);

                let is_editing = self.editing_byte == Some(offset);

                if is_editing {
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.edit_input)
                            .desired_width(18.0)
                            .font(egui::TextStyle::Monospace),
                    );

                    if response.lost_focus() {
                        if let Ok(value) = u8::from_str_radix(&self.edit_input, 16) {
                            self.write_byte(offset, value);
                        }
                        self.editing_byte = None;
                        self.edit_input.clear();
                    }

                    response.request_focus();
                } else {
                    let text = format!("{byte:02X}");
                    let response = ui.add(
                        egui::Label::new(egui::RichText::new(&text).color(if byte == 0 {
                            egui::Color32::DARK_GRAY
                        } else {
                            egui::Color32::WHITE
                        }))
                        .sense(egui::Sense::click()),
                    );

                    if response.clicked() {
                        self.editing_byte = Some(offset);
                        self.edit_input = format!("{byte:02X}");
                    }

                    if response.hovered() {
                        #[allow(clippy::cast_possible_truncation)] // GBA addresses are 32-bit
                        let addr = row_addr.wrapping_add(col as u32);
                        #[allow(clippy::cast_possible_wrap)] // intentional for signed display
                        let signed_byte = byte as i8;
                        response.on_hover_text(format!("0x{addr:08X}: {byte} ({signed_byte})",));
                    }
                }

                ui.add_space(1.0);
            }

            ui.add_space(4.0);

            // ASCII representation
            let mut ascii = String::with_capacity(BYTES_PER_ROW as usize);
            for col in 0..BYTES_PER_ROW as usize {
                let offset = row_start + col;
                let byte = self.memory_data.get(offset).copied().unwrap_or(0);
                ascii.push(if byte.is_ascii_graphic() || byte == b' ' {
                    byte as char
                } else {
                    '.'
                });
            }
            ui.label(ascii);
        });
    }

    /// Renders the hex dump view with header and all memory rows.
    fn render_hex_dump(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.style_mut().override_font_id = Some(egui::FontId::monospace(12.0));

            ui.horizontal(|ui| {
                ui.label("Address  ");
                for i in 0..BYTES_PER_ROW {
                    ui.label(format!("{i:02X} "));
                }
                ui.label(" ASCII");
            });

            ui.separator();

            for row in 0..ROWS_TO_DISPLAY {
                self.render_memory_row(ui, row);
            }
        });
    }
}

impl UiTool for MemoryInspector {
    fn name(&self) -> &'static str {
        "Memory Inspector"
    }

    fn show(&mut self, ctx: &egui::Context, open: &mut bool) {
        self.check_pending_data();

        // Auto-refresh logic
        if self.auto_refresh {
            self.refresh_counter += 1;
            if self.refresh_counter >= 30 {
                // ~0.5 sec at 60fps
                self.refresh_counter = 0;
                if !self.pending_request {
                    self.request_memory();
                }
            }
        }

        egui::Window::new(self.name())
            .default_width(520.0)
            .default_height(400.0)
            .open(open)
            .default_pos(egui::pos2(10.0, 200.0))
            .show(ctx, |ui| self.ui(ui));
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        self.render_toolbar(ui);
        ui.separator();
        self.render_navigation(ui);
        ui.separator();
        self.render_hex_dump(ui);
    }
}
