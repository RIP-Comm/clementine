use crate::bitwise::Bits;
use crate::cpu::arm::alu_instruction::shift; // TODO: Move this to a more appropriate location, extract common code in "alu" module for example
use crate::cpu::arm7tdmi::Arm7tdmi;
use crate::cpu::condition::Condition;
use crate::cpu::flags::{LoadStoreKind, OperandKind, Operation, ReadWriteKind, ShiftKind};
use crate::cpu::registers::{REG_LR, REG_PROGRAM_COUNTER, REG_SP};
use crate::cpu::thumb;
use crate::cpu::thumb::alu_instructions::{ThumbHighRegisterOperation, ThumbModeAluInstruction};
use crate::cpu::thumb::mode::ThumbModeOpcode;
use std::ops::Mul;

pub const SIZE_OF_INSTRUCTION: u32 = 2;

impl Arm7tdmi {
    pub fn move_shifted_reg(&mut self, op: ShiftKind, offset5: u16, rs: u16, rd: u16) {
        let source = self.registers.register_at(rs.into());
        let r = shift(op, offset5.into(), source, self.cpsr.carry_flag());
        self.registers.set_register_at(rd.into(), r.result);

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
        let rs = self.registers.register_at(rs.into());
        let offset = match operation_kind {
            OperandKind::Immediate => rn_offset3 as u32,
            OperandKind::Register => self.registers.register_at(rn_offset3.into()),
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
        let dest = r_destination.into();
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
        let rs = self.registers.register_at(rs.into());
        match op {
            ThumbModeAluInstruction::And => {
                self.and(rd.into(), self.registers.register_at(rd.into()), rs, true)
            }
            ThumbModeAluInstruction::Eor => {
                self.eor(rd.into(), self.registers.register_at(rd.into()), rs, true)
            }
            ThumbModeAluInstruction::Lsl => {
                let destination = rd.into();
                // If the shift amount is 0 then the second operand is just Rd
                let second_operand = if rs == 0 {
                    self.registers.register_at(destination)
                } else {
                    // Otherwise we reuse the ARM logic
                    self.shift_operand(
                        crate::cpu::arm::alu_instruction::ArmModeAluInstruction::Mov,
                        true,
                        ShiftKind::Lsl,
                        rs,
                        self.registers.register_at(destination),
                    )
                };

                self.mov(destination, second_operand, true);
            }
            ThumbModeAluInstruction::Lsr => {
                let destination = rd.into();
                // If the shift amount is 0 then the second operand is just Rd
                let second_operand = if rs == 0 {
                    self.registers.register_at(destination)
                } else {
                    // Otherwise we reuse the ARM logic
                    self.shift_operand(
                        crate::cpu::arm::alu_instruction::ArmModeAluInstruction::Mov,
                        true,
                        ShiftKind::Lsr,
                        rs,
                        self.registers.register_at(destination),
                    )
                };

                self.mov(destination, second_operand, true);
            }
            ThumbModeAluInstruction::Asr => {
                let destination = rd.into();
                // If the shift amount in 0 then the second operand is just Rd
                let second_operand = if rs == 0 {
                    self.registers.register_at(destination)
                } else {
                    // Otherwise we reuse the ARM logic
                    self.shift_operand(
                        crate::cpu::arm::alu_instruction::ArmModeAluInstruction::Mov,
                        true,
                        ShiftKind::Asr,
                        rs,
                        self.registers.register_at(destination),
                    )
                };

                self.mov(destination, second_operand, true);
            }
            ThumbModeAluInstruction::Adc => {
                self.adc(rd.into(), self.registers.register_at(rd.into()), rs, true)
            }
            ThumbModeAluInstruction::Sbc => {
                self.sbc(rd.into(), self.registers.register_at(rd.into()), rs, true)
            }
            ThumbModeAluInstruction::Ror => {
                self.ror(rd.into(), rs);
            }
            ThumbModeAluInstruction::Tst => self.tst(self.registers.register_at(rd.into()), rs),
            ThumbModeAluInstruction::Neg => {
                self.neg(rd.into(), rs);
            }
            ThumbModeAluInstruction::Cmp => self.cmp(self.registers.register_at(rd.into()), rs),
            ThumbModeAluInstruction::Cmn => {
                let op1 = self.registers.register_at(rd.into());
                let op2 = rs;
                self.cmn(op1, op2);
            }
            ThumbModeAluInstruction::Orr => {
                self.orr(rd.into(), self.registers.register_at(rd.into()), rs, true)
            }
            ThumbModeAluInstruction::Mul => {
                self.thumb_mul(rd.into(), rs, self.registers.register_at(rd.into()))
            }
            ThumbModeAluInstruction::Bic => {
                let op1 = self.registers.register_at(rd.into());

                self.bic(rd.into(), op1, rs, true);
            }
            ThumbModeAluInstruction::Mvn => {
                self.mvn(rd.into(), rs, true);
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
        let value = self.read_word(address);
        let dest = r_destination.into();
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
        let ro = self.registers.register_at(offset_register.into());
        let rb = self.registers.register_at(base_register.into());
        let address: usize = rb.wrapping_add(ro).try_into().unwrap();
        let rd: usize = source_destination_register.into();

        match (load_store, byte_word) {
            (LoadStoreKind::Store, ReadWriteKind::Byte) => {
                let rd = (self.registers.register_at(rd) & 0xFF) as u8;
                self.bus.write_byte(address, rd);
            }
            (LoadStoreKind::Store, ReadWriteKind::Word) => {
                let rd = self.registers.register_at(rd);
                self.bus.write_word(address, rd);
            }
            (LoadStoreKind::Load, ReadWriteKind::Byte) => {
                let value = self.bus.read_byte(address);
                self.registers.set_register_at(rd, value as u32);
            }
            (LoadStoreKind::Load, ReadWriteKind::Word) => {
                let value = self.read_word(address);
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
            // Load halfword/sign-extended halfword
            (_, true) => {
                let value = self.read_half_word(address, sign_extend_flag);

                self.registers
                    .set_register_at(r_destination.try_into().unwrap(), value);
            }
            // Load sign-extended byte
            (true, false) => {
                let mut value = self.bus.read_byte(address) as u32;
                value = value.sign_extended(8);

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
        let rd = op_code.get_bits(0..=2).into();

        let base = self.registers.register_at(rb.into());
        let address = base.wrapping_add(offset).try_into().unwrap();

        match (load_store, byte_word) {
            (LoadStoreKind::Store, ReadWriteKind::Word) => {
                let v = self.registers.register_at(rd);
                self.bus.write_word(address, v)
            }
            (LoadStoreKind::Store, ReadWriteKind::Byte) => {
                let v = self.registers.register_at(rd);
                self.bus.write_byte(address, v as u8)
            }
            (LoadStoreKind::Load, ReadWriteKind::Word) => {
                let v = self.read_word(address);
                self.registers.set_register_at(rd, v);
            }
            (LoadStoreKind::Load, ReadWriteKind::Byte) => {
                let v = self.bus.read_byte(address);
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
        let rb = self.registers.register_at(base_register.into());
        let address: usize = rb.wrapping_add(offset as u32).try_into().unwrap();

        match load_store {
            LoadStoreKind::Load => {
                let value = self.read_half_word(address, false);

                self.registers
                    .set_register_at(source_destination_register as usize, value);
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

        let rd = r_destination.into();
        match load_store {
            LoadStoreKind::Load => {
                let value = self.read_word(address.try_into().unwrap());

                self.registers.set_register_at(rd, value);
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
            pc.set_bit_off(1);
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
                        let value = self.read_word(reg_sp.try_into().unwrap());

                        self.registers.set_register_at(r.into(), value);

                        reg_sp += 4;
                    }
                }

                if pc_lr {
                    let value = self.read_word(reg_sp.try_into().unwrap());
                    self.registers.set_program_counter(value);

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
        mut register_list: u16,
    ) {
        // According to the documentation register list should never be empty.
        // In reality the THUMB tests have a test for it. What happens in this case is that
        // register list will only be composed by R15 but the write back address will be the same as if
        // all 16 registers are present in the register list.

        let base_address = self.registers.register_at(base_register);
        let mut address = base_address;
        let register_count = if register_list == 0 {
            register_list = 0b1 << 15;
            16
        } else {
            register_list.count_ones()
        };

        match load_store {
            LoadStoreKind::Store => {
                for r in 0..=15 {
                    if register_list.is_bit_on(r) {
                        let value = self.registers.register_at(r as usize)
                            // If we store PC we should increase it by an instruction length.
                            + if r == 15 {
                                thumb::operations::SIZE_OF_INSTRUCTION
                            } else {
                                0
                            };

                        self.bus.write_word(address as usize, value);

                        address += 4;
                    }
                }
            }
            LoadStoreKind::Load => {
                for r in 0..=15 {
                    if register_list.get_bit(r) {
                        let value = self.read_word(address as usize);
                        self.registers.set_register_at(r as usize, value);

                        address += 4;
                    }
                }
            }
        }

        self.registers
            .set_register_at(base_register, base_address + register_count * 4);

        if load_store == LoadStoreKind::Load && register_list.is_bit_on(15) {
            self.flush_pipeline();
        }
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
            ThumbModeInstruction::MoveCompareAddSubtractImm {
                operation: Operation::Mov,
                destination_register: 0,
                offset: 0,
            },
            op_code.instruction,
        );

        cpu.execute_thumb(op_code);

        assert!(!cpu.cpsr.carry_flag());
        assert!(!cpu.cpsr.sign_flag());
        assert!(cpu.cpsr.zero_flag());
    }
}
