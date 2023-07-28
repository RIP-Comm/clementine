use crate::bitwise::Bits;
use crate::cpu::arm::alu_instruction::shift; // TODO: Move this to a more appropriate location, extract common code in "alu" module for example
use crate::cpu::arm7tdmi::Arm7tdmi;
use crate::cpu::condition::Condition;
use crate::cpu::flags::{LoadStoreKind, OperandKind, Operation, ReadWriteKind, ShiftKind};
use crate::cpu::registers::{REG_LR, REG_PROGRAM_COUNTER, REG_SP};
use crate::cpu::thumb::alu_instructions::{ThumbHighRegisterOperation, ThumbModeAluInstruction};
use crate::cpu::thumb::mode::ThumbModeOpcode;
use std::ops::Mul;

pub const SIZE_OF_INSTRUCTION: u32 = 2;

impl Arm7tdmi {
    pub fn move_shifted_reg(&mut self, op: ShiftKind, offset5: u16, rs: u16, rd: u16) {
        let source = self.registers.register_at(rs.try_into().unwrap());
        let r = shift(op, offset5.into(), source, self.cpsr.carry_flag());
        self.registers
            .set_register_at(rd.try_into().unwrap(), r.result);

        self.cpsr.set_carry_flag(r.carry);
        self.cpsr.set_zero_flag(r.result == 0);
        self.cpsr.set_sign_flag(r.result.get_bit(31));
    }

    pub fn add_subtract(
        &mut self,
        operation_kind: OperandKind,
        op: bool,
        rn_offset3: u16,
        rs: u16,
        rd: u16,
    ) {
        let rs = self.registers.register_at(rs.try_into().unwrap());
        let offset = match operation_kind {
            OperandKind::Immediate => rn_offset3 as u32,
            OperandKind::Register => self.registers.register_at(rn_offset3.try_into().unwrap()),
        };

        match op {
            // TODO: Create a meaningful enum for this
            // Add
            false => {
                let add_result = Self::add_inner_op(rs, offset);
                self.registers
                    .set_register_at(rd as usize, add_result.result);
                self.cpsr.set_flags(add_result);
            }
            // Sub
            true => {
                let sub_result = Self::sub_inner_op(rs, offset);
                self.registers
                    .set_register_at(rd as usize, sub_result.result);
                self.cpsr.set_flags(sub_result);
            }
        };
    }

    pub fn move_compare_add_sub_imm(&mut self, op: Operation, r_destination: u16, offset: u32) {
        let dest = r_destination.try_into().unwrap();
        match op {
            Operation::Mov => {
                self.registers.set_register_at(dest, offset);

                // FIXME: Not sure if we should preserve the carry flag.
                // Documentation says that this is equal to an ARM MOVS Rd, #offset8
                // And in general MOV doesn't preserve the carry flag in ARM
                self.cpsr.set_carry_flag(false);
                self.cpsr.set_zero_flag(offset == 0);

                // FIXME: Since we're using an 8bits immediate it can't be negative since it's zero-extended
                // To check if it's zero-extended for real. Documentation says that this is equal to an
                // ARM MOVS Rd, #offset8 and ARM zero-extends in Mov immediate.
                self.cpsr.set_sign_flag(false);
            }
            Operation::Cmp => {
                let rd = self.registers.register_at(dest);
                let sub_result = Self::sub_inner_op(rd, offset);
                self.cpsr.set_flags(sub_result);
            }
            Operation::Add => {
                let rd_value = self.registers.register_at(dest);
                let add_result = Self::add_inner_op(rd_value, offset);
                self.registers.set_register_at(dest, add_result.result);
                self.cpsr.set_flags(add_result);
            }
            Operation::Sub => {
                let rd_value = self.registers.register_at(dest);
                let sub_result = Self::sub_inner_op(rd_value, offset);
                self.registers.set_register_at(dest, sub_result.result);
                self.cpsr.set_flags(sub_result);
            }
        };
    }

    pub fn alu_op(&mut self, op: ThumbModeAluInstruction, rs: u16, rd: u16) {
        let rs = self.registers.register_at(rs.try_into().unwrap());
        match op {
            ThumbModeAluInstruction::And => self.and(
                rd.into(),
                self.registers.register_at(rd.try_into().unwrap()),
                rs,
                true,
            ),
            ThumbModeAluInstruction::Eor => self.eor(
                rd.into(),
                self.registers.register_at(rd.try_into().unwrap()),
                rs,
                true,
            ),
            ThumbModeAluInstruction::Lsl => todo!(),
            ThumbModeAluInstruction::Lsr => todo!(),
            ThumbModeAluInstruction::Asr => todo!(),
            ThumbModeAluInstruction::Adc => todo!(),
            ThumbModeAluInstruction::Sbc => todo!(),
            ThumbModeAluInstruction::Ror => {
                self.ror(rd.try_into().unwrap(), rs);
            }
            ThumbModeAluInstruction::Tst => {
                self.tst(self.registers.register_at(rd.try_into().unwrap()), rs)
            }
            ThumbModeAluInstruction::Neg => {
                self.neg(rd.try_into().unwrap(), rs);
            }
            ThumbModeAluInstruction::Cmp => {
                self.cmp(self.registers.register_at(rd.try_into().unwrap()), rs)
            }
            ThumbModeAluInstruction::Cmn => todo!(),
            ThumbModeAluInstruction::Orr => self.orr(
                rd.into(),
                self.registers.register_at(rd.try_into().unwrap()),
                rs,
                true,
            ),
            ThumbModeAluInstruction::Mul => self.thumb_mul(
                rd.into(),
                rs,
                self.registers.register_at(rd.try_into().unwrap()),
            ),
            ThumbModeAluInstruction::Bic => todo!(),
            ThumbModeAluInstruction::Mvn => {
                self.mvn(rd.try_into().unwrap(), rs, true);
            }
        }
    }

    pub fn hi_reg_operation_branch_ex(
        &mut self,
        op: ThumbHighRegisterOperation,
        reg_source: u16,
        reg_destination: u16,
    ) {
        let d_value = self.registers.register_at(reg_destination as usize);
        let s_value = self.registers.register_at(reg_source as usize);

        match op {
            ThumbHighRegisterOperation::Add => {
                let r = d_value.wrapping_add(s_value);
                self.registers.set_register_at(reg_destination as usize, r);

                if reg_destination == REG_PROGRAM_COUNTER as u16 {
                    self.flush_pipeline();
                }
            }
            ThumbHighRegisterOperation::Cmp => {
                let sub_result = Self::sub_inner_op(d_value, s_value);

                self.cpsr.set_flags(sub_result);
            }
            ThumbHighRegisterOperation::Mov => {
                self.registers
                    .set_register_at(reg_destination as usize, s_value);

                if reg_destination == REG_PROGRAM_COUNTER as u16 {
                    self.flush_pipeline();
                }
            }
            ThumbHighRegisterOperation::BxOrBlx => {
                let new_state = s_value.get_bit(0);
                self.cpsr.set_cpu_state(new_state.into());
                let new_pc = s_value & !1;
                self.registers.set_program_counter(new_pc);

                self.flush_pipeline();
            }
        }
    }

    pub fn pc_relative_load(&mut self, r_destination: u16, immediate_value: u16) {
        let mut pc = self.registers.program_counter() as u32;
        // word alignment
        pc.set_bit_off(1);
        pc.set_bit_off(0);
        let address = pc.wrapping_add(immediate_value as u32) as usize;
        let value = self.bus.read_word(address);
        let dest = r_destination.try_into().unwrap();
        self.registers.set_register_at(dest, value);
    }

    pub fn load_store_register_offset(
        &mut self,
        load_store: LoadStoreKind,
        byte_word: ReadWriteKind,
        offset_register: u16,
        base_register: u16,
        source_destination_register: u16,
    ) {
        let ro = self
            .registers
            .register_at(offset_register.try_into().unwrap());
        let rb = self
            .registers
            .register_at(base_register.try_into().unwrap());
        let address: usize = rb.wrapping_add(ro).try_into().unwrap();
        let rd: usize = source_destination_register.try_into().unwrap();

        match (load_store, byte_word) {
            (LoadStoreKind::Store, ReadWriteKind::Byte) => {
                let rd = (self.registers.register_at(rd) & 0xFF) as u8;
                self.bus.write_at(address, rd);
            }
            (LoadStoreKind::Store, ReadWriteKind::Word) => {
                let rd = self.registers.register_at(rd);
                self.bus.write_word(address, rd);
            }
            (LoadStoreKind::Load, ReadWriteKind::Byte) => {
                let value = self.bus.read_at(address);
                self.registers.set_register_at(rd, value as u32);
            }
            (LoadStoreKind::Load, ReadWriteKind::Word) => {
                let value = self.bus.read_word(address);
                self.registers.set_register_at(rd, value);
            }
        };
    }

    pub fn load_store_sign_extend_byte_halfword(
        &mut self,
        h_flag: bool,
        sign_extend_flag: bool,
        r_offset: u32,
        r_base: u32,
        r_destination: u32,
    ) {
        let offset = self.registers.register_at(r_offset.try_into().unwrap());
        let base = self.registers.register_at(r_base.try_into().unwrap());
        let address: usize = base.wrapping_add(offset).try_into().unwrap();

        match (sign_extend_flag, h_flag) {
            // Store halfword
            (false, false) => {
                let value = self
                    .registers
                    .register_at(r_destination.try_into().unwrap());

                self.bus.write_half_word(address, value as u16);
            }
            // Load halfword
            (false, true) => {
                let value = self.bus.read_half_word(address);

                self.registers
                    .set_register_at(r_destination.try_into().unwrap(), value as u32);
            }
            // Load sign-extended byte
            (true, false) => {
                let mut value = self.bus.read_at(address) as u32;
                value = value.sign_extended(8);

                self.registers
                    .set_register_at(r_destination.try_into().unwrap(), value);
            }
            // Load sign-extended halfword
            (true, true) => {
                let mut value = self.bus.read_half_word(address) as u32;
                value = value.sign_extended(16);

                self.registers
                    .set_register_at(r_destination.try_into().unwrap(), value);
            }
        }
    }

    pub fn load_store_immediate_offset(&mut self, op_code: ThumbModeOpcode) {
        let byte_word: ReadWriteKind = op_code.get_bit(12).into();
        let load_store: LoadStoreKind = op_code.get_bit(11).into();
        let offset5 = op_code.get_bits(6..=10) as u32;
        let offset = offset5
            << match byte_word {
                ReadWriteKind::Word => 2,
                ReadWriteKind::Byte => 0,
            };

        let rb = op_code.get_bits(3..=5);
        let rd = op_code.get_bits(0..=2).try_into().unwrap();

        let base = self.registers.register_at(rb.try_into().unwrap());
        let address = base.wrapping_add(offset).try_into().unwrap();

        match (load_store, byte_word) {
            (LoadStoreKind::Store, ReadWriteKind::Word) => {
                let v = self.registers.register_at(rd);
                self.bus.write_word(address, v)
            }
            (LoadStoreKind::Store, ReadWriteKind::Byte) => {
                let v = self.registers.register_at(rd);
                self.bus.write_at(address, v as u8)
            }
            (LoadStoreKind::Load, ReadWriteKind::Word) => {
                let v = self.bus.read_word(address);
                self.registers.set_register_at(rd, v);
            }
            (LoadStoreKind::Load, ReadWriteKind::Byte) => {
                let v = self.bus.read_at(address);
                self.registers.set_register_at(rd, v as u32);
            }
        }
    }

    pub fn load_store_halfword(
        &mut self,
        load_store: LoadStoreKind,
        offset: u16,
        base_register: u16,
        source_destination_register: u16,
    ) {
        let rb = self
            .registers
            .register_at(base_register.try_into().unwrap());
        let address: usize = rb.wrapping_add(offset as u32).try_into().unwrap();

        match load_store {
            LoadStoreKind::Load => {
                self.registers.set_register_at(
                    source_destination_register as usize,
                    self.bus.read_half_word(address) as u32,
                );
            }
            LoadStoreKind::Store => {
                self.bus.write_half_word(
                    address,
                    self.registers
                        .register_at(source_destination_register as usize)
                        as u16,
                );
            }
        }
    }

    pub fn sp_relative_load_store(
        &mut self,
        load_store: LoadStoreKind,
        r_destination: u16,
        word8: u16,
    ) {
        let address = self.registers.register_at(REG_SP) + (word8 as u32);

        let rd = r_destination.try_into().unwrap();
        match load_store {
            LoadStoreKind::Load => {
                self.registers
                    .set_register_at(rd, self.bus.read_word(address.try_into().unwrap()));
            }
            LoadStoreKind::Store => {
                self.bus
                    .write_word(address.try_into().unwrap(), self.registers.register_at(rd));
            }
        }
    }

    pub fn load_address(&mut self, sp: bool, r_destination: usize, offset: u32) {
        let v = if sp {
            let stack_pointer = self.registers.register_at(REG_SP);
            stack_pointer.wrapping_add(offset)
        } else {
            let mut pc = self.registers.program_counter() as u32;
            pc.set_bit_off(0);
            pc.wrapping_add(offset)
        };

        self.registers.set_register_at(r_destination, v);
    }

    pub fn add_offset_sp(&mut self, s: bool, word7: u16) {
        let value = (word7 as i32).mul(if s { -1 } else { 1 });
        let old_sp = self.registers.register_at(REG_SP) as i32;
        let new_sp = old_sp.wrapping_add(value);

        self.registers.set_register_at(REG_SP, new_sp as u32);
    }

    pub fn push_pop_register(
        &mut self,
        load_store: LoadStoreKind,
        pc_lr: bool,
        register_list: u16,
    ) {
        let mut reg_sp = self.registers.register_at(REG_SP);

        match load_store {
            LoadStoreKind::Store => {
                if pc_lr {
                    reg_sp -= 4;
                    self.bus.write_word(
                        reg_sp.try_into().unwrap(),
                        self.registers.register_at(REG_LR),
                    );
                }

                for r in (0..=7).rev() {
                    if register_list.get_bit(r) {
                        reg_sp -= 4;
                        self.bus.write_word(
                            reg_sp.try_into().unwrap(),
                            self.registers.register_at(r.into()),
                        );
                    }
                }
            }
            LoadStoreKind::Load => {
                for r in 0..=7 {
                    if register_list.get_bit(r) {
                        self.registers.set_register_at(
                            r.try_into().unwrap(),
                            self.bus.read_word(reg_sp.try_into().unwrap()),
                        );

                        reg_sp += 4;
                    }
                }

                if pc_lr {
                    self.registers
                        .set_program_counter(self.bus.read_word(reg_sp.try_into().unwrap()));

                    reg_sp += 4;
                }
            }
        }

        self.registers.set_register_at(REG_SP, reg_sp);

        if load_store == LoadStoreKind::Load && pc_lr {
            self.flush_pipeline();
        }
    }

    pub fn multiple_load_store(
        &mut self,
        load_store: LoadStoreKind,
        base_register: usize,
        register_list: u16,
    ) {
        let mut address = self.registers.register_at(base_register);

        match load_store {
            LoadStoreKind::Store => {
                for r in 0..=7 {
                    if register_list.is_bit_on(r) {
                        let value = self.registers.register_at(r as usize);
                        self.bus.write_word(address as usize, value);

                        address += 4;
                    }
                }
            }
            LoadStoreKind::Load => {
                for r in 0..=7 {
                    if register_list.get_bit(r) {
                        let value = self.bus.read_word(address as usize);
                        self.registers.set_register_at(r as usize, value);

                        address += 4;
                    }
                }
            }
        }

        self.registers.set_register_at(base_register, address);
    }

    pub fn cond_branch(&mut self, condition: Condition, immediate_offset: i32) {
        if self.cpsr.can_execute(condition) {
            let pc = self.registers.program_counter() as i32;
            let new_pc = pc.wrapping_add(immediate_offset);
            self.registers.set_program_counter(new_pc as u32);

            self.flush_pipeline();
        }
    }

    pub fn uncond_branch(&mut self, offset: u32) {
        let offset = offset.sign_extended(12) as i32;
        let pc = self.registers.program_counter() as i32;
        let new_pc = pc + offset;
        self.registers.set_program_counter(new_pc as u32);

        self.flush_pipeline();
    }

    pub fn long_branch_link(&mut self, h: bool, offset: u32) {
        if h {
            let offset = offset << 1;

            let next_instruction = self.registers.program_counter() as u32 - SIZE_OF_INSTRUCTION;
            let lr = self.registers.register_at(REG_LR);

            self.registers.set_program_counter(lr.wrapping_add(offset));
            self.registers.set_register_at(REG_LR, next_instruction | 1);

            self.flush_pipeline();
        } else {
            let offset = offset << 12;
            let offset = offset.sign_extended(23);

            let pc = self.registers.program_counter() as u32;
            self.registers
                .set_register_at(REG_LR, pc.wrapping_add(offset));
        }
    }

    pub fn thumb_mul(&mut self, reg_result: usize, op1: u32, op2: u32) {
        let result = op1 as u64 * op2 as u64;

        self.registers.set_register_at(reg_result, result as u32);
        self.cpsr.set_zero_flag(result == 0);
        self.cpsr.set_sign_flag(result.get_bit(31));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpu::thumb::instruction::ThumbModeInstruction;
    use crate::cpu::thumb::mode::ThumbModeOpcode;
    use pretty_assertions::assert_eq;

    #[test]
    fn check_move_compare_add_sub_imm() {
        let mut cpu = Arm7tdmi::default();
        let op_code = 0b0010_0000_0000_0000_u16;
        let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);
        assert_eq!(
            op_code.instruction,
            ThumbModeInstruction::MoveCompareAddSubtractImm {
                operation: Operation::Mov,
                destination_register: 0,
                offset: 0,
            }
        );

        cpu.execute_thumb(op_code);

        assert!(!cpu.cpsr.carry_flag());
        assert!(!cpu.cpsr.sign_flag());
        assert!(cpu.cpsr.zero_flag());
    }
}
