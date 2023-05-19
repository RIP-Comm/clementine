use std::collections::HashMap;

use logger::log;

use crate::bitwise::Bits;

use super::{
    io_device::IoDevice,
    io_registers::{IORegister, IORegisterAccessControl},
};

pub struct SerialBus {
    sio_mode_select: IORegister,
    infrared_register: IORegister,
    sio_joy_bus_control: IORegister,
    sio_joy_bus_receive_data: IORegister,
    sio_joy_bus_transmit_data: IORegister,
    sio_joy_bus_receive_status: IORegister,
    unused_region: HashMap<usize, u8>,
}

impl Default for SerialBus {
    fn default() -> Self {
        Self::new()
    }
}

impl SerialBus {
    pub fn new() -> Self {
        Self {
            sio_mode_select: IORegister::with_access_control(IORegisterAccessControl::ReadWrite),
            infrared_register: IORegister::with_access_control(IORegisterAccessControl::ReadWrite),
            sio_joy_bus_control: IORegister::with_access_control(
                IORegisterAccessControl::ReadWrite,
            ),
            sio_joy_bus_receive_data: IORegister::with_access_control(
                IORegisterAccessControl::ReadWrite,
            ),
            sio_joy_bus_transmit_data: IORegister::with_access_control(
                IORegisterAccessControl::ReadWrite,
            ),
            sio_joy_bus_receive_status: IORegister::with_access_control(
                IORegisterAccessControl::ReadWrite,
            ),
            unused_region: HashMap::default(),
        }
    }
}

impl IoDevice for SerialBus {
    type Address = usize;
    type Value = u8;

    fn read_at(&self, address: usize) -> u8 {
        match address {
            0x04000134 => self.sio_mode_select.read().get_byte(0),
            0x04000135 => self.sio_mode_select.read().get_byte(1),
            0x04000136 => self.infrared_register.read().get_byte(0),
            0x04000137 => self.infrared_register.read().get_byte(1),
            0x04000140 => self.sio_joy_bus_control.read().get_byte(0),
            0x04000141 => self.sio_joy_bus_control.read().get_byte(1),
            0x04000150 => self.sio_joy_bus_receive_data.read().get_byte(0),
            0x04000151 => self.sio_joy_bus_receive_data.read().get_byte(1),
            0x04000152 => self.sio_joy_bus_receive_data.read().get_byte(2),
            0x04000153 => self.sio_joy_bus_receive_data.read().get_byte(3),
            0x04000154 => self.sio_joy_bus_transmit_data.read().get_byte(0),
            0x04000155 => self.sio_joy_bus_transmit_data.read().get_byte(1),
            0x04000156 => self.sio_joy_bus_transmit_data.read().get_byte(2),
            0x04000157 => self.sio_joy_bus_transmit_data.read().get_byte(3),
            0x04000158 => self.sio_joy_bus_receive_status.read().get_byte(0),
            0x04000159 => self.sio_joy_bus_receive_status.read().get_byte(1),
            0x04000138..=0x04000139 | 0x04000142..=0x0400014F | 0x0400015A..=0x0400015F => {
                log(format!("read on unused memory {address:x}"));
                self.unused_region.get(&address).map_or(0, |v| *v)
            }
            _ => panic!("Not implemented read memory address: {address:x}"),
        }
    }

    fn write_at(&mut self, address: Self::Address, value: Self::Value) {
        match address {
            0x04000134 => self.sio_mode_select.set_byte(0, value),
            0x04000135 => self.sio_mode_select.set_byte(1, value),
            0x04000136 => self.infrared_register.set_byte(0, value),
            0x04000137 => self.infrared_register.set_byte(1, value),
            0x04000140 => self.sio_joy_bus_control.set_byte(0, value),
            0x04000141 => self.sio_joy_bus_control.set_byte(1, value),
            0x04000150 => self.sio_joy_bus_receive_data.set_byte(0, value),
            0x04000151 => self.sio_joy_bus_receive_data.set_byte(1, value),
            0x04000152 => self.sio_joy_bus_receive_data.set_byte(2, value),
            0x04000153 => self.sio_joy_bus_receive_data.set_byte(3, value),
            0x04000154 => self.sio_joy_bus_transmit_data.set_byte(0, value),
            0x04000155 => self.sio_joy_bus_transmit_data.set_byte(1, value),
            0x04000156 => self.sio_joy_bus_transmit_data.set_byte(2, value),
            0x04000157 => self.sio_joy_bus_transmit_data.set_byte(3, value),
            0x04000158 => self.sio_joy_bus_receive_status.set_byte(0, value),
            0x04000159 => self.sio_joy_bus_receive_status.set_byte(1, value),
            0x04000138..=0x04000139 | 0x04000142..=0x0400014F | 0x0400015A..=0x0400015F => {
                log(format!("write on unused memory, {address:x}"));
                self.unused_region.insert(address, value);
            }
            _ => panic!("Not implemented write memory address: {address:x}"),
        };
    }
}
