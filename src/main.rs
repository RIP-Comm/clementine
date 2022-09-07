use std::{env, error, fs, io::Read};

use cartridge_header::CartridgeHeader;

use crate::{arm7tdmi::Arm7tdmi, cpu::Cpu};

mod alu_instruction;
mod arm7tdmi;
mod cartridge_header;
mod condition;
mod cpsr;
mod cpu;
mod instruction;

fn main() {
    println!("clementine v0.1.0");

    let cartridge_name = env::args().skip(1).collect::<Vec<String>>();

    let name = match cartridge_name.first() {
        Some(name) => {
            println!("loading {name}");
            name
        }
        None => {
            println!("no cartridge found :(");
            std::process::exit(1)
        }
    };

    let data = match read_file(name) {
        Ok(d) => d,
        Err(e) => {
            println!("{e}");
            std::process::exit(2);
        }
    };

    let cartridge_header = CartridgeHeader::new(&data);
    println!("{}", cartridge_header.game_title);

    let mut cpu = Arm7tdmi::new(data);
    loop {
        cpu.step();

        let mut buf = String::default();
        std::io::stdin().read_line(&mut buf).unwrap();
    }
}

fn read_file(filepath: &str) -> Result<Vec<u8>, Box<dyn error::Error>> {
    let mut f = fs::File::open(filepath)?;
    let mut buf = vec![];
    f.read_to_end(&mut buf)?;

    Ok(buf)
}
