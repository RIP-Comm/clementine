//! # Emulator Thread
//!
//! This module implements a dedicated thread for running the GBA emulator,
//! communicating with the UI thread via lock-free SPSC channels.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────┐              ┌─────────────────────────────┐
//! │      CPU Thread         │              │        UI Thread            │
//! │                         │              │                             │
//! │  ┌─────────────────┐    │   Commands   │    ┌─────────────────────┐  │
//! │  │      Gba        │    │ ◄─────────── │    │     EmuHandle       │  │
//! │  │  (owned here)   │    │   (SPSC)     │    │                     │  │
//! │  └────────┬────────┘    │              │    │  - send commands    │  │
//! │           │             │   Events     │    │  - poll events      │  │
//! │           ▼             │ ───────────► │    │  - read state       │  │
//! │  loop {                 │   (SPSC)     │    └─────────────────────┘  │
//! │    process commands     │              │                             │
//! │    if running:          │   Disasm     │                             │
//! │      gba.step()         │ ───────────► │                             │
//! │      send events        │   (SPSC)     │                             │
//! │  }                      │              │                             │
//! └─────────────────────────┘              └─────────────────────────────┘
//! ```
//!
//! ## Communication
//!
//! - **Commands** (UI → CPU): `EmuCommand` enum for control (run, pause, step, etc.)
//! - **Events** (CPU → UI): `EmuEvent` enum for state updates and frames
//! - **Disasm** (CPU → UI): `DisasmEntry` for disassembler (reuses existing channel)

use std::collections::BTreeSet;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub use emu::cpu::hardware::keypad::GbaButton;
use emu::cpu::DisasmEntry;
use emu::gba::Gba;

/// GBA LCD dimensions
pub const LCD_WIDTH: usize = 240;
pub const LCD_HEIGHT: usize = 160;

/// Number of CPU cycles to run per batch before checking commands.
/// This only affects responsiveness to pause/step commands.
/// Frame sending is triggered by `VBlank` (~every 280,896 cycles).
/// Larger values = better performance but less responsive to pause commands.
const CYCLES_PER_BATCH: u32 = 10000;

/// Target frame duration for ~59.73 FPS
/// 1 / 59.7275 ≈ 16.74ms
const FRAME_DURATION: Duration = Duration::from_micros(16_742);

/// Channel buffer sizes
const COMMAND_BUFFER_SIZE: usize = 64;
const EVENT_BUFFER_SIZE: usize = 64;

/// Commands sent from the UI thread to the emulator thread.
#[derive(Debug, Clone)]
pub enum EmuCommand {
    /// Run continuously until paused or breakpoint hit.
    Run,
    /// Pause execution.
    Pause,
    /// Step N cycles then pause.
    Step(u32),
    /// Add a breakpoint at address.
    AddBreakpoint { address: u32, kind: BreakpointKind },
    /// Remove a breakpoint at address.
    RemoveBreakpoint(u32),
    /// Request a full state snapshot.
    RequestState,
    /// Load a save state.
    LoadState(Vec<u8>),
    /// Request save state data.
    RequestSaveState,
    /// Set button state (pressed or released).
    SetKey { button: GbaButton, pressed: bool },
    /// Read memory at address for given length.
    ReadMemory { address: u32, length: usize },
    /// Write a byte to memory.
    WriteByte { address: u32, value: u8 },
    /// Enable or disable disassembly output.
    SetDisasmEnabled(bool),
    /// Set emulation speed multiplier.
    SetSpeed(f32),
    /// Shutdown the emulator thread.
    Shutdown,
}

/// Type of breakpoint condition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BreakpointKind {
    /// Break when PC equals the address.
    Equal,
    /// Break when PC is greater than the address.
    GreaterThan,
}

/// Events sent from the emulator thread to the UI thread.
#[derive(Debug, Clone)]
pub enum EmuEvent {
    /// State snapshot for UI display.
    State(EmuState),
    /// A new LCD frame is ready.
    Frame(Box<FrameBuffer>),
    /// Emulator paused.
    Paused { reason: PauseReason },
    /// Save state data.
    SaveStateData(Vec<u8>),
    /// Memory data response.
    MemoryData { address: u32, data: Vec<u8> },
}

/// Snapshot of emulator state for UI display.
#[derive(Debug, Clone, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct EmuState {
    /// General purpose registers R0-R15.
    pub registers: [u32; 16],
    /// Current Program Status Register.
    pub cpsr: u32,
    /// Saved Program Status Register.
    pub spsr: u32,
    /// Current cycle count.
    pub cycle: u128,
    /// Whether the emulator is currently running.
    pub is_running: bool,
    /// Cartridge game title (12 chars max).
    pub cartridge_title: String,
    /// Cartridge game code (4 chars, e.g., "BPEE" for Pokemon Emerald).
    pub game_code: String,
    /// Maker/publisher code (2 chars, e.g., "01" for Nintendo).
    pub maker_code: String,
    /// Software version.
    pub software_version: u8,
    /// Keypad input register (KEYINPUT). Bits 0-9 represent buttons.
    /// Bit = 0 means pressed, bit = 1 means released (active-low).
    pub key_input: u16,

    /// Whether the Nintendo logo in the header is valid.
    pub logo_valid: bool,
    /// Whether the header checksum is valid.
    pub checksum_valid: bool,
    /// Whether the fixed value (0x96) is valid.
    pub fixed_value_valid: bool,
    /// Decoded entry point address from ARM branch instruction.
    pub entry_point: u32,
    /// Whether the entry point looks like a valid branch instruction.
    pub entry_point_valid: bool,
}

/// Reason why the emulator paused.
#[derive(Debug, Clone)]
pub enum PauseReason {
    /// User requested pause.
    User,
    /// Hit a breakpoint at the given address.
    Breakpoint(u32),
    /// Completed requested step count.
    Step,
}

/// LCD frame buffer - RGB values for each pixel.
/// Stored as [R, G, B] for each pixel, row by row.
pub type FrameBuffer = [u8; LCD_WIDTH * LCD_HEIGHT * 3];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Breakpoint {
    address: u32,
    kind: BreakpointKind,
}

/// The emulator thread that owns and runs the GBA.
struct EmuThread {
    gba: Gba,
    cmd_rx: rtrb::Consumer<EmuCommand>,
    event_tx: rtrb::Producer<EmuEvent>,

    // State
    running: bool,
    steps_remaining: u32,
    breakpoints: BTreeSet<Breakpoint>,

    /// Timestamp when the last frame started (for frame rate limiting).
    frame_start: Instant,

    /// Speed multiplier (1.0 = normal, 0.0 = uncapped).
    speed_multiplier: f32,

    /// Frame counter for frame skipping at high speeds.
    frame_counter: u32,
}

impl EmuThread {
    fn new(
        gba: Gba,
        cmd_rx: rtrb::Consumer<EmuCommand>,
        event_tx: rtrb::Producer<EmuEvent>,
    ) -> Self {
        Self {
            gba,
            cmd_rx,
            event_tx,
            running: false,
            steps_remaining: 0,
            breakpoints: BTreeSet::new(),
            frame_start: Instant::now(),
            speed_multiplier: 1.0,
            frame_counter: 0,
        }
    }

    fn run(mut self) {
        loop {
            if self.process_commands() {
                return; // shutdown
            }

            if self.running || self.steps_remaining > 0 {
                self.execute_batch();
            } else {
                // sleep briefly to avoid busy-waiting
                thread::sleep(Duration::from_millis(1));
            }
        }
    }

    /// Process all pending commands. Returns true if should shutdown.
    fn process_commands(&mut self) -> bool {
        while let Ok(cmd) = self.cmd_rx.pop() {
            match cmd {
                EmuCommand::Run => {
                    self.running = true;
                    self.steps_remaining = 0;
                }
                EmuCommand::Pause => {
                    self.running = false;
                    self.steps_remaining = 0;
                    self.send_event(EmuEvent::Paused {
                        reason: PauseReason::User,
                    });
                    self.send_state();
                }
                EmuCommand::Step(count) => {
                    self.running = false;
                    self.steps_remaining = count;
                }
                EmuCommand::AddBreakpoint { address, kind } => {
                    self.breakpoints.insert(Breakpoint { address, kind });
                }
                EmuCommand::RemoveBreakpoint(address) => {
                    self.breakpoints.retain(|b| b.address != address);
                }
                EmuCommand::RequestState => {
                    self.send_state();
                }
                EmuCommand::LoadState(data) => {
                    // save channel of dis-ASM before replacing CPU state
                    let disasm_tx = self.gba.cpu.disasm_tx.take();

                    match bincode::deserialize(&data) {
                        Ok(cpu) => {
                            self.gba.cpu = cpu;
                            self.gba.cpu.disasm_tx = disasm_tx;
                            tracing::info!("Save state loaded successfully");
                        }
                        Err(e) => {
                            self.gba.cpu.disasm_tx = disasm_tx;
                            tracing::error!("Failed to load save state: {e}");
                        }
                    }
                    self.send_state();
                    self.send_frame();
                }
                EmuCommand::RequestSaveState => match bincode::serialize(&self.gba.cpu) {
                    Ok(data) => {
                        tracing::info!("Save state created: {} bytes", data.len());
                        self.send_event(EmuEvent::SaveStateData(data));
                    }
                    Err(e) => {
                        tracing::error!("Failed to create save state: {e}");
                    }
                },
                EmuCommand::SetKey { button, pressed } => {
                    self.gba.cpu.bus.keypad.set_button(button, pressed);
                }
                EmuCommand::ReadMemory { address, length } => {
                    let mut data = Vec::with_capacity(length);
                    for i in 0..length {
                        // Loop index is small (memory inspector fetches max 256 bytes)
                        #[allow(clippy::cast_possible_truncation)]
                        let addr = address.wrapping_add(i as u32);
                        data.push(self.gba.cpu.bus.read_byte(addr as usize));
                    }
                    self.send_event(EmuEvent::MemoryData { address, data });
                }
                EmuCommand::WriteByte { address, value } => {
                    self.gba.cpu.bus.write_byte(address as usize, value);
                }
                EmuCommand::SetDisasmEnabled(enabled) => {
                    if enabled {
                    } else {
                        // Disable by dropping the transmitter
                        self.gba.cpu.disasm_tx = None;
                    }
                }
                EmuCommand::SetSpeed(multiplier) => {
                    self.speed_multiplier = multiplier;
                }
                EmuCommand::Shutdown => {
                    return true;
                }
            }
        }
        false
    }

    /// Execute a batch of CPU cycles.
    ///
    /// Runs up to `CYCLES_PER_BATCH` cycles, then returns to check for commands.
    /// Frames and state are sent automatically when `VBlank` starts (natural frame boundary).
    fn execute_batch(&mut self) {
        let has_breakpoints = !self.breakpoints.is_empty();
        let is_stepping = self.steps_remaining > 0;

        for _ in 0..CYCLES_PER_BATCH {
            // Check breakpoints before executing (only if any are set)
            if has_breakpoints {
                // GBA has 32-bit address space
                #[allow(clippy::cast_possible_truncation)]
                let pc = self.gba.cpu.registers.program_counter() as u32;
                if let Some(bp) = self.check_breakpoint(pc) {
                    self.running = false;
                    self.steps_remaining = 0;
                    self.send_event(EmuEvent::Paused {
                        reason: PauseReason::Breakpoint(bp.address),
                    });
                    self.send_state();
                    return;
                }
            }

            // Execute 1 cycle - returns true when VBlank starts
            let vblank_started = self.gba.step();

            // On VBlank: send frame, state update, and handle timing
            if vblank_started {
                self.frame_counter += 1;

                // Calculate frame skip to maintain ~60 FPS display regardless of emulation speed
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let frame_skip = if self.speed_multiplier == 0.0 {
                    // Uncapped: skip frames but still update display at ~60 FPS
                    // We'll send frame based on elapsed time instead
                    u32::MAX
                } else if self.speed_multiplier > 1.0 {
                    // Fast forward: skip frames proportionally
                    self.speed_multiplier.round() as u32
                } else {
                    // Normal/slow speed: send every frame
                    1
                };

                // For uncapped mode, send frame based on real time (~60 FPS)
                // For other modes, send based on frame counter
                let should_send_frame = if self.speed_multiplier == 0.0 {
                    // Uncapped: send frame if 16ms has elapsed (60 FPS)
                    self.frame_start.elapsed() >= FRAME_DURATION
                } else {
                    // Fixed speed: send every Nth frame
                    self.frame_counter.is_multiple_of(frame_skip)
                };

                if should_send_frame {
                    self.send_frame();
                    self.send_state();
                    if self.speed_multiplier == 0.0 {
                        self.frame_start = Instant::now();
                    }
                }

                // Frame rate limiting (only for speeds <= 4x)
                // Above 4x, just run as fast as possible with frame skipping
                if self.speed_multiplier > 0.0 && self.speed_multiplier <= 4.0 {
                    let target_duration = FRAME_DURATION.div_f32(self.speed_multiplier);
                    let elapsed = self.frame_start.elapsed();
                    if let Some(sleep_duration) = target_duration.checked_sub(elapsed) {
                        thread::sleep(sleep_duration);
                    }
                    self.frame_start = Instant::now();
                }
            }

            // Handle stepping mode (only if stepping)
            if is_stepping {
                self.steps_remaining -= 1;
                if self.steps_remaining == 0 {
                    self.running = false;
                    self.send_event(EmuEvent::Paused {
                        reason: PauseReason::Step,
                    });
                    self.send_state();
                    return;
                }
            }
        }
    }

    /// Check if any breakpoint matches the current PC.
    fn check_breakpoint(&self, pc: u32) -> Option<Breakpoint> {
        for bp in &self.breakpoints {
            let matches = match bp.kind {
                BreakpointKind::Equal => pc == bp.address,
                BreakpointKind::GreaterThan => pc > bp.address,
            };
            if matches {
                return Some(*bp);
            }
        }
        None
    }

    /// Send current state to UI.
    fn send_state(&mut self) {
        let header = &self.gba.cartridge_header;
        let state = EmuState {
            registers: self.get_registers(),
            cpsr: u32::from(self.gba.cpu.cpsr),
            spsr: u32::from(self.gba.cpu.spsr),
            cycle: self.gba.cpu.current_cycle,
            is_running: self.running,
            cartridge_title: header.game_title.clone(),
            game_code: header.game_code.clone(),
            maker_code: header.marker_code.clone(),
            software_version: header.software_version,
            key_input: self.gba.cpu.bus.keypad.key_input,
            logo_valid: header.logo_valid,
            checksum_valid: header.checksum_valid,
            fixed_value_valid: header.fixed_value_valid,
            entry_point: header.entry_point_address(),
            entry_point_valid: header.has_valid_entry_point(),
        };
        self.send_event(EmuEvent::State(state));
    }

    /// Get all 16 registers as an array.
    fn get_registers(&self) -> [u32; 16] {
        let mut regs = [0u32; 16];
        for (i, reg) in regs.iter_mut().enumerate() {
            *reg = self.gba.cpu.registers.register_at(i);
        }
        regs
    }

    /// Send current LCD frame to UI.
    fn send_frame(&mut self) {
        #[allow(clippy::large_stack_arrays)]
        let mut frame = Box::new([0u8; LCD_WIDTH * LCD_HEIGHT * 3]);

        for (y, row) in self.gba.cpu.bus.lcd.buffer.iter().enumerate() {
            for (x, pixel) in row.iter().enumerate() {
                let idx = (y * LCD_WIDTH + x) * 3;
                // Convert 5-bit color to 8-bit
                frame[idx] = (pixel.red() << 3) | (pixel.red() >> 2);
                frame[idx + 1] = (pixel.green() << 3) | (pixel.green() >> 2);
                frame[idx + 2] = (pixel.blue() << 3) | (pixel.blue() >> 2);
            }
        }

        self.send_event(EmuEvent::Frame(frame));
    }

    /// Send an event to the UI (non-blocking, drops if full).
    fn send_event(&mut self, event: EmuEvent) {
        let _ = self.event_tx.push(event);
    }
}

/// Handle for the UI thread to communicate with the emulator thread.
pub struct EmuHandle {
    cmd_tx: rtrb::Producer<EmuCommand>,
    event_rx: rtrb::Consumer<EmuEvent>,
    disasm_rx: rtrb::Consumer<DisasmEntry>,
    thread_handle: Option<JoinHandle<()>>,

    /// Latest state snapshot from the emulator.
    pub state: EmuState,
    /// Latest frame from the emulator.
    pub frame: Option<Box<FrameBuffer>>,
    /// List of active breakpoints (mirrored from emu thread).
    pub breakpoints: Vec<(u32, BreakpointKind)>,
    /// Pending save state data (set when save is requested, cleared when taken).
    pub pending_save_state: Option<Vec<u8>>,
    /// Pending memory data (address and bytes read).
    pub pending_memory_data: Option<(u32, Vec<u8>)>,
    /// Current speed multiplier.
    pub speed: f32,
}

impl EmuHandle {
    /// Send a command to the emulator thread.
    pub fn send(&mut self, cmd: EmuCommand) {
        match &cmd {
            EmuCommand::AddBreakpoint { address, kind } => {
                self.breakpoints.push((*address, *kind));
                self.breakpoints.sort_by_key(|(addr, _)| *addr);
            }
            EmuCommand::RemoveBreakpoint(address) => {
                self.breakpoints.retain(|(addr, _)| addr != address);
            }
            EmuCommand::SetSpeed(multiplier) => {
                self.speed = *multiplier;
            }
            _ => {}
        }
        let _ = self.cmd_tx.push(cmd);
    }

    /// Poll for events and update cached state.
    pub fn poll(&mut self) {
        while let Ok(event) = self.event_rx.pop() {
            match event {
                EmuEvent::State(state) => {
                    self.state = state;
                }
                EmuEvent::Frame(frame) => {
                    self.frame = Some(frame);
                }
                EmuEvent::Paused { reason: _ } => {
                    self.state.is_running = false;
                }
                EmuEvent::SaveStateData(data) => {
                    tracing::info!("Received save state data: {} bytes", data.len());
                    self.pending_save_state = Some(data);
                }
                EmuEvent::MemoryData { address, data } => {
                    self.pending_memory_data = Some((address, data));
                }
            }
        }
    }

    /// Access the disassembler consumer directly.
    pub const fn disasm_rx(&mut self) -> &mut rtrb::Consumer<DisasmEntry> {
        &mut self.disasm_rx
    }
}

impl Drop for EmuHandle {
    fn drop(&mut self) {
        let _ = self.cmd_tx.push(EmuCommand::Shutdown);
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

/// Spawn the emulator thread and return a handle for communication.
///
/// # Arguments
/// * `gba` - The GBA instance to run (ownership transferred to the thread)
/// * `disasm_rx` - The disassembler consumer (taken from Gba before calling)
///
/// # Returns
/// An `EmuHandle` for sending commands and receiving events.
pub fn spawn(gba: Gba, disasm_rx: rtrb::Consumer<DisasmEntry>) -> EmuHandle {
    // Create command channel (UI → CPU)
    let (cmd_tx, cmd_rx) = rtrb::RingBuffer::new(COMMAND_BUFFER_SIZE);

    // Create event channel (CPU → UI)
    let (event_tx, event_rx) = rtrb::RingBuffer::new(EVENT_BUFFER_SIZE);

    // Get initial state before moving gba
    let header = &gba.cartridge_header;
    let initial_state = EmuState {
        registers: {
            let mut regs = [0u32; 16];
            for (i, reg) in regs.iter_mut().enumerate() {
                *reg = gba.cpu.registers.register_at(i);
            }
            regs
        },
        cpsr: u32::from(gba.cpu.cpsr),
        spsr: u32::from(gba.cpu.spsr),
        cycle: gba.cpu.current_cycle,
        is_running: false,
        cartridge_title: header.game_title.clone(),
        game_code: header.game_code.clone(),
        maker_code: header.marker_code.clone(),
        software_version: header.software_version,
        key_input: gba.cpu.bus.keypad.key_input,
        logo_valid: header.logo_valid,
        checksum_valid: header.checksum_valid,
        fixed_value_valid: header.fixed_value_valid,
        entry_point: header.entry_point_address(),
        entry_point_valid: header.has_valid_entry_point(),
    };

    // Spawn the emulator thread
    let thread_handle = thread::spawn(move || {
        let emu_thread = EmuThread::new(gba, cmd_rx, event_tx);
        emu_thread.run();
    });

    EmuHandle {
        cmd_tx,
        event_rx,
        disasm_rx,
        thread_handle: Some(thread_handle),
        state: initial_state,
        frame: None,
        breakpoints: Vec::new(),
        pending_save_state: None,
        pending_memory_data: None,
        speed: 1.0,
    }
}
