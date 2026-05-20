//! Runs the jsmolka test ROMs (arm.gba / thumb.gba) and checks they pass.
//!
//! Each ROM stores the failing test id in r12 and then spins in `idle: b idle`.
//! r12 == 0 means every sub-test passed. The ROMs poll DISPSTAT for VBlank
//! instead of using interrupts, so they exercise the LCD pixel clock and the
//! frame-ready signaling in `Bus::step`.
//!
//! These tests need the real BIOS (`gba_bios.bin` in the repo root) and the
//! jsmolka ROMs checked out as a sibling of this repo. They skip when the files
//! are missing so CI without those assets stays green.

use emu::gba::Gba;

/// Run a ROM until it settles into the `b self` idle loop, then return r12.
fn run_until_idle(rom_path: &str) -> Option<u32> {
    let bios = std::fs::read("../gba_bios.bin").ok()?;
    let rom = std::fs::read(rom_path).ok()?;

    let bios: [u8; 0x4000] = bios.get(0..0x4000)?.try_into().ok()?;
    let mut gba = Gba::new(bios, &rom);

    // The idle loop is a single `b .`, but the 3-stage pipeline makes the
    // reported PC cycle over a 3-word band as it keeps refilling. Treat "PC
    // confined to a <=12 byte band for a long stretch" as the idle state, but
    // only once we are running from the ROM (>= 0x0800_0000) so the tight wait
    // loops the BIOS spins in during the boot animation do not match.
    let mut window_min = usize::MAX;
    let mut window_max = 0usize;
    let mut stable = 0u32;
    const BUDGET: u64 = 300_000_000;

    for _ in 0..BUDGET {
        gba.step();
        let pc = gba.cpu.registers.program_counter();

        if pc >= 0x0800_0000 && pc.max(window_max) - pc.min(window_min) <= 12 {
            window_min = window_min.min(pc);
            window_max = window_max.max(pc);
            stable += 1;
            if stable > 200_000 {
                return Some(gba.cpu.registers.register_at(12));
            }
        } else {
            stable = 0;
            window_min = pc;
            window_max = pc;
        }
    }

    // Did not settle: report r12 anyway so the assert message is useful.
    Some(gba.cpu.registers.register_at(12))
}

#[test]
fn arm_rom_passes() {
    match run_until_idle("../../gba-tests/arm/arm.gba") {
        None => eprintln!("skipping: arm.gba or gba_bios.bin not found"),
        Some(r12) => assert_eq!(r12, 0, "arm.gba failed at test number {r12}"),
    }
}

#[test]
fn thumb_rom_passes() {
    match run_until_idle("../../gba-tests/thumb/thumb.gba") {
        None => eprintln!("skipping: thumb.gba or gba_bios.bin not found"),
        Some(r12) => assert_eq!(r12, 0, "thumb.gba failed at test number {r12}"),
    }
}
