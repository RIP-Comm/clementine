#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_sign_loss)]
#[allow(clippy::cast_possible_wrap)]
mod bitwise;

#[allow(clippy::missing_panics_doc)]
#[allow(clippy::cast_lossless)]
#[allow(clippy::large_stack_frames)]
#[allow(clippy::unreadable_literal)]
pub mod bus;

#[allow(clippy::similar_names)]
pub mod cartridge_header;
pub mod cpu;
pub mod gba;
pub mod render;
