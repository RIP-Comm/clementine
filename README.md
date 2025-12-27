
![Alt text](img/clementine_logo_200px.png?raw=true "Clementine_logo")

[![Rust](https://github.com/RIP-Comm/clementine/actions/workflows/rust.yml/badge.svg)](https://github.com/RIP-Comm/clementine/actions/workflows/rust.yml)

![](./extra/init.gif)

# Clementine - A collaborative approach to GBA emulation

Welcome to the first ripsters' project. Our goal is to understand how GameBoy Advance works and to create a modern emulator written in Rust (if you want to collaborate but you can't code in Rust take a look [here](https://doc.rust-lang.org/book/)).

Everything is work in progress. We will update this document a lot of times in this stage.


## Collaborative Guidelines

We love collaborating with others, so feel free to interact with us however you want. First of all, we strongly suggest you to enter in our Discord channel where you can find all of us ([here](https://discord.com/channels/919139369774891088/1013367016666714112)).

[Contributing doc](./CONTRIBUTING.md)

[Resources](https://github.com/RIP-Comm/clementine/wiki/Resources)

## Build and quick start

- clone the repository :)
- we are using `just` and not `make` then if you want take the benefit of this install it `cargo install just`

> Tip: Run `just` to see all the available commands

```zsh
# quick check all is working on you machine
just build
just test

# run a .gba file (debug build)
just run ~/Desktop/my_game.gba
```

## Requirements

Before running the emulator, you need:

1. **GBA BIOS file**: A file named `gba_bios.bin` (16KB) placed in the directory where you run the emulator. This is the GBA boot ROM and is required for the emulator to function.
   > Note: The BIOS path is currently hardcoded to `gba_bios.bin` in the current working directory.
2. **A GBA ROM file**: Any `.gba` ROM file you want to run.

## Running the Emulator

### Using Just Commands

| Command | Description |
|---------|-------------|
| `just run <rom>` | Run ROM in debug mode |
| `just run-release <rom>` | Run ROM in release mode (better performance) |
| `just run-log <rom>` | Run in debug mode with logging to file |
| `just run-release-log <rom>` | Run in release mode with logging to file |

**Examples:**
```zsh
# Run a game in debug mode
just run ~/roms/pokemon_emerald.gba

# Run with better performance (recommended for playing)
just run-release ~/roms/pokemon_emerald.gba

# Run with logging enabled (logs saved to temp directory)
just run-log ~/roms/pokemon_emerald.gba
```

### Logging

When `--log-to-file` is passed, logs are written to `clementine.log` in your system's temp directory. The path is printed at startup.

## UI Tools

The emulator includes several debug tools accessible via the sidebar:

- **Gba Display** - Main game display (3x scale)
- **Cpu Handler** - Run/Pause/Step controls and breakpoints
- **Cpu Registers** - View ARM7TDMI register values
- **Disassembler** - Real-time disassembly of executed instructions
- **Save Game** - Save/Load state

## Development

| Command | Description |
|---------|-------------|
| `just build` | Build the entire project |
| `just test` | Run all tests across the workspace |
| `just lint` | Run clippy with strict configuration |
| `just fmt` | Format all code |
| `just check-fmt` | Check formatting without modifying |
| `just clean` | Clean build directory |
| `just doc` | Generate and open documentation |

### Documentation

The codebase is documented with Rust doc comments explaining how each component works. This is useful for understanding the GBA hardware and for contributors.

```zsh
# Generate and open documentation in your browser
just doc
```

## Architecture

The emulator uses a multi-threaded architecture:

- **UI Thread**: Runs the egui/eframe GUI at ~60fps
- **CPU Thread**: Runs the GBA emulation independently

Communication between threads uses lock-free SPSC (single-producer, single-consumer) channels for commands (UI -> CPU) and events (CPU -> UI).

## Tests ROM

All tests + implementation are based on [jsmolka/gba-tests.git](https://github.com/jsmolka/gba-tests.git) + documentation in Wiki and online resources.

- [x] Thumb rom
- [x] ARM rom
- [x] Memory rom
- [x] Bios rom
