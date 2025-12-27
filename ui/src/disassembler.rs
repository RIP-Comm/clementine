//! # Disassembler Widget
//!
//! Real-time disassembly viewer for executed CPU instructions.
//!
//! ## Architecture
//!
//! The disassembler uses a lock-free SPSC (single-producer, single-consumer)
//! channel to receive instruction data from the CPU without blocking:
//!
//! ```text
//! ┌─────────────────┐         ┌─────────────────┐
//! │   Arm7tdmi      │         │  Disassembler   │
//! │                 │         │                 │
//! │  execute_arm()  │         │  drain_entries()│
//! │  execute_thumb()│  SPSC   │       │         │
//! │       │         │ Channel │       ▼         │
//! │       ▼         │         │  format()       │
//! │  tx.push(entry) ├────────►│       │         │
//! │                 │         │       ▼         │
//! │  (no format!)   │         │  display_text   │
//! └─────────────────┘         └─────────────────┘
//!      CPU thread                 UI thread
//! ```
//!
//! ## Frame Update Flow
//!
//! Each frame (~60fps), [`Disassembler::ui()`] is called which:
//!
//! 1. Calls [`drain_entries()`](Disassembler::drain_entries) to consume up to
//!    `MAX_ENTRIES_PER_FRAME` items
//! 2. Each entry is formatted and appended to `display_text`
//! 3. Old lines are removed when exceeding `MAX_DISPLAY_LINES`
//! 4. The text is rendered in a scrollable view

use std::sync::{Arc, Mutex};

use crate::emu_thread::EmuHandle;
use crate::ui_traits::UiTool;
use egui::{ScrollArea, TextEdit, TextStyle};

/// Maximum number of disassembled lines to keep in the display buffer.
const MAX_DISPLAY_LINES: usize = 5000;

/// Maximum number of entries to process per frame to avoid UI lag.
const MAX_ENTRIES_PER_FRAME: usize = 10000;

pub struct Disassembler {
    /// Handle to the emulator (contains the disasm channel consumer).
    emu_handle: Arc<Mutex<EmuHandle>>,
    /// Pre-built display text (avoids rebuilding every frame).
    display_text: String,
    /// Number of lines currently in `display_text`.
    line_count: usize,
}

impl Disassembler {
    pub fn new(emu_handle: Arc<Mutex<EmuHandle>>) -> Self {
        Self {
            emu_handle,
            display_text: String::with_capacity(MAX_DISPLAY_LINES * 50),
            line_count: 0,
        }
    }

    /// Drain available entries from the channel and append to display.
    /// Limits processing to avoid blocking the UI.
    fn drain_entries(&mut self) {
        let mut processed = 0;

        // Get access to the disasm channel through the emu handle
        if let Ok(mut handle) = self.emu_handle.lock() {
            while processed < MAX_ENTRIES_PER_FRAME {
                match handle.disasm_rx().pop() {
                    Ok(entry) => {
                        // If we're at max lines, remove the first line
                        if self.line_count >= MAX_DISPLAY_LINES
                            && let Some(newline_pos) = self.display_text.find('\n')
                        {
                            self.display_text.drain(..=newline_pos);
                            self.line_count -= 1;
                        }

                        if !self.display_text.is_empty() {
                            self.display_text.push('\n');
                        }
                        self.display_text.push_str(&entry.format());
                        self.line_count += 1;
                        processed += 1;
                    }
                    Err(_) => break, // channel empty
                }
            }
        }
    }
}

impl UiTool for Disassembler {
    fn name(&self) -> &'static str {
        "Disassembler"
    }

    fn show(&mut self, ctx: &egui::Context, open: &mut bool) {
        egui::Window::new(self.name())
            .resizable(true)
            .open(open)
            .default_pos(egui::pos2(750.0, 50.0))
            .default_width(400.0)
            .default_height(500.0)
            .show(ctx, |ui| {
                self.ui(ui);
            });
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        self.drain_entries();

        let mut text = self.display_text.as_str();

        ScrollArea::both().stick_to_bottom(true).show(ui, |ui| {
            ui.add(
                TextEdit::multiline(&mut text)
                    .interactive(false)
                    .font(TextStyle::Monospace)
                    .desired_width(f32::INFINITY),
            );
        });
    }
}
