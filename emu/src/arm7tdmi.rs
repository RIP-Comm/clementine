use std::convert::TryInto;

use crate::alu_instruction::AluInstructionKind;
use crate::alu_instruction::ArmModeAluInstruction;
use crate::alu_instruction::Kind;
use crate::bitwise::Bits;
use crate::instruction::ArmModeInstruction;
use crate::memory::internal_memory::InternalMemory;
use crate::memory::io_device::IoDevice;
use crate::opcode::ArmModeOpcode;
use crate::{cpsr::Cpsr, cpu::Cpu};

/// Contains the 16 registers for the CPU, latest (R15) is special because
/// is the program counter.
#[derive(Default)]
pub struct Registers([u32; 16]);

impl Registers {
    pub fn program_counter(&self) -> usize {
        self.0[15].try_into().unwrap()
    }

    #[cfg(test)] // TODO: remove cfg when this API will be used at least one in prod code.
    pub fn set_program_counter(&mut self, new_value: u32) {
        self.0[15] = new_value
    }

    pub fn advance_program_counter(&mut self, bytes: u32) {
        self.0[15] = self.0[15].wrapping_add(bytes);
    }

    #[allow(clippy::only_used_in_recursion)] // FIXME: Possible bug of clippy?
    pub fn set_register_at(&mut self, reg: usize, new_value: u32) {
        self.0[reg] = new_value;
    }

    pub const fn register_at(&self, reg: usize) -> u32 {
        self.0[reg]
    }

    pub fn to_vec(&self) -> Vec<u32> {
        self.0.as_slice().to_vec()
    }
}

pub struct Arm7tdmi {
    rom: Vec<u8>,

    pub(crate) registers: Registers,
    cpsr: Cpsr,

    pub(crate) memory: InternalMemory,
}

const OPCODE_ARM_SIZE: usize = 4;

impl Cpu for Arm7tdmi {
    type OpCodeType = ArmModeOpcode;

    fn fetch(&self) -> u32 {
        let instruction_index = self.registers.program_counter();
        let end_instruction = instruction_index + OPCODE_ARM_SIZE;
        let data_instruction: [u8; 4] = self.rom[instruction_index..end_instruction]
            .try_into()
            .expect("`istruction` conversion into [u8; 4]");

        u32::from_le_bytes(data_instruction)
    }

    fn decode(&self, op_code: u32) -> Self::OpCodeType {
        let op_code = ArmModeOpcode::try_from(op_code).unwrap();
        println!("{}", op_code);
        if op_code.instruction == ArmModeInstruction::Unknown {
            todo!("implement this instruction")
        }

        op_code
    }

    fn execute(&mut self, op_code: Self::OpCodeType) {
        use ArmModeInstruction::*;
        // Instruction functions should return whether PC has to be advanced
        // after instruction executed.
        let should_advance_pc = match op_code.instruction {
            Branch => self.branch(op_code),
            BranchLink => self.branch_link(op_code),
            DataProcessing1 | DataProcessing2 | DataProcessing3 => self.data_processing(op_code),
            TransImm9 => self.single_data_transfer(op_code),
            BlockDataTransfer => self.block_data_transfer(op_code),
            Unknown => {
                todo!("implement this instruction")
            }
        };

        if should_advance_pc {
            self.registers.advance_program_counter(4); // FIXME: don't sure of this
        }
    }

    fn step(&mut self) {
        let op_code = self.fetch();

        let op_code = self.decode(op_code);
        if self.cpsr.can_execute(op_code.condition) {
            self.execute(op_code)
        }
    }

    fn registers(&self) -> Vec<u32> {
        self.registers.to_vec()
    }
}

impl Arm7tdmi {
    pub fn new(rom: Vec<u8>) -> Self {
        Self {
            rom,
            registers: Registers::default(),
            cpsr: Cpsr::default(),
            memory: InternalMemory::new(),
        }
    }

    fn branch(&mut self, op_code: ArmModeOpcode) -> bool {
        let offset = op_code.get_bits(0..=23);

        self.registers.advance_program_counter(8 + offset * 4);

        // Never advance PC after B
        false
    }

    fn branch_link(&mut self, op_code: ArmModeOpcode) -> bool {
        let pc: u32 = self.registers.program_counter().try_into().unwrap();
        self.registers.set_register_at(14, pc.wrapping_add(4)); // R14 = LR

        let offset = op_code.get_bits(0..=23);

        self.registers.advance_program_counter(8 + offset * 4);

        // Never advance PC after BL
        false
    }

    fn get_operand(&mut self, alu_opcode: u32, s: bool, i: bool, op2: u32) -> u32 {
        match i {
            // we get the operand from a register and then we shift it
            false => {
                // bits [0-3] 2nd Operand Register (R0..R15) (including PC=R15)
                let rm = op2.get_bits(0..=3);
                // bit [4] - is Shift by Register Flag (0=Immediate, 1=Register)
                let r = op2.get_bit(4);
                let offset = match rm {
                    // if Rm is R15(PC) we need to offset its value because of
                    // instruction pipelining
                    0xF => self.get_pc_offset_alu(i, r),
                    _ => 0,
                };
                let rm = self.registers.register_at(rm.try_into().unwrap()) + offset;
                // bits [6-5] - Shift Type (0=LSL, 1=LSR, 2=ASR, 3=ROR)
                let shift_type = op2.get_bits(5..=6);

                let shift_amount = match r {
                    // the shift amount is in the instruction
                    false => {
                        // bits [7-11] - Shift amount
                        op2.get_bits(7..=11)
                    }
                    // the shift amount is read from Rs
                    true => {
                        // bits [11-8] - Shift register (R0-R14) - only lower 8bit 0-255 used
                        let rs = op2.get_bits(8..=11);

                        let rs = self.registers.register_at(rs.try_into().unwrap()) & 0xFF;

                        // If shift is taken from register and the value is 0 Rm is directly used as operand
                        if rs == 0 {
                            return rm;
                        }

                        rs
                    }
                };

                self.shift_operand(alu_opcode, s, shift_type, shift_amount, rm)
            }
            true => {
                // bits [7-0] are the immediate value
                let imm = op2.get_bits(0..=7);
                // bit [11-8] are the rotate amount
                let rotate_amount = op2.get_bits(8..=11);

                imm.rotate_right(rotate_amount * 2)
            }
        }
    }

    /// Returns the offset that has to be applied to the value read by `PC`
    /// in the case of data processing (ALU) instruction.
    ///
    /// This is needed because when the instruction at address `X` is executing,
    /// PC points to `X+8` because of pipelining. If we need to read the shift
    /// amount from register (`i` is `False` and `r` is `True`) the instruction
    /// takes an additional cycle, thus `PC` points to `X+12`.
    ///
    /// # Arguments
    ///
    /// * `i` - A boolean value representing whether the 2nd operand is immediate or not
    /// * `r` - A boolean value representing whether the shift amount is to be taken from register or not
    const fn get_pc_offset_alu(&self, i: bool, r: bool) -> u32 {
        if !i && r {
            12
        } else {
            8
        }
    }

    fn data_processing(&mut self, op_code: ArmModeOpcode) -> bool {
        // bit [25] is I = Immediate Flag
        let i: bool = op_code.get_bit(25);
        // bits [24-21]
        let alu_op_code = op_code.get_bits(21..=24);
        // bit [20] is sets condition codes
        let s = op_code.get_bit(20);
        // bits [15-12] are the Rd
        let rd = op_code.get_bits(12..=15);
        // bits [19-16] are the Rn
        let rn = op_code.get_bits(16..=19);
        let offset = match rn {
            // if Rn is R15(PC) we need to offset its value because of
            // instruction pipelining
            0xF => self.get_pc_offset_alu(i, op_code.get_bit(4)),
            _ => 0,
        };

        let rn = self.registers.register_at(rn.try_into().unwrap()) + offset;

        let op2 = self.get_operand(alu_op_code, s, i, op_code.get_bits(0..=11));

        // S = 1 and Rd = 0xF should not be allowed in User Mode.
        // TODO: When in other modes it should load SPSR_<current_mode> into CPSR
        if s && rd == 0xF {
            unimplemented!("Implement cases when S=1 and Rd=0xF");
        }

        match ArmModeAluInstruction::from(alu_op_code) {
            ArmModeAluInstruction::And => self.and(rd.try_into().unwrap(), rn, op2, s),
            ArmModeAluInstruction::Eor => self.eor(rd.try_into().unwrap(), rn, op2, s),
            ArmModeAluInstruction::Bic => self.bic(rd.try_into().unwrap(), rn, op2, s),
            ArmModeAluInstruction::Mvn => self.mvn(rd.try_into().unwrap(), op2, s),
            ArmModeAluInstruction::Tst => {
                if s {
                    self.tst(rn, op2)
                }
            }
            ArmModeAluInstruction::Mov => self.mov(rd.try_into().unwrap(), op2, s),
            ArmModeAluInstruction::Teq => {
                if s {
                    self.teq(rn, op2)
                }
            }
            ArmModeAluInstruction::Cmp => {
                if s {
                    self.cmp(rn, op2)
                }
            }
            ArmModeAluInstruction::Cmn => {
                if s {
                    self.cmn(rn, op2)
                }
            }
            ArmModeAluInstruction::Add => self.add(rd.try_into().unwrap(), rn, op2, s),
            ArmModeAluInstruction::Orr => self.orr(rd.try_into().unwrap(), rn, op2, s),
            ArmModeAluInstruction::Sub => self.sub(rd.try_into().unwrap(), rn, op2, s),
            ArmModeAluInstruction::Rsb => self.rsb(rd.try_into().unwrap(), rn, op2, s),
            _ => todo!("implement alu operation: {}", alu_op_code),
        };

        // If is a "test" ALU instruction we ever advance PC.
        match ArmModeAluInstruction::from(alu_op_code) {
            ArmModeAluInstruction::Teq
            | ArmModeAluInstruction::Cmp
            | ArmModeAluInstruction::Cmn
            | ArmModeAluInstruction::Tst => true,
            _ => rd != 0xF,
        }
    }

    fn block_data_transfer(&mut self, op_code: ArmModeOpcode) -> bool {
        let pre_post = op_code.get_bit(24);
        let up_down = op_code.get_bit(23);
        let s = op_code.get_bit(22);
        if s {
            todo!()
        }
        let write_back = op_code.get_bit(21);
        let load_store = op_code.get_bit(20);
        let rn = op_code.get_bits(16..=19);
        let reg_list = op_code.get_bits(0..=15);

        let memory_base = self.registers.register_at(rn.try_into().unwrap());
        let alignment = 4; // Since are word, the alignment is 4.
        let mut address = memory_base;
        let change_address = |address: u32| {
            if up_down {
                address.wrapping_add(alignment)
            } else {
                address.wrapping_sub(alignment)
            }
        };

        if load_store {
            for reg_destination in 0..16 {
                if reg_list.is_bit_on(reg_destination) {
                    if pre_post {
                        address = change_address(address);
                    }

                    let part_0: u32 = self.memory.read_at(address).try_into().unwrap();
                    let part_1: u32 = self.memory.read_at(address + 1).try_into().unwrap();
                    let part_2: u32 = self.memory.read_at(address + 2).try_into().unwrap();
                    let part_3: u32 = self.memory.read_at(address + 3).try_into().unwrap();
                    let value: u32 = part_3 << 24_u32 | part_2 << 16_u32 | part_1 << 8_u32 | part_0;
                    self.registers
                        .set_register_at(reg_destination.try_into().unwrap(), value);

                    if !pre_post {
                        address = change_address(address);
                    }
                }
            }
        } else {
            todo!()
        }

        if write_back {
            self.registers
                .set_register_at(rn.try_into().unwrap(), address);
        };

        // If LDM and R15 is in register list we don't advance PC
        !(load_store && reg_list.is_bit_on(15))
    }

    fn mvn(&mut self, rd: usize, op2: u32, s: bool) {
        let result = !op2;

        self.registers.set_register_at(rd, result);

        if s {
            self.cpsr.set_sign_flag(result.get_bit(31));
            self.cpsr.set_zero_flag(result == 0);
        }
    }

    fn bic(&mut self, rd: usize, rn: u32, op2: u32, s: bool) {
        let result = rn & !op2;

        self.registers.set_register_at(rd, result);

        if s {
            self.cpsr.set_sign_flag(result.get_bit(31));
            self.cpsr.set_zero_flag(result == 0);
        }
    }

    fn eor(&mut self, rd: usize, rn: u32, op2: u32, s: bool) {
        let result = rn ^ op2;

        self.registers.set_register_at(rd, result);

        if s {
            self.cpsr.set_zero_flag(result == 0);
            self.cpsr.set_sign_flag(result.get_bit(31));
        }
    }

    fn and(&mut self, rd: usize, rn: u32, op2: u32, s: bool) {
        let result = rn & op2;

        self.registers.set_register_at(rd, result);

        if s {
            self.cpsr.set_zero_flag(result == 0);
            self.cpsr.set_sign_flag(result.get_bit(31));
        }
    }

    fn add_inner_op(first_op: u32, second_op: u32) -> ArithmeticOpResult {
        // we do the sum in 64bits so that the 32nd bit is the carry
        let result_and_carry: u64 = (first_op as u64).wrapping_add(second_op as u64);
        let result: u32 = result_and_carry as u32;

        let sign_op1: bool = first_op.get_bit(31);
        let sign_op2: bool = second_op.get_bit(31);
        let sign_r: bool = result.get_bit(31);

        let carry: bool = (result_and_carry & 0x100000000) >> 32 == 1;

        // overflow only occurs when operands have the same sign and result has the opposite one
        let same_sign: bool = sign_op1 == sign_op2;

        ArithmeticOpResult {
            result,
            carry,
            overflow: same_sign && (sign_op1 != sign_r),
            sign: result.get_bit(31),
            zero: result == 0,
        }
    }

    fn add(&mut self, rd: usize, rn: u32, op2: u32, s: bool) {
        let add_result = Self::add_inner_op(rn, op2);

        self.registers.set_register_at(rd, add_result.result);

        if s {
            self.cpsr.set_flags(add_result);
        }
    }

    fn orr(&mut self, rd: usize, rn: u32, op2: u32, s: bool) {
        let result: u32 = rn | op2;

        self.registers.set_register_at(rd, result);

        if s {
            self.cpsr.set_zero_flag(result == 0);
            self.cpsr.set_sign_flag(result.is_bit_on(31));
        }
    }

    fn mov(&mut self, rd: usize, op2: u32, s: bool) {
        self.registers.set_register_at(rd, op2);

        if s {
            self.cpsr.set_zero_flag(op2 == 0);
            self.cpsr.set_sign_flag(op2.get_bit(31));
        }
    }

    fn tst(&mut self, rn: u32, op2: u32) {
        let value = rn & op2;

        self.cpsr.set_sign_flag(value.is_bit_on(31));
        self.cpsr.set_zero_flag(value == 0);
    }

    fn teq(&mut self, rn: u32, op2: u32) {
        let value = rn ^ op2;
        self.cpsr.set_sign_flag(value.is_bit_on(31));
        self.cpsr.set_zero_flag(value == 0);
    }

    fn sub_inner_op(first_op: u32, second_op: u32) -> ArithmeticOpResult {
        let result = first_op.wrapping_sub(second_op);

        let sign_op1: bool = first_op.get_bit(31);
        let sign_op2: bool = second_op.get_bit(31);
        let sign_r: bool = result.get_bit(31);

        let different_sign = sign_op1 != sign_op2;

        ArithmeticOpResult {
            result,
            carry: first_op < second_op,
            overflow: different_sign && sign_op2 == sign_r,
            sign: result.get_bit(31),
            zero: result == 0,
        }
    }

    fn cmn(&mut self, rn: u32, op2: u32) {
        let add_result = Self::add_inner_op(rn, op2);

        self.cpsr.set_flags(add_result);
    }

    fn cmp(&mut self, rn: u32, op2: u32) {
        let sub_result = Self::sub_inner_op(rn, op2);

        self.cpsr.set_flags(sub_result);
    }

    fn sub(&mut self, rd: usize, rn: u32, op2: u32, s: bool) {
        let sub_result = Self::sub_inner_op(rn, op2);

        self.registers.set_register_at(rd, sub_result.result);

        if s {
            self.cpsr.set_flags(sub_result);
        }
    }

    fn rsb(&mut self, rd: usize, rn: u32, op2: u32, s: bool) {
        self.sub(rd, op2, rn, s);
    }

    fn shift_operand(
        &mut self,
        alu_opcode: u32,
        s: bool,
        shift_type: u32,
        shift_amount: u32,
        rm: u32,
    ) -> u32 {
        // Shift Type (0=LSL, 1=LSR, 2=ASR, 3=ROR)
        let mut carry: bool = self.cpsr.carry_flag();

        let result = match shift_type {
            // LSL
            0 => {
                match shift_amount {
                    // LSL#0: No shift performed, ie. directly value=Rm, the C flag is NOT affected.
                    0 => rm,
                    // LSL#1..32: Normal left logical shift
                    1..=32 => {
                        carry = rm.get_bit((32 - shift_amount).try_into().unwrap());

                        rm << shift_amount
                    }
                    // LSL#33...: Result is 0 and carry is 0
                    _ => {
                        carry = false;

                        0
                    }
                }
            }
            // LSR
            1 => {
                match shift_amount {
                    // LSR#0 is used to encode LSR#32, it has 0 result and carry equal to bit 31 of Rm
                    0 => {
                        carry = rm.get_bit(31);

                        0
                    }
                    // LSR#1..32: Normal right logical shift
                    1..=32 => {
                        carry = rm.get_bit((shift_amount - 1).try_into().unwrap());

                        rm >> shift_amount
                    }
                    // LSR#33...: result is 0 and carry is 0
                    _ => {
                        carry = false;

                        0
                    }
                }
            }
            //ASR
            2 => {
                match shift_amount {
                    // ASR#1..31: normal arithmetic right shift
                    1..=31 => {
                        carry = rm.get_bit((shift_amount - 1).try_into().unwrap());

                        ((rm as i32) >> shift_amount) as u32
                    }
                    // ASR#0 (which is used to encode ASR#32), ASR#32 and above all have the same result
                    _ => {
                        carry = rm.get_bit(31);
                        // arithmetically shifting by 31 is the same as shifting by 32, but with 32 rust complains
                        ((rm as i32) >> 31) as u32
                    }
                }
            }
            // ROR
            3 => {
                // from documentation: ROR by n where n is greater than 32 will give the same
                // result and carry out as ROR by n-32; therefore repeatedly y subtract 32 from n until the amount is
                // in the range 1 to 32
                let mut new_shift_amount = shift_amount;

                if shift_amount > 32 {
                    new_shift_amount %= 32;

                    // if modulo operation yields 0 it means that shift_amount was a multiple of 32
                    // so we should do ROR#32
                    if new_shift_amount == 0 {
                        new_shift_amount = 32;
                    }
                }

                match new_shift_amount {
                    // ROR#0 is used to encode RRX (appending C to the left and shift right by 1)
                    0 => {
                        let old_carry = self.cpsr.carry_flag() as u32;

                        carry = rm.get_bit(0);

                        (rm >> 1) | (old_carry << 31)
                    }
                    // ROR#1..31: normal rotate right
                    1..=31 => {
                        carry = rm.get_bit((shift_amount - 1).try_into().unwrap());

                        rm.rotate_right(shift_amount)
                    }
                    // ROR#32 doesn't change rm but sets carry to bit 31 of rm
                    32 => {
                        carry = rm.get_bit(31);

                        rm
                    }
                    // ROR#i with i > 32 is the same of ROR#n where n = i % 32
                    _ => unreachable!(),
                }
            }
            _ => unreachable!(),
        };

        // If the instruction is a logical ALU instruction and S is set we set the carry flag
        if ArmModeAluInstruction::from(alu_opcode).kind() == AluInstructionKind::Logical && s {
            self.cpsr.set_carry_flag(carry);
        }

        result
    }
}

pub struct ArithmeticOpResult {
    result: u32,
    pub carry: bool,
    pub overflow: bool,
    pub sign: bool,
    pub zero: bool,
}
#[cfg(test)]
mod tests {
    use crate::condition::Condition;
    use crate::instruction::ArmModeInstruction;
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn decode_branch() {
        let output: ArmModeOpcode = 0b1110_1010_0000_0000_0000_0000_0111_1111
            .try_into()
            .unwrap();
        assert_eq!(output.instruction, ArmModeInstruction::Branch);
    }

    #[test]
    fn decode_branch_link() {
        let output: ArmModeOpcode = 0b1110_1011_0000_0000_0000_0000_0111_1111
            .try_into()
            .unwrap();
        assert_eq!(output.instruction, ArmModeInstruction::BranchLink);
    }

    #[test]
    fn test_registers_14_after_branch_link() {
        let mut cpu: Arm7tdmi = Arm7tdmi::new(vec![]);
        cpu.registers = Registers([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
        let pc: u32 = cpu.registers.program_counter().try_into().unwrap();
        cpu.branch_link(0b0_u32.try_into().unwrap());
        assert_eq!(cpu.registers.register_at(14), pc.wrapping_add(4));
    }

    #[test]
    fn check_mov_rx_immediate() {
        // MOV R0, 0
        let mut op_code: u32 = 0b1110_0011_1010_0000_0000_0000_0000_0000;

        // bits [11-8] are ROR-Shift applied to nn
        let is = op_code & 0b0000_0000_0000_0000_0000_1111_0000_0000;

        // MOV Rx,x
        let mut cpu = Arm7tdmi::new(vec![]);
        for rx in 0..=0xF {
            let register_for_op = rx << 12;
            let immediate_value = rx;

            // Rd parameter
            op_code = (op_code & 0b1111_1111_1111_1111_0000_1111_1111_1111) + register_for_op;
            // Immediate parameter
            op_code = (op_code & 0b1111_1111_1111_1111_1111_1111_0000_0000) + immediate_value;

            let op_code = cpu.decode(op_code);
            assert_eq!(op_code.condition, Condition::AL);
            assert_eq!(op_code.instruction, ArmModeInstruction::DataProcessing3);

            cpu.execute(op_code);
            let rotated = rx.rotate_right(is * 2);
            assert_eq!(cpu.registers.register_at(rx.try_into().unwrap()), rotated);
        }
    }

    #[test]
    fn check_mov_cpsr() {
        // Checks for Z flag
        let op_code = 0b1110_00_0_1101_1_0000_0001_00000_00_0_0010;
        let mut cpu = Arm7tdmi::new(vec![]);
        let op_code = cpu.decode(op_code);

        cpu.execute(op_code);

        assert!(cpu.cpsr.zero_flag());

        // Checks for Z flag
        let op_code = 0b1110_00_0_1101_1_0000_0001_00000_00_0_0010;
        let mut cpu = Arm7tdmi::new(vec![]);
        let op_code = cpu.decode(op_code);

        cpu.registers.set_register_at(2, -5_i32 as u32);
        cpu.execute(op_code);

        assert!(cpu.cpsr.sign_flag());
    }
    #[test]
    fn check_teq() {
        // This case cover S=0 then it will skip the execution of TEQ.
        {
            let op_code = 0b1110_0001_0010_1001_0011_0000_0000_0000;
            let mut cpu = Arm7tdmi::new(vec![]);
            let op_code = cpu.decode(op_code);
            assert_eq!(op_code.instruction, ArmModeInstruction::DataProcessing1);
            let rn = 9_usize;
            cpu.registers.set_register_at(rn, 100);
            cpu.cpsr.set_sign_flag(true); // set for later verify.
            cpu.execute(op_code);
            assert!(cpu.cpsr.sign_flag());
            assert!(!cpu.cpsr.zero_flag());
        }
    }

    #[test]
    fn check_cmp_s1() {
        let op_code: u32 = 0b1110_0001_0011_1010_0011_0000_0000_0000;
        let mut cpu = Arm7tdmi::new(vec![]);
        let op_code = cpu.decode(op_code);
        assert_eq!(op_code.instruction, ArmModeInstruction::DataProcessing1);
        let rn = 9_usize;
        cpu.registers.set_register_at(rn, 1);
        cpu.execute(op_code);
        assert!(!cpu.cpsr.sign_flag());
        assert!(cpu.cpsr.zero_flag());
    }

    #[test]
    fn check_cmp_s0() {
        let op_code: u32 = 0b1110_0001_0010_1010_0011_0000_0000_0000;
        let mut cpu = Arm7tdmi::new(vec![]);
        let op_code = cpu.decode(op_code);
        assert_eq!(op_code.instruction, ArmModeInstruction::DataProcessing1);
        let rn = 9_usize;
        cpu.registers.set_register_at(rn, 1);
        cpu.cpsr.set_sign_flag(true); // set for later verify.
        cpu.execute(op_code);
        assert!(cpu.cpsr.sign_flag());
        assert!(!cpu.cpsr.zero_flag());
    }

    #[test]
    fn check_add() {
        let op_code = 0b1110_0010_1000_1111_0000_0000_0010_0000;
        let mut cpu = Arm7tdmi::new(vec![]);
        let op_code = cpu.decode(op_code);
        assert_eq!(op_code.instruction, ArmModeInstruction::DataProcessing3);
        cpu.registers.set_register_at(15, 15);
        cpu.execute(op_code);
        assert_eq!(cpu.registers.register_at(0), 15 + 8 + 32);
    }

    #[test]
    fn check_add_pc_operand_shift_register() {
        // Case when R15 is used as operand and shift amount is taken from register:
        // R2 = R1 + (R15 << R3)
        let op_code = 0b1110_0000_1000_0001_0010_0011_0001_1111;
        let mut cpu = Arm7tdmi::new(vec![]);
        let op_code = cpu.decode(op_code);
        assert_eq!(op_code.instruction, ArmModeInstruction::DataProcessing2);

        cpu.registers.set_register_at(2, 5);
        cpu.registers.set_register_at(1, 10);
        cpu.registers.set_register_at(15, 500);
        cpu.registers.set_register_at(3, 0);

        cpu.execute(op_code);

        assert_eq!(cpu.registers.register_at(2), 500 + 12 + 10);
    }

    #[test]
    fn check_add_carry_bit() {
        let op_code: u32 = 0b1110_0000_1001_1111_0000_0000_0000_1110;
        let mut cpu = Arm7tdmi::new(vec![]);
        let op_code = cpu.decode(op_code);

        assert_eq!(op_code.instruction, ArmModeInstruction::DataProcessing1);

        cpu.registers.set_register_at(15, 1 << 31);
        cpu.registers.set_register_at(14, 1 << 31);
        cpu.execute(op_code);
        assert_eq!(cpu.registers.register_at(0), 8);
        assert!(cpu.cpsr.carry_flag());
        assert!(cpu.cpsr.overflow_flag());
        assert!(!cpu.cpsr.sign_flag());
        assert!(!cpu.cpsr.zero_flag());
    }

    #[test]
    #[should_panic]
    fn check_unknown_instruction() {
        let op_code = 0b1110_1111_1111_1111_1111_1111_1111_1111;
        let mut cpu = Arm7tdmi::new(vec![]);

        let op_code = cpu.decode(op_code);
        assert_eq!(op_code.instruction, ArmModeInstruction::Unknown);
        assert_eq!(op_code.condition, Condition::AL);

        cpu.execute(op_code);
    }

    #[test]
    fn check_block_trans() {
        let op_code = 0b1110_1000_1011_1101_1000_0000_0000_0111;
        let mut cpu = Arm7tdmi::new(vec![]);
        let op_code = cpu.decode(op_code);

        cpu.registers.set_register_at(13, 0x03000000); // fake mem address simulate dirty reg.
        cpu.memory.write_at(0x03000000, 10);
        cpu.memory.write_at(0x03000000 + 4, 11);
        cpu.memory.write_at(0x03000000 + 8, 12);
        cpu.memory.write_at(0x03000000 + 12, 13);

        assert_eq!(op_code.instruction, ArmModeInstruction::BlockDataTransfer);
        assert_eq!(op_code.condition, Condition::AL);
        cpu.execute(op_code);

        assert_eq!(cpu.registers.program_counter(), 13);
        assert_eq!(cpu.registers.register_at(0), 10);
        assert_eq!(cpu.registers.register_at(1), 11);
        assert_eq!(cpu.registers.register_at(2), 12);

        assert_eq!(cpu.registers.register_at(13), 0x3000010);
    }

    #[test]
    fn shift_from_register_is_0() {
        let op_code = 0b1110_0000_1000_0000_0001_0011_0111_0010;
        let mut cpu = Arm7tdmi::new(vec![]);

        let op_code = cpu.decode(op_code);

        cpu.registers.set_register_at(0, 5);
        cpu.registers.set_register_at(2, 11);
        cpu.registers.set_register_at(3, 8 << 8);

        cpu.execute(op_code);

        assert_eq!(cpu.registers.register_at(1), 16);
    }

    #[test]
    fn check_and() {
        let op_code = 0b1110_00_1_0000_0_0000_0001_0000_10101010;
        let mut cpu = Arm7tdmi::new(vec![]);
        let op_code = cpu.decode(op_code);

        // All 1 except msb
        cpu.registers.set_register_at(0, 2_u32.pow(31) - 1);

        cpu.execute(op_code);

        assert_eq!(cpu.registers.register_at(1), 0b10101010);
    }

    #[test]
    fn check_eor() {
        let op_code = 0b1110_00_1_0001_0_0000_0001_0000_10101010;
        let mut cpu = Arm7tdmi::new(vec![]);
        let op_code = cpu.decode(op_code);

        cpu.registers.set_register_at(0, 0b11111111);

        cpu.execute(op_code);

        assert_eq!(cpu.registers.register_at(1), 0b01010101);
    }

    #[test]
    fn check_tst() {
        // Covers S = 0
        let op_code = 0b1110_00_1_1000_0_0000_0001_0000_10101010;
        let mut cpu = Arm7tdmi::new(vec![]);
        let op_code = cpu.decode(op_code);

        cpu.registers.set_register_at(0, 0b11111111);

        cpu.execute(op_code);

        assert!(!cpu.cpsr.zero_flag());
        assert!(!cpu.cpsr.sign_flag());

        cpu.cpsr.set_sign_flag(true);
        // Covers S = 1
        let op_code = 0b1110_00_1_1000_1_0000_0001_0000_00000000;
        let op_code = cpu.decode(op_code);

        cpu.execute(op_code);

        assert!(cpu.cpsr.zero_flag());
        assert!(!cpu.cpsr.sign_flag());
    }

    #[test]
    fn check_bic() {
        let op_code = 0b1110_00_1_1110_0_0000_0001_0000_10101010;
        let mut cpu = Arm7tdmi::new(vec![]);
        let op_code = cpu.decode(op_code);

        cpu.registers.set_register_at(0, 0b11111111);

        cpu.execute(op_code);

        assert_eq!(cpu.registers.register_at(1), 0b01010101);
    }

    #[test]
    fn check_mvn() {
        let op_code = 0b1110_00_1_1111_1_0000_0001_0000_11111111;
        let mut cpu = Arm7tdmi::new(vec![]);
        let op_code = cpu.decode(op_code);

        cpu.execute(op_code);

        assert_eq!(cpu.registers.register_at(1), (2_u32.pow(24) - 1) << 8);
        assert!(cpu.cpsr.sign_flag());
    }

    #[test]
    fn check_sub() {
        let op_code = 0b1110_00_0_0010_1_0000_0001_00000_00_0_0010;
        let mut cpu = Arm7tdmi::new(vec![]);
        let op_code = cpu.decode(op_code);

        cpu.registers.set_register_at(0, 10);
        cpu.registers.set_register_at(2, 5);
        cpu.execute(op_code);

        assert_eq!(cpu.registers.register_at(1), 5);
        assert!(!cpu.cpsr.carry_flag());
        assert!(!cpu.cpsr.overflow_flag());
        assert!(!cpu.cpsr.zero_flag());
        assert!(!cpu.cpsr.sign_flag());

        //Covers carry logic
        let op_code = 0b1110_00_0_0010_1_0000_0001_00000_00_0_0010;
        let op_code = cpu.decode(op_code);
        cpu.registers.set_register_at(2, 15);
        cpu.execute(op_code);

        assert_eq!(cpu.registers.register_at(1) as i32, -5);
        assert!(cpu.cpsr.carry_flag());
        assert!(!cpu.cpsr.overflow_flag());
        assert!(cpu.cpsr.sign_flag());
        assert!(!cpu.cpsr.zero_flag());

        // Covers overflow logic
        let op_code = 0b1110_00_0_0010_1_0000_0001_00000_00_0_0010;
        let op_code = cpu.decode(op_code);

        cpu.registers.set_register_at(0, 1);
        cpu.registers.set_register_at(2, i32::MIN as u32);

        cpu.execute(op_code);

        assert_eq!(cpu.registers.register_at(1), (i32::MIN + 1) as u32);
        assert!(cpu.cpsr.carry_flag());
        assert!(cpu.cpsr.overflow_flag());
        assert!(cpu.cpsr.sign_flag());
        assert!(!cpu.cpsr.zero_flag());
    }
}
