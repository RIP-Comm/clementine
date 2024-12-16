mod arm;

#[allow(clippy::cast_lossless)]
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::large_stack_frames)]
#[allow(clippy::module_name_repetitions)]
pub mod arm7tdmi;
mod condition;
mod cpu_modes;

#[allow(clippy::cast_possible_truncation)]
mod flags;

#[allow(clippy::cast_possible_truncation)]
pub mod hardware;
mod psr;
mod register_bank;
mod registers;
mod thumb;
