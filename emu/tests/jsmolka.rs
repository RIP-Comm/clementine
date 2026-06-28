//! Runs the jsmolka gba-tests ROMs headless and checks they pass.
//!
//! Each ROM stores the failing test id in r12 and then spins in `idle: b idle`.
//! r12 == 0 means every sub-test passed. The ROMs poll DISPSTAT for `VBlank`
//! instead of using interrupts, so they exercise the LCD pixel clock and the
//! frame-ready signaling in `Bus::step`.
//!
//! The ROMs are not vendored. Point `CLEMENTINE_TEST_ROMS` at a checkout of
//! <https://github.com/jsmolka/gba-tests> (see `scripts/fetch-test-roms.sh`).
//! A real BIOS is required too: `CLEMENTINE_BIOS`, or `gba_bios.bin` in the repo
//! root. When either is missing the tests skip so CI without the assets stays
//! green.

use emu::gba::Gba;
use std::path::PathBuf;

/// Directory holding the jsmolka gba-tests checkout.
fn test_roms_dir() -> Option<PathBuf> {
    std::env::var_os("CLEMENTINE_TEST_ROMS").map(PathBuf::from)
}

/// BIOS path: `CLEMENTINE_BIOS` if set, otherwise the repo-root `gba_bios.bin`.
fn bios_path() -> PathBuf {
    std::env::var_os("CLEMENTINE_BIOS")
        .map_or_else(|| PathBuf::from("../gba_bios.bin"), PathBuf::from)
}

/// Run a ROM until it settles into the `b self` idle loop, then return r12.
/// Returns `None` when the BIOS or the ROM is not available, so the caller can
/// skip instead of failing.
fn run_until_idle(rom_relative: &str) -> Option<u32> {
    const BUDGET: u64 = 300_000_000;

    let bios = std::fs::read(bios_path()).ok()?;
    let rom = std::fs::read(test_roms_dir()?.join(rom_relative)).ok()?;

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

/// Assert a ROM passes, or skip when the assets are missing.
fn check(rom_relative: &str) {
    match run_until_idle(rom_relative) {
        None => {
            eprintln!("skipping {rom_relative}: set CLEMENTINE_TEST_ROMS (and a BIOS) to run it");
        }
        Some(r12) => assert_eq!(r12, 0, "{rom_relative} failed at test number {r12}"),
    }
}

#[test]
fn arm_rom_passes() {
    check("arm/arm.gba");
}

#[test]
fn thumb_rom_passes() {
    check("thumb/thumb.gba");
}

#[test]
fn memory_rom_passes() {
    check("memory/memory.gba");
}
