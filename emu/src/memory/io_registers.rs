use crate::bitwise::Bits;

pub enum IORegisterAccessControl {
    Read,
    Write,
    ReadWrite,
}

pub struct IORegister {
    value: u32,
    access: IORegisterAccessControl,
}

impl IORegister {
    pub fn read(&self) -> u32 {
        match self.access {
            IORegisterAccessControl::Read | IORegisterAccessControl::ReadWrite => self.value,
            _ => panic!("Requested IO Register cannot be read."),
        }
    }

    pub fn write(&mut self, value: u32) {
        match self.access {
            IORegisterAccessControl::Write | IORegisterAccessControl::ReadWrite => {
                self.value = value;
            }

            _ => panic!("Requested IO Register cannot be written."),
        }
    }

    pub fn set_byte(&mut self, byte: u8, value: u8) {
        match self.access {
            IORegisterAccessControl::Write | IORegisterAccessControl::ReadWrite => {
                self.value.set_byte(byte, value);
            }

            _ => panic!("Requested IO Register cannot be written."),
        }
    }

    pub const fn with_access_control(access: IORegisterAccessControl) -> Self {
        Self { value: 0, access }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use IORegisterAccessControl::*;
    #[test]
    #[should_panic]
    fn write_only_register() {
        let reg = IORegister::with_access_control(Write);

        reg.read();
    }

    #[test]
    #[should_panic]
    fn read_only_register_write() {
        let mut reg = IORegister::with_access_control(Read);

        reg.write(5);
    }

    #[test]
    #[should_panic]
    fn read_only_register_set_byte() {
        let mut reg = IORegister::with_access_control(Read);

        reg.set_byte(3, 4);
    }

    #[test]
    fn test_ioregister() {
        let mut reg = IORegister::with_access_control(ReadWrite);

        assert_eq!(reg.read(), 0);

        reg.write(5);

        assert_eq!(reg.read(), 5);

        reg.set_byte(1, 3);

        assert_eq!(reg.read(), (3 << 8) | 5);
    }
}
