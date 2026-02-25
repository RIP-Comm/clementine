#![allow(clippy::cast_possible_truncation)]

//! LCD I/O Registers.
//!
//! This module contains the LCD control and status registers that configure
//! how the GBA renders graphics. These registers control background modes,
//! layer visibility, scrolling, windows, and special effects.
//!
//! # Register Map
//!
//! | Address       | Register | Description                              |
//! |---------------|----------|------------------------------------------|
//! | `0x0400_0000` | DISPCNT  | LCD control (mode, layer enables)        |
//! | `0x0400_0004` | DISPSTAT | LCD status (vblank, hblank flags)        |
//! | `0x0400_0006` | VCOUNT   | Current scanline (0-227)                 |
//! | `0x0400_0008` | BG0CNT   | BG0 control (priority, tiles, size)      |
//! | `0x0400_000A` | BG1CNT   | BG1 control                              |
//! | `0x0400_000C` | BG2CNT   | BG2 control                              |
//! | `0x0400_000E` | BG3CNT   | BG3 control                              |
//! | `0x0400_0010` | BG0HOFS  | BG0 horizontal scroll                    |
//! | `0x0400_0012` | BG0VOFS  | BG0 vertical scroll                      |
//! | ...           | ...      | (similar for BG1-BG3)                    |
//! | `0x0400_0040` | WIN0H    | Window 0 horizontal bounds               |
//! | `0x0400_0042` | WIN1H    | Window 1 horizontal bounds               |
//! | `0x0400_0044` | WIN0V    | Window 0 vertical bounds                 |
//! | `0x0400_0046` | WIN1V    | Window 1 vertical bounds                 |
//! | `0x0400_0048` | WININ    | Layer enables inside windows 0/1         |
//! | `0x0400_004A` | WINOUT   | Layer enables outside/OBJ window         |
//!
//! # Background Modes
//!
//! DISPCNT bits 0-2 select the background mode:
//!
//! | Mode | BG0      | BG1      | BG2      | BG3      |
//! |------|----------|----------|----------|----------|
//! | 0    | Text     | Text     | Text     | Text     |
//! | 1    | Text     | Text     | Affine   | -        |
//! | 2    | -        | -        | Affine   | Affine   |
//! | 3    | -        | -        | Bitmap   | -        |
//! | 4    | -        | -        | Bitmap   | -        |
//! | 5    | -        | -        | Bitmap   | -        |
//!
//! # Window System
//!
//! The GBA has two rectangular windows (WIN0, WIN1) plus an object window (WINOBJ).
//! Each window can independently enable/disable layers within its bounds.
//! Priority order: WIN0 > WIN1 > WINOBJ > Outside.
//!
//! Window coordinates use wrap-around logic: if right < left or bottom < top,
//! the window wraps around the screen edge.

use serde::{Deserialize, Serialize};

use crate::bitwise::Bits;

use super::ObjMappingKind;

/// LCD control and status registers.
///
/// These registers configure the PPU's rendering behavior including
/// which layers are visible, scrolling offsets, window regions, and effects.
#[derive(Default, Serialize, Deserialize)]
pub struct Registers {
    /// LCD Control
    pub dispcnt: u16,
    /// Undocumented
    pub green_swap: u16,
    /// General LCD Status (STAT, LYC)
    pub dispstat: u16,
    /// Vertical Counter (LY)
    pub vcount: u16,
    /// BG0 Control
    pub bg0cnt: u16,
    /// BG1 Control
    pub bg1cnt: u16,
    /// BG2 Control
    pub bg2cnt: u16,
    /// BG3 Control
    pub bg3cnt: u16,
    /// BG0 `X-Offset`
    pub bg0hofs: u16,
    /// BG0 `Y_Offset`
    pub bg0vofs: u16,
    /// BG1 `X-Offset`
    pub bg1hofs: u16,
    /// BG1 `Y_Offset`
    pub bg1vofs: u16,
    /// BG2 `X-Offset`
    pub bg2hofs: u16,
    /// BG2 `Y_Offset`
    pub bg2vofs: u16,
    /// BG3 `X-Offset`
    pub bg3hofs: u16,
    /// BG3 `Y_Offset`
    pub bg3vofs: u16,
    /// BG2 Rotation/Scaling Parameter A (dx)
    pub bg2pa: u16,
    /// BG2 Rotation/Scaling Parameter B (dmx)
    pub bg2pb: u16,
    /// BG2 Rotation/Scaling Parameter C (dy)
    pub bg2pc: u16,
    /// BG2 Rotation/Scaling Parameter D (dmy)
    pub bg2pd: u16,
    /// BG2 Reference Point X-Coordinate
    pub bg2x: u32,
    /// BG2 Reference Point Y-Coordinate
    pub bg2y: u32,
    /// BG3 Rotation/Scaling Parameter A (dx)
    pub bg3pa: u16,
    /// BG3 Rotation/Scaling Parameter B (dmx)
    pub bg3pb: u16,
    /// BG3 Rotation/Scaling Parameter C (dy)
    pub bg3pc: u16,
    /// BG3 Rotation/Scaling Parameter D (dmy)
    pub bg3pd: u16,
    /// BG3 Reference Point X-Coordinate
    pub bg3x: u32,
    /// BG3 Reference Point Y-Coordinate
    pub bg3y: u32,
    /// Window 0 Horizontal Dimensions
    pub win0h: u16,
    /// Window 1 Horizontal Dimensions
    pub win1h: u16,
    /// Window 0 Vertical Dimensions
    pub win0v: u16,
    /// Window 1 Vertical Dimensions
    pub win1v: u16,
    /// Inside of Window 0 and 1
    pub winin: u16,
    /// Inside of OBJ Window & Outside of Windows
    pub winout: u16,
    /// Mosaic Size
    pub mosaic: u16,
    /// Color Special Effects Selection
    pub bldcnt: u16,
    /// Alpha Blending Coefficients
    pub bldalpha: u16,
    /// Brightness (Fade-In/Out) Coefficient
    pub bldy: u16,
}

impl Registers {
    pub(super) fn get_bg0_enabled(&self) -> bool {
        self.dispcnt.get_bit(8)
    }

    pub(super) fn get_bg1_enabled(&self) -> bool {
        self.dispcnt.get_bit(9)
    }

    pub(super) fn get_bg2_enabled(&self) -> bool {
        self.dispcnt.get_bit(10)
    }

    pub(super) fn get_bg3_enabled(&self) -> bool {
        self.dispcnt.get_bit(11)
    }

    pub(super) fn get_obj_enabled(&self) -> bool {
        self.dispcnt.get_bit(12)
    }

    /// Info about vram fields used to render display.
    pub(super) fn get_bg_mode(&self) -> u8 {
        self.dispcnt.get_bits(0..=2).try_into().unwrap()
    }

    pub(super) fn get_obj_character_vram_mapping(&self) -> ObjMappingKind {
        self.dispcnt.get_bit(6).into()
    }

    pub(super) fn get_vcount_setting(&self) -> u8 {
        self.dispstat.get_byte(1)
    }

    pub(super) fn get_vblank_irq_enable(&self) -> bool {
        self.dispstat.get_bit(3)
    }

    pub(super) fn get_hblank_irq_enable(&self) -> bool {
        self.dispstat.get_bit(4)
    }

    pub(super) fn get_vcounter_irq_enable(&self) -> bool {
        self.dispstat.get_bit(5)
    }

    pub(super) fn set_vblank_flag(&mut self, value: bool) {
        self.dispstat.set_bit(0, value);
    }

    pub(super) fn set_hblank_flag(&mut self, value: bool) {
        self.dispstat.set_bit(1, value);
    }

    pub(super) fn set_vcounter_flag(&mut self, value: bool) {
        self.dispstat.set_bit(2, value);
    }

    /// Get BG0 priority (0-3, lower = higher priority).
    pub(super) fn get_bg0_priority(&self) -> u8 {
        self.bg0cnt.get_bits(0..=1) as u8
    }

    /// Get BG0 character base block (0-3).
    ///
    /// Each block is 16KB starting at VRAM `0x0600_0000`.
    /// Block N starts at offset N × 0x4000.
    pub(super) fn get_bg0_character_base_block(&self) -> u8 {
        self.bg0cnt.get_bits(2..=3) as u8
    }

    /// Get BG0 screen base block (0-31).
    ///
    /// Each block is 2KB (one 32×32 tilemap). Block N starts at VRAM offset N × 0x800.
    pub(super) fn get_bg0_screen_base_block(&self) -> u8 {
        self.bg0cnt.get_bits(8..=12) as u8
    }

    /// Get BG0 color mode.
    ///
    /// Returns `false` for 4bpp (16 colors per palette), `true` for 8bpp (256 colors).
    pub(super) fn get_bg0_color_mode(&self) -> bool {
        self.bg0cnt.get_bit(7)
    }

    /// Get BG0 screen size in pixels for text mode backgrounds.
    ///
    /// Text backgrounds can span multiple screen blocks arranged in a grid:
    ///
    /// | Size bits | Dimensions | Tiles  | Screen blocks         |
    /// |-----------|------------|--------|-----------------------|
    /// | 0         | 256×256    | 32×32  | 1 block               |
    /// | 1         | 512×256    | 64×32  | 2 blocks horizontal   |
    /// | 2         | 256×512    | 32×64  | 2 blocks vertical     |
    /// | 3         | 512×512    | 64×64  | 4 blocks (2×2 grid)   |
    ///
    /// The tilemap wraps at these boundaries when scrolling.
    pub(super) fn get_bg0_screen_size(&self) -> (usize, usize) {
        match self.bg0cnt.get_bits(14..=15) {
            0 => (256, 256),
            1 => (512, 256),
            2 => (256, 512),
            3 => (512, 512),
            _ => unreachable!(),
        }
    }

    /// Get BG1 priority (0-3, lower = higher priority).
    pub(super) fn get_bg1_priority(&self) -> u8 {
        self.bg1cnt.get_bits(0..=1) as u8
    }

    /// Get BG1 character base block (0-3).
    pub(super) fn get_bg1_character_base_block(&self) -> u8 {
        self.bg1cnt.get_bits(2..=3) as u8
    }

    /// Get BG1 screen base block (0-31).
    pub(super) fn get_bg1_screen_base_block(&self) -> u8 {
        self.bg1cnt.get_bits(8..=12) as u8
    }

    /// Get BG1 color mode (`false` = 4bpp, `true` = 8bpp).
    pub(super) fn get_bg1_color_mode(&self) -> bool {
        self.bg1cnt.get_bit(7)
    }

    /// Get BG1 screen size in pixels. See [`get_bg0_screen_size`](Self::get_bg0_screen_size).
    pub(super) fn get_bg1_screen_size(&self) -> (usize, usize) {
        match self.bg1cnt.get_bits(14..=15) {
            0 => (256, 256),
            1 => (512, 256),
            2 => (256, 512),
            3 => (512, 512),
            _ => unreachable!(),
        }
    }

    /// Get BG2 priority (0-3, lower = higher priority).
    pub(super) fn get_bg2_priority(&self) -> u8 {
        self.bg2cnt.get_bits(0..=1) as u8
    }

    /// Get BG2 character base block (0-3).
    pub(super) fn get_bg2_character_base_block(&self) -> u8 {
        self.bg2cnt.get_bits(2..=3) as u8
    }

    /// Get BG2 screen base block (0-31).
    pub(super) fn get_bg2_screen_base_block(&self) -> u8 {
        self.bg2cnt.get_bits(8..=12) as u8
    }

    /// Get BG2 color mode (`false` = 4bpp, `true` = 8bpp).
    pub(super) fn get_bg2_color_mode(&self) -> bool {
        self.bg2cnt.get_bit(7)
    }

    /// Get BG2 screen size in pixels for text mode. See [`get_bg0_screen_size`](Self::get_bg0_screen_size).
    ///
    /// Note: In affine modes (1-2), BG2 uses different size encoding.
    pub(super) fn get_bg2_screen_size(&self) -> (usize, usize) {
        match self.bg2cnt.get_bits(14..=15) {
            0 => (256, 256),
            1 => (512, 256),
            2 => (256, 512),
            3 => (512, 512),
            _ => unreachable!(),
        }
    }

    /// Get BG3 priority (0-3, lower = higher priority).
    pub(super) fn get_bg3_priority(&self) -> u8 {
        self.bg3cnt.get_bits(0..=1) as u8
    }

    /// Get BG3 character base block (0-3).
    pub(super) fn get_bg3_character_base_block(&self) -> u8 {
        self.bg3cnt.get_bits(2..=3) as u8
    }

    /// Get BG3 screen base block (0-31).
    pub(super) fn get_bg3_screen_base_block(&self) -> u8 {
        self.bg3cnt.get_bits(8..=12) as u8
    }

    /// Get BG3 color mode (`false` = 4bpp, `true` = 8bpp).
    pub(super) fn get_bg3_color_mode(&self) -> bool {
        self.bg3cnt.get_bit(7)
    }

    /// Get BG3 screen size in pixels for text mode. See [`get_bg0_screen_size`](Self::get_bg0_screen_size).
    pub(super) fn get_bg3_screen_size(&self) -> (usize, usize) {
        match self.bg3cnt.get_bits(14..=15) {
            0 => (256, 256),
            1 => (512, 256),
            2 => (256, 512),
            3 => (512, 512),
            _ => unreachable!(),
        }
    }

    // The GBA window system allows rectangular regions of the screen to have
    // independent layer visibility. There are 3 window types:
    //
    // - WIN0/WIN1: Rectangular windows defined by coordinates
    // - WINOBJ: Masked by sprite pixels with GfxMode::ObjectWindow
    //
    // Priority: WIN0 > WIN1 > WINOBJ > Outside (WINOUT)
    //
    // Each window has a set of enable bits controlling which layers (BG0-3, OBJ)
    // and effects are visible within that window region.

    /// Check if Window 0 is enabled (DISPCNT bit 13).
    pub(super) fn get_win0_enabled(&self) -> bool {
        self.dispcnt.get_bit(13)
    }

    /// Check if Window 1 is enabled (DISPCNT bit 14).
    pub(super) fn get_win1_enabled(&self) -> bool {
        self.dispcnt.get_bit(14)
    }

    /// Check if Object Window is enabled (DISPCNT bit 15).
    pub(super) fn get_winobj_enabled(&self) -> bool {
        self.dispcnt.get_bit(15)
    }

    // Window 0 boundary coordinates (from WIN0H and WIN0V registers)
    fn get_win0_left(&self) -> u8 {
        self.win0h.get_byte(1)
    }
    fn get_win0_right(&self) -> u8 {
        self.win0h.get_byte(0)
    }
    fn get_win0_top(&self) -> u8 {
        self.win0v.get_byte(1)
    }
    fn get_win0_bottom(&self) -> u8 {
        self.win0v.get_byte(0)
    }

    // Window 1 boundary coordinates (from WIN1H and WIN1V registers)
    fn get_win1_left(&self) -> u8 {
        self.win1h.get_byte(1)
    }
    fn get_win1_right(&self) -> u8 {
        self.win1h.get_byte(0)
    }
    fn get_win1_top(&self) -> u8 {
        self.win1v.get_byte(1)
    }
    fn get_win1_bottom(&self) -> u8 {
        self.win1v.get_byte(0)
    }

    /// Get layer enable flags for pixels inside Window 0.
    ///
    /// Returns tuple of (BG0, BG1, BG2, BG3, OBJ, color effects) enable flags.
    /// When a flag is false, that layer is hidden within WIN0's bounds.
    pub(super) fn get_win0_enables(&self) -> (bool, bool, bool, bool, bool, bool) {
        (
            self.winin.get_bit(0),
            self.winin.get_bit(1),
            self.winin.get_bit(2),
            self.winin.get_bit(3),
            self.winin.get_bit(4),
            self.winin.get_bit(5),
        )
    }

    /// Get layer enable flags for pixels inside Window 1.
    ///
    /// Returns tuple of (BG0, BG1, BG2, BG3, OBJ, color effects) enable flags.
    pub(super) fn get_win1_enables(&self) -> (bool, bool, bool, bool, bool, bool) {
        (
            self.winin.get_bit(8),
            self.winin.get_bit(9),
            self.winin.get_bit(10),
            self.winin.get_bit(11),
            self.winin.get_bit(12),
            self.winin.get_bit(13),
        )
    }

    /// Get layer enable flags for pixels outside all windows.
    ///
    /// Returns tuple of (BG0, BG1, BG2, BG3, OBJ, color effects) enable flags.
    /// These settings apply when a pixel is not in WIN0, WIN1, or WINOBJ.
    pub(super) fn get_winout_enables(&self) -> (bool, bool, bool, bool, bool, bool) {
        (
            self.winout.get_bit(0),
            self.winout.get_bit(1),
            self.winout.get_bit(2),
            self.winout.get_bit(3),
            self.winout.get_bit(4),
            self.winout.get_bit(5),
        )
    }

    /// Get layer enable flags for pixels inside Object Window.
    ///
    /// Returns tuple of (BG0, BG1, BG2, BG3, OBJ, color effects) enable flags.
    /// WINOBJ applies to pixels covered by sprites with `GfxMode::ObjectWindow`.
    pub(super) fn get_winobj_enables(&self) -> (bool, bool, bool, bool, bool, bool) {
        (
            self.winout.get_bit(8),
            self.winout.get_bit(9),
            self.winout.get_bit(10),
            self.winout.get_bit(11),
            self.winout.get_bit(12),
            self.winout.get_bit(13),
        )
    }

    /// Check if a screen coordinate is inside Window 0's bounds.
    ///
    /// Window coordinates handle wrap-around: if right < left, the window
    /// extends from left edge to screen right, then wraps to screen left up to right.
    /// Same logic applies vertically.
    pub(super) fn is_in_win0(&self, x: u8, y: u8) -> bool {
        let left = self.get_win0_left();
        let right = self.get_win0_right();
        let top = self.get_win0_top();
        let bottom = self.get_win0_bottom();

        in_horizontal_range(left, right, x) && in_vertical_range(top, bottom, y)
    }

    /// Check if a screen coordinate is inside Window 1's bounds.
    ///
    /// See [`is_in_win0`](Self::is_in_win0) for wrap-around behavior.
    pub(super) fn is_in_win1(&self, x: u8, y: u8) -> bool {
        let left = self.get_win1_left();
        let right = self.get_win1_right();
        let top = self.get_win1_top();
        let bottom = self.get_win1_bottom();

        in_horizontal_range(left, right, x) && in_vertical_range(top, bottom, y)
    }

    /// Get the blend mode from BLDCNT.
    /// 0 = Off, 1 = Alpha blend, 2 = Brightness increase (white), 3 = Brightness decrease (black)
    pub(super) fn get_blend_mode(&self) -> u8 {
        self.bldcnt.get_bits(6..=7) as u8
    }

    /// Get first target layers for blending (BLDCNT bits 0-5).
    /// Returns (BG0, BG1, BG2, BG3, OBJ, Backdrop) as targets.
    pub(super) fn get_blend_target1(&self) -> (bool, bool, bool, bool, bool, bool) {
        (
            self.bldcnt.get_bit(0), // BG0
            self.bldcnt.get_bit(1), // BG1
            self.bldcnt.get_bit(2), // BG2
            self.bldcnt.get_bit(3), // BG3
            self.bldcnt.get_bit(4), // OBJ
            self.bldcnt.get_bit(5), // Backdrop
        )
    }

    /// Get second target layers for alpha blending (BLDCNT bits 8-13).
    /// Returns (BG0, BG1, BG2, BG3, OBJ, Backdrop) as targets.
    pub(super) fn get_blend_target2(&self) -> (bool, bool, bool, bool, bool, bool) {
        (
            self.bldcnt.get_bit(8),  // BG0
            self.bldcnt.get_bit(9),  // BG1
            self.bldcnt.get_bit(10), // BG2
            self.bldcnt.get_bit(11), // BG3
            self.bldcnt.get_bit(12), // OBJ
            self.bldcnt.get_bit(13), // Backdrop
        )
    }

    /// Get alpha blending coefficients (EVA, EVB) from BLDALPHA.
    /// Both are 0-16, representing 0/16 to 16/16 blend factors.
    pub(super) fn get_blend_alpha(&self) -> (u8, u8) {
        let eva = (self.bldalpha.get_bits(0..=4) as u8).min(16);
        let evb = (self.bldalpha.get_bits(8..=12) as u8).min(16);
        (eva, evb)
    }

    /// Get brightness coefficient (EVY) from BLDY.
    /// Range 0-16, representing fade amount (0 = none, 16 = full white/black).
    pub(super) fn get_blend_brightness(&self) -> u8 {
        (self.bldy.get_bits(0..=4) as u8).min(16)
    }
}

/// Check if x is within horizontal window bounds [left, right).
///
/// Handles wrap-around: if right < left, the range wraps around the screen edge
/// (i.e., x >= left OR x < right). Returns false if left == right (empty window).
fn in_horizontal_range(left: u8, right: u8, x: u8) -> bool {
    match right.cmp(&left) {
        std::cmp::Ordering::Greater => x >= left && x < right,
        std::cmp::Ordering::Less => x >= left || x < right,
        std::cmp::Ordering::Equal => false,
    }
}

/// Check if y is within vertical window bounds [top, bottom).
///
/// Handles wrap-around: if bottom < top, the range wraps around the screen edge.
/// Returns false if top == bottom (empty window).
fn in_vertical_range(top: u8, bottom: u8, y: u8) -> bool {
    match bottom.cmp(&top) {
        std::cmp::Ordering::Greater => y >= top && y < bottom,
        std::cmp::Ordering::Less => y >= top || y < bottom,
        std::cmp::Ordering::Equal => false,
    }
}
