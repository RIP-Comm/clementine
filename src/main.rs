use std::env;

use crate::{arm7tdmi::Arm7tdmi, cartridge::Cartridge, cpu::Cpu};

mod arm7tdmi;
mod cartridge;
mod condition;
mod cpsr;
mod cpu;
fn main() {
    println!("clementine v0.1.0");

    let args = env::args().skip(1).collect::<Vec<String>>();

    let file_path = match args.first() {
        Some(name) => {
            println!("loading {name}");
            name
        }
        None => {
            println!("no cartridge found :(");
            std::process::exit(1)
        }
    };

    let cartridge = match Cartridge::from_file(file_path) {
        Ok(val) => {
            println!("Title = {}", val.header().game_title());
            val
        }
        Err(e) => {
            println!("Err: {e}");
            std::process::exit(2);
        }
    };

    let mut cpu = Arm7tdmi::new(cartridge.rom());
    loop {
        cpu.step();
    }
}
