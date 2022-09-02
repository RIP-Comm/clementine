use std::{env, error, fs, io::Read};

use cartridge_header::CartridgeHeader;

use crate::cpu::CPU;

mod cartridge_header;
mod cpu;

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
    println!("{}", cartridge_header.title);

    let mut cpu = CPU::new(data);
    cpu.step();
}

fn read_file(filepath: &str) -> Result<Vec<u8>, Box<dyn error::Error>> {
    let mut f = fs::File::open(filepath)?;
    let mut buf = vec![];
    f.read_to_end(&mut buf)?;

    Ok(buf)
}
