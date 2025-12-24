//! Background and sprite layer rendering.
//!
//! The GBA PPU composites multiple layers to produce the final image. This module
//! defines the [`Layer`] trait and provides implementations for each layer type.
//!
//! # Layer Types
//!
//! | Layer        | Module        | Description                                    |
//! |--------------|---------------|------------------------------------------------|
//! | BG0          | [`layer_0`]   | Regular tiled background (modes 0-1)           |
//! | BG1          | [`layer_1`]   | Regular tiled background (modes 0-1)           |
//! | BG2          | [`layer_2`]   | Text/Affine/Bitmap background (all modes)      |
//! | BG3          | [`layer_3`]   | Regular/Affine background (modes 0, 2)         |
//! | OBJ          | [`layer_obj`] | Sprites (128 objects, hardware transformed)    |
//!
//! # Rendering Pipeline
//!
//! For each pixel, the LCD controller:
//! 1. Calls [`Layer::render`] on each enabled layer
//! 2. Filters out transparent pixels (`None` results)
//! 3. Sorts by priority (lower = higher priority)
//! 4. Displays the topmost non-transparent pixel
//!
//! # Tile-Based Rendering
//!
//! Most layers use tile-based rendering (see [`layer_0`] for detailed docs):
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                     Tile Rendering Flow                         │
//! ├─────────────────────────────────────────────────────────────────┤
//! │  Screen Position (x, y)                                         │
//! │         │                                                       │
//! │         ▼                                                       │
//! │  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐      │
//! │  │ Add Scroll   │───▶│ Tilemap      │───▶│ Tile Data    │      │
//! │  │ Offset       │    │ Lookup       │    │ (Character)  │      │
//! │  └──────────────┘    └──────────────┘    └──────────────┘      │
//! │                             │                    │              │
//! │                             ▼                    ▼              │
//! │                      Tile Number,         Palette Index         │
//! │                      Flip Flags,                │               │
//! │                      Palette Bank               ▼               │
//! │                                          ┌──────────────┐       │
//! │                                          │ Palette RAM  │       │
//! │                                          │ Color Lookup │       │
//! │                                          └──────────────┘       │
//! │                                                 │               │
//! │                                                 ▼               │
//! │                                           Final Color           │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Transparency
//!
//! Palette index 0 is always transparent for both backgrounds and sprites.
//! Returning `None` from [`Layer::render`] indicates a transparent pixel.

use super::{PixelInfo, memory::Memory, registers::Registers};

pub mod layer_0;
pub mod layer_1;
pub mod layer_2;
pub mod layer_3;
pub mod layer_obj;

/// Trait for renderable display layers.
///
/// Each layer type implements this trait to participate in the PPU's
/// compositing pipeline. The LCD controller calls [`render`](Layer::render)
/// for each enabled layer at each pixel position.
pub trait Layer {
    /// Renders a single pixel at the given screen coordinates.
    ///
    /// # Arguments
    /// * `x` - Screen X coordinate (0-239)
    /// * `y` - Screen Y coordinate (0-159)
    /// * `memory` - Reference to VRAM and palette RAM
    /// * `registers` - LCD control registers
    ///
    /// # Returns
    /// * `Some(PixelInfo)` - The rendered color and priority
    /// * `None` - Pixel is transparent (palette index 0 or out of bounds)
    fn render(
        &self,
        x: usize,
        y: usize,
        memory: &Memory,
        registers: &Registers,
    ) -> Option<PixelInfo>;
}
