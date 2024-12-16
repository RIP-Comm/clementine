pub mod dma;
pub mod internal_memory;
pub mod interrupt_control;
pub mod keypad;

#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_lossless)]
#[allow(clippy::cast_sign_loss)]
#[allow(clippy::cast_possible_wrap)]
#[allow(clippy::large_stack_frames)]
pub mod lcd;
pub mod serial;
pub mod sound;
pub mod timers;

#[must_use]
pub const fn get_unmasked_address(
    address: usize,
    mask_get: usize,
    mask_set: usize,
    mask_shift: usize,
    modulo: usize,
) -> usize {
    // Get the index of the mirror
    let idx = (address & mask_get) >> mask_shift;
    // Remove the mirror index from the address
    let mut address = address & mask_set;
    // Insert the unmasked index in the address
    address |= (idx % modulo) << mask_shift;

    address
}
