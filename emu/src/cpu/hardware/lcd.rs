use crate::bitwise::Bits;

pub struct Lcd {
    /// LCD Control
    pub dispcnt: u16,
    /// Undocumented - Green Swap
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
    /// BG0 X-Offset
    pub bg0hofs: u16,
    /// BG0 Y_Offset
    pub bg0vofs: u16,
    /// BG1 X-Offset
    pub bg1hofs: u16,
    /// BG1 Y_Offset
    pub bg1vofs: u16,
    /// BG2 X-Offset
    pub bg2hofs: u16,
    /// BG2 Y_Offset
    pub bg2vofs: u16,
    /// BG3 X-Offset
    pub bg3hofs: u16,
    /// BG3 Y_Offset
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

    /// From 0x05000000 to  0x050001FF (512 bytes, 256 colors).
    pub bg_palette_ram: Vec<u8>,
    /// From 0x05000200 to 0x050003FF (512 bytes, 256 colors).
    pub obj_palette_ram: Vec<u8>,
    /// From 0x06000000 to 0x06017FFF (96 kb).
    pub video_ram: Vec<u8>,
    /// From 0x07000000 to 0x070003FF (1kbyte)
    pub obj_attributes: Vec<u8>,

    pixel_index: u32,
}

impl Default for Lcd {
    fn default() -> Self {
        Self {
            dispcnt: 0,
            green_swap: 0,
            dispstat: 0,
            vcount: 0,
            bg0cnt: 0,
            bg1cnt: 0,
            bg2cnt: 0,
            bg3cnt: 0,
            bg0hofs: 0,
            bg0vofs: 0,
            bg1hofs: 0,
            bg1vofs: 0,
            bg2hofs: 0,
            bg2vofs: 0,
            bg3hofs: 0,
            bg3vofs: 0,
            bg2pa: 0,
            bg2pb: 0,
            bg2pc: 0,
            bg2pd: 0,
            bg2x: 0,
            bg2y: 0,
            bg3pa: 0,
            bg3pb: 0,
            bg3pc: 0,
            bg3pd: 0,
            bg3x: 0,
            bg3y: 0,
            win0h: 0,
            win1h: 0,
            win0v: 0,
            win1v: 0,
            winin: 0,
            winout: 0,
            mosaic: 0,
            bldcnt: 0,
            bldalpha: 0,
            bldy: 0,
            bg_palette_ram: vec![0; 0x200],
            obj_palette_ram: vec![0; 0x200],
            video_ram: vec![0; 0x00018000],
            obj_attributes: vec![0; 0x400],
            pixel_index: 0,
        }
    }
}
#[derive(Default)]
pub struct LcdStepOutput {
    pub request_vblank_irq: bool,
    pub request_hblank_irq: bool,
    pub request_vcount_irq: bool,
}

impl Lcd {
    pub fn step(&mut self) -> LcdStepOutput {
        // This will be much more complex obviously
        let mut output = LcdStepOutput::default();

        if self.vcount < 160 {
            // We either are in Vdraw or Hblank
            if self.pixel_index == 0 {
                // We're drawing the first pixel of the scanline, we're entering Vdraw

                self.set_hblank_flag(false);
                self.set_vblank_flag(false);

                // TODO: Do something
            } else if self.pixel_index == 240 {
                // We're entering Hblank

                self.set_hblank_flag(true);

                if self.get_hblank_irq_enable() {
                    output.request_hblank_irq = true;
                }
            }
        } else if self.vcount == 160 && self.pixel_index == 0 {
            // We're drawing the first pixel of the Vdraw period

            self.set_vblank_flag(true);

            if self.get_vblank_irq_enable() {
                output.request_vblank_irq = true;
            }
        }

        // TODO: draw the pixel

        self.pixel_index += 1;

        if self.pixel_index == 308 {
            // We finished to draw the scanline
            self.pixel_index = 0;
            self.vcount += 1;

            // We finished to draw the screen
            if self.vcount == 228 {
                self.vcount = 0;
            }
        }

        self.set_vcounter_flag(false);

        if self.vcount.get_byte(0) == self.get_vcount_setting() {
            self.set_vcounter_flag(true);

            if self.get_vcounter_irq_enable() {
                output.request_vcount_irq = true;
            }
        }

        output
    }

    /// Info about vram fields used to render display.
    pub fn get_bg_mode(&self) -> u8 {
        self.dispcnt.get_bits(0..=2).try_into().unwrap()
    }

    fn get_vcount_setting(&self) -> u8 {
        self.dispstat.get_byte(1)
    }

    fn get_vblank_irq_enable(&self) -> bool {
        self.dispstat.get_bit(3)
    }

    fn get_hblank_irq_enable(&self) -> bool {
        self.dispstat.get_bit(4)
    }

    fn get_vcounter_irq_enable(&self) -> bool {
        self.dispstat.get_bit(5)
    }

    fn set_vblank_flag(&mut self, value: bool) {
        self.dispstat.set_bit(0, value);
    }

    fn set_hblank_flag(&mut self, value: bool) {
        self.dispstat.set_bit(1, value);
    }

    fn set_vcounter_flag(&mut self, value: bool) {
        self.dispstat.set_bit(2, value);
    }
}
