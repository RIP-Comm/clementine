mod about;
pub mod app;
mod cpu_handler;
mod cpu_registers;
#[cfg(feature = "disassembler")]
mod disassembler;
mod gba_color;
mod gba_display;
mod savegame;
mod ui_traits;
