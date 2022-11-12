/// This module contains all the data structures used to render the GBA display.
pub mod color;
pub mod gba_lcd;
pub mod ppu;

/// GBA display width
pub const LCD_WIDTH: usize = 240;

/// GBA display height
pub const LCD_HEIGHT: usize = 160;

/// GBC display width
pub const GBC_LCD_WIDTH: usize = 160;

/// GBC display height
pub const GBC_LCD_HEIGHT: usize = 128;

/// Each palette can contains 16 colors
pub const MAX_COLORS_SINGLE_PALETTE: usize = 16;

/// BG palettes and OBJ palettes can be use as a single palette
pub const MAX_COLORS_FULL_PALETTE: usize = 256;

/// Number of max palettes both for BG and OBG
pub const MAX_PALETTES_BY_TYPE: usize = 16;

/// Memory info about BG palette
pub const BG_PALETTE_ADDRESS: u32 = 0x05000000;
pub const _BG_PALETTE_SIZE: usize = 0x200;

/// Memory info about OBJ palette
pub const OBJ_PALETTE_ADDRESS: u32 = 0x05000200;
pub const _OBJ_PALETTE_SIZE: usize = 0x200;
