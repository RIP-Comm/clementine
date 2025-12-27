//! # ARM Instruction Decoding
//!
//! This module handles decoding 32-bit ARM instructions into their component
//! fields and classifying them by type.
//!
//! ## Instruction Categories
//!
//! ARM instructions are identified by examining specific bit patterns:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │                    ARM Instruction Categories                           │
//! ├─────────────────────────────────────────────────────────────────────────┤
//! │                                                                         │
//! │  Bits 27-25 determine the basic category:                              │
//! │                                                                         │
//! │  000 + special patterns  →  Multiply, Multiply Long, SWP, BX           │
//! │  000                     →  Data Processing (register operand)         │
//! │  001                     →  Data Processing (immediate operand)        │
//! │  010                     →  Load/Store (immediate offset)              │
//! │  011                     →  Load/Store (register offset)               │
//! │  100                     →  Block Data Transfer (LDM/STM)              │
//! │  101                     →  Branch (B/BL)                              │
//! │  110                     →  Coprocessor Data Transfer                  │
//! │  111                     →  Software Interrupt / Coprocessor ops       │
//! │                                                                         │
//! └─────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Decoding Priority
//!
//! Some instructions have overlapping bit patterns. The decoder checks
//! instructions in this priority order:
//!
//! 1. Branch and Exchange (BX) - very specific pattern
//! 2. Single Data Swap (SWP/SWPB)
//! 3. Multiply Long (UMULL, SMULL, UMLAL, SMLAL)
//! 4. Multiply (MUL, MLA)
//! 5. Halfword Data Transfer (LDRH, STRH, LDRSB, LDRSH)
//! 6. Software Interrupt (SWI)
//! 7. Coprocessor operations
//! 8. Block Data Transfer (LDM, STM)
//! 9. Branch (B, BL)
//! 10. Single Data Transfer (LDR, STR)
//! 11. Data Processing (AND, ADD, etc.)
//!
//! ## Instruction Encoding Example
//!
//! ```text
//! ADD R0, R1, R2, LSL #3
//!
//! 31-28  27-26  25  24-21  20  19-16  15-12  11-7   6-5  4  3-0
//! [1110] [ 00 ] [0] [0100] [0] [0001] [0000] [00011][00] [0][0010]
//!   ↑       ↑    ↑    ↑     ↑    ↑      ↑      ↑     ↑   ↑   ↑
//!   │       │    │    │     │    │      │      │     │   │   └─ Rm = R2
//!   │       │    │    │     │    │      │      │     │   └──── Shift by imm
//!   │       │    │    │     │    │      │      │     └──────── LSL
//!   │       │    │    │     │    │      │      └────────────── Shift = 3
//!   │       │    │    │     │    │      └───────────────────── Rd = R0
//!   │       │    │    │     │    └──────────────────────────── Rn = R1
//!   │       │    │    │     └───────────────────────────────── S = 0 (no flags)
//!   │       │    │    └─────────────────────────────────────── ADD opcode
//!   │       │    └──────────────────────────────────────────── Register operand
//!   │       └───────────────────────────────────────────────── Data processing
//!   └───────────────────────────────────────────────────────── Always execute
//! ```

use crate::bitwise::Bits;
use crate::cpu::arm::alu_instruction::{AluSecondOperandInfo, ArmModeAluInstr, ShiftOperator};
use crate::cpu::arm7tdmi::HalfwordTransferKind;
use crate::cpu::condition::Condition;
use crate::cpu::flags::{
    HalfwordDataTransferOffsetKind, Indexing, LoadStoreKind, Offsetting, OperandKind,
    ReadWriteKind, ShiftKind,
};
use serde::{Deserialize, Serialize};

use super::alu_instruction::{PsrKind, PsrOpKind};

/// The type of single data transfer operation (LDR/STR).
///
/// Determined by the L bit (bit 20):
/// - L=0: Store (STR)
/// - L=1: Load (LDR)
///
/// PLD (Preload Data) is a cache hint instruction added in `ARMv5TE`.
#[derive(Debug, Eq, PartialEq, Copy, Clone, Serialize, Deserialize)]
pub enum SingleDataTransferKind {
    /// Load from memory into a register (`LDR`).
    Ldr,

    /// Store from a register into memory (`STR`).
    Str,

    /// Preload Data cache hint (`ARMv5TE`+, not commonly used on GBA).
    Pld,
}

impl From<u32> for SingleDataTransferKind {
    fn from(op_code: u32) -> Self {
        let must_for_pld = op_code.are_bits_on(28..=31);
        if op_code.get_bit(20) {
            if must_for_pld { Self::Pld } else { Self::Ldr }
        } else {
            Self::Str
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SingleDataTransferOffsetInfo {
    Immediate {
        offset: u32,
    },
    RegisterImmediate {
        shift_amount: u32,
        shift_kind: ShiftKind,
        reg_offset: u32,
    },
}

impl std::fmt::Display for SingleDataTransferOffsetInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Immediate { offset } => {
                f.write_str("#")?;
                // FIXME: should we put the sign?
                write!(f, "{offset}")?;
            }
            Self::RegisterImmediate {
                shift_amount,
                shift_kind,
                reg_offset,
            } => {
                write!(f, "{reg_offset}, {shift_kind} #{shift_amount}")?;
            }
        }

        Ok(())
    }
}
/// All ARM instruction types after decoding.
///
/// This enum represents a fully decoded ARM instruction with all fields
/// extracted and validated. The [`From<u32>`] implementation performs
/// the decoding.
///
/// ## Instruction Categories
///
/// | Variant                | Example Instructions       | Description                    |
/// |------------------------|---------------------------|--------------------------------|
/// | `DataProcessing`       | AND, ADD, CMP, MOV        | ALU operations                 |
/// | `Multiply`             | MUL, MLA                  | 32-bit multiply                |
/// | `MultiplyLong`         | UMULL, SMULL              | 64-bit multiply                |
/// | `PSRTransfer`          | MRS, MSR                  | Status register access         |
/// | `SingleDataSwap`       | SWP, SWPB                 | Atomic memory swap             |
/// | `BranchAndExchange`    | BX                        | Branch + possible ARM↔Thumb    |
/// | `HalfwordDataTransfer` | LDRH, STRH, LDRSB         | 16-bit and signed loads        |
/// | `SingleDataTransfer`   | LDR, STR, LDRB            | 32-bit and byte loads/stores   |
/// | `BlockDataTransfer`    | LDM, STM                  | Multiple register load/store   |
/// | `Branch`               | B, BL                     | Branch (and link)              |
/// | `SoftwareInterrupt`    | SWI                       | BIOS call                      |
/// | `Undefined`            | -                         | Triggers undefined exception   |
#[derive(Debug, PartialEq, Eq, Copy, Clone, Serialize, Deserialize)]
pub enum ArmModeInstruction {
    /// Data Processing: ALU operations (AND, ADD, CMP, MOV, etc.)
    DataProcessing {
        condition: Condition,
        alu_instruction: ArmModeAluInstr,
        set_conditions: bool,
        op_kind: OperandKind,
        rn: u32,
        destination: u32,
        op2: AluSecondOperandInfo,
    },
    Multiply {
        variant: ArmModeMultiplyVariant,
        condition: Condition,
        should_set_codes: bool,
        rd_destination_register: u32,
        rn_accumulate_register: u32,
        rs_operand_register: u32,
        rm_operand_register: u32,
    },
    MultiplyLong {
        variant: ArmModeMultiplyLongVariant,
        condition: Condition,
        should_set_codes: bool,
        rdhi_destination_register: u32,
        rdlo_destination_register: u32,
        rs_operand_register: u32,
        rm_operand_register: u32,
    },
    PSRTransfer {
        condition: Condition,
        psr_kind: PsrKind,
        kind: PsrOpKind,
    },
    SingleDataSwap {
        condition: Condition,
        byte: bool, // true = byte, false = word
        rn: u32,    // base register (address)
        rd: u32,    // destination register
        rm: u32,    // source register
    },
    BranchAndExchange {
        condition: Condition,
        register: usize,
    },
    HalfwordDataTransfer {
        condition: Condition,
        indexing: Indexing,
        offsetting: Offsetting,
        write_back: bool,
        load_store_kind: LoadStoreKind,
        offset_kind: HalfwordDataTransferOffsetKind,
        base_register: u32,
        source_destination_register: u32,
        transfer_kind: HalfwordTransferKind,
    },
    SingleDataTransfer {
        condition: Condition,
        kind: SingleDataTransferKind,
        quantity: ReadWriteKind,
        write_back: bool,
        indexing: Indexing,
        rd: u32,
        base_register: u32,
        offset_info: SingleDataTransferOffsetInfo,
        offsetting: Offsetting,
    },
    Undefined,
    BlockDataTransfer {
        condition: Condition,
        indexing: Indexing,
        offsetting: Offsetting,
        load_psr: bool,
        write_back: bool,
        load_store: LoadStoreKind,
        rn: u32,
        register_list: u32,
    },
    Branch {
        condition: Condition,
        link: bool,
        offset: u32,
    },
    CoprocessorDataTransfer {
        condition: Condition,
        indexing: Indexing,
        offsetting: Offsetting,
        transfer_length: bool,
        write_back: bool,
        load_store: LoadStoreKind,
        rn: u32,
        crd: u32,
        cp_number: u32,
        offset: u32,
    },
    CoprocessorDataOperation,
    CoprocessorRegisterTransfer,
    SoftwareInterrupt,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArmModeMultiplyVariant {
    Mul,
    Mla,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArmModeMultiplyLongVariant {
    Umull,
    Umlal,
    Smull,
    Smlal,
}

impl std::fmt::Display for ArmModeMultiplyLongVariant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Umull => f.write_str("UMULL"),
            Self::Umlal => f.write_str("UMLAL"),
            Self::Smull => f.write_str("SMULL"),
            Self::Smlal => f.write_str("SMLAL"),
        }
    }
}

impl From<u32> for ArmModeMultiplyVariant {
    fn from(op_code: u32) -> Self {
        let mul_op_code: u32 = op_code.get_bits(21..=24);
        match mul_op_code {
            0b0000 => Self::Mul,
            0b0001 => Self::Mla,
            _ => unreachable!(),
        }
    }
}

impl From<u32> for ArmModeMultiplyLongVariant {
    fn from(op_code: u32) -> Self {
        let mul_op_code: u32 = op_code.get_bits(21..=24);
        match mul_op_code {
            0b0100 => Self::Umull,
            0b0101 => Self::Umlal,
            0b0110 => Self::Smull,
            0b0111 => Self::Smlal,
            _ => unreachable!(),
        }
    }
}

impl ArmModeInstruction {
    #[must_use]
    #[allow(clippy::too_many_lines)]
    #[allow(clippy::missing_panics_doc)]
    pub fn disassembler(&self) -> String {
        match self {
            Self::DataProcessing {
                condition,
                alu_instruction,
                set_conditions,
                op_kind: _,
                rn,
                destination,
                op2,
            } => {
                let set_string = if *set_conditions { "S" } else { "" };
                match alu_instruction {
                    ArmModeAluInstr::And
                    | ArmModeAluInstr::Eor
                    | ArmModeAluInstr::Sub
                    | ArmModeAluInstr::Rsb
                    | ArmModeAluInstr::Add
                    | ArmModeAluInstr::Adc
                    | ArmModeAluInstr::Sbc
                    | ArmModeAluInstr::Rsc
                    | ArmModeAluInstr::Orr
                    | ArmModeAluInstr::Bic => {
                        format!(
                            "{alu_instruction}{condition}{set_string} R{destination}, R{rn}, {op2}"
                        )
                    }
                    ArmModeAluInstr::Tst
                    | ArmModeAluInstr::Teq
                    | ArmModeAluInstr::Cmp
                    | ArmModeAluInstr::Cmn => {
                        format!("{alu_instruction}{condition} R{rn}, {op2}")
                    }
                    ArmModeAluInstr::Mov | ArmModeAluInstr::Mvn => {
                        format!("{alu_instruction}{condition}{set_string} R{destination}, {op2}")
                    }
                }
            }
            Self::Multiply {
                variant,
                condition,
                should_set_codes,
                rd_destination_register,
                rn_accumulate_register,
                rs_operand_register,
                rm_operand_register,
            } => match variant {
                ArmModeMultiplyVariant::Mul => format!(
                    "MUL{condition}{should_set_codes} {rd_destination_register}, {rm_operand_register}, {rs_operand_register}"
                ),
                ArmModeMultiplyVariant::Mla => format!(
                    "MLA{condition}{should_set_codes} {rd_destination_register}, {rm_operand_register}, {rs_operand_register}, {rn_accumulate_register}"
                ),
            },
            Self::MultiplyLong {
                variant,
                condition,
                should_set_codes,
                rdhi_destination_register,
                rdlo_destination_register,
                rs_operand_register,
                rm_operand_register,
            } => {
                format!(
                    "{variant}{condition}{should_set_codes} {rdlo_destination_register}, {rdhi_destination_register}, {rm_operand_register}, {rs_operand_register}"
                )
            }
            Self::PSRTransfer {
                condition,
                psr_kind,
                kind,
            } => match kind {
                PsrOpKind::Mrs {
                    destination_register,
                } => {
                    format!("MRS{condition} R{destination_register}, {psr_kind}")
                }
                PsrOpKind::Msr { source_register } => {
                    format!("MSR{condition} {psr_kind}, R{source_register}")
                }
                PsrOpKind::MsrFlg {
                    operand,
                    field_mask,
                } => {
                    format!("MSR{condition} {psr_kind}, {operand} (mask: 0x{field_mask:X})")
                }
            },
            Self::SingleDataSwap {
                condition,
                byte,
                rn,
                rd,
                rm,
            } => {
                let mnemonic = if *byte { "swpb" } else { "swp" };
                format!("{mnemonic}{condition} r{rd}, r{rm}, [r{rn}]")
            }
            Self::BranchAndExchange {
                condition,
                register,
            } => format!("BX{condition} R{register}"),
            Self::HalfwordDataTransfer {
                condition,
                indexing,
                offsetting,
                load_store_kind,
                transfer_kind,
                source_destination_register,
                offset_kind,
                base_register,
                write_back,
                ..
            } => {
                let sign = match offsetting {
                    Offsetting::Up => "+",
                    Offsetting::Down => "-",
                };

                let offset = match offset_kind {
                    HalfwordDataTransferOffsetKind::Immediate { offset } => {
                        if *offset == 0 {
                            String::new()
                        } else {
                            format!(",#{sign}{offset}")
                        }
                    }
                    HalfwordDataTransferOffsetKind::Register { register } => {
                        format!(",{sign}R{register}")
                    }
                };

                let w = if *write_back { "!" } else { "" };

                let address = match indexing {
                    Indexing::Pre => {
                        format!("[R{base_register}{offset}{w}]")
                    }
                    Indexing::Post => {
                        format!("[R{base_register}]{offset}")
                    }
                };

                format!(
                    "{load_store_kind}{condition}{transfer_kind} R{source_destination_register}, {address}"
                )
            }
            Self::SingleDataTransfer {
                condition,
                kind,
                quantity,
                write_back,
                indexing,
                rd,
                base_register: _,
                offset_info,
                offsetting: _,
            } => {
                let b = match quantity {
                    ReadWriteKind::Word => "",
                    ReadWriteKind::Byte => "B",
                };

                let _ = write_back;
                let _ = offset_info;
                let _ = indexing;

                // let address = base_register;

                let op = match kind {
                    SingleDataTransferKind::Ldr => "LDR",
                    SingleDataTransferKind::Str => "STR",
                    SingleDataTransferKind::Pld => unimplemented!(),
                };

                format!("{op}{condition}{b} R{rd}, {offset_info}")
            }
            Self::Undefined => panic!("Undefined not implemented"),
            Self::BlockDataTransfer {
                condition,
                indexing,
                offsetting,
                load_psr,
                write_back,
                load_store,
                rn,
                register_list,
            } => {
                let op = match load_store {
                    LoadStoreKind::Store => "STM",
                    LoadStoreKind::Load => "LDM",
                };

                let offset_modifier = match offsetting {
                    Offsetting::Down => "D",
                    Offsetting::Up => "I",
                };
                let index_type = match indexing {
                    Indexing::Pre => "B",
                    Indexing::Post => "A",
                };

                let mut regs = Vec::new();
                for i in 0..=15 {
                    if register_list.get_bit(i) {
                        regs.push(format!("R{i}"));
                    }
                }
                let registers = regs.join("-");

                let w = if *write_back { "!" } else { "" };
                let f = if *load_psr { "^" } else { "" };
                format!("{op}{condition}{offset_modifier}{index_type}, R{rn}{w} {{{registers}}}{f}")
            }
            Self::Branch {
                condition,
                link,
                offset,
            } => {
                let link = if *link { "L" } else { "" };
                format!("B{link}{condition} 0x{offset:08X}")
            }
            Self::CoprocessorDataTransfer {
                condition,
                indexing,
                offsetting,
                transfer_length,
                write_back,
                load_store,
                rn,
                crd,
                cp_number,
                offset,
            } => {
                // <LDC|STC>{cond}{L} p#,cd,<Address>
                let op = match load_store {
                    LoadStoreKind::Store => "SDC",
                    LoadStoreKind::Load => "LDC",
                };
                let long_transfer = if *transfer_length { "L" } else { "" };

                let address = offset;
                let _ = rn;
                let _ = write_back;
                let _ = offsetting;
                let _ = indexing;
                // FIXME: Finish address
                format!("{op}{condition}{long_transfer} p{cp_number},{crd},{address:08X}")
            }
            Self::CoprocessorDataOperation => panic!("CoprocessorDataOperation not implemented"),
            Self::CoprocessorRegisterTransfer => {
                panic!("CoprocessorRegisterTransfer not implemented")
            }
            Self::SoftwareInterrupt => panic!("SoftwareInterrupt not implemented"),
        }
    }
}

impl From<u32> for ArmModeInstruction {
    #[allow(clippy::too_many_lines)]
    fn from(op_code: u32) -> Self {
        let condition = Condition::from(op_code.get_bits(28..=31) as u8);
        // NOTE: The order is based on how many bits are already know at decoding time.
        // It can happen `op_code` coalesced into one/two or more than two possible solution, that's because
        // we tried to order with this priority.
        if op_code.get_bits(4..=27) == 0b0001_0010_1111_1111_1111_0001 {
            let register = op_code.get_bits(0..=3) as usize;
            Self::BranchAndExchange {
                condition,
                register,
            }
        } else if op_code.get_bits(23..=27) == 0b00010
            && op_code.get_bits(20..=21) == 0b00
            && op_code.get_bits(4..=11) == 0b0000_1001
        {
            let byte = op_code.get_bit(22);
            let rn = op_code.get_bits(16..=19);
            let rd = op_code.get_bits(12..=15);
            let rm = op_code.get_bits(0..=3);

            Self::SingleDataSwap {
                condition,
                byte,
                rn,
                rd,
                rm,
            }
        } else if op_code.get_bits(23..=27) == 0b00001 && op_code.get_bits(4..=7) == 0b1001 {
            let variant = ArmModeMultiplyLongVariant::from(op_code);

            let should_set_codes = op_code.get_bit(20);

            let rm_operand_register = op_code.get_bits(0..=3);
            let rs_operand_register = op_code.get_bits(8..=11);
            let rdlo_destination_register = op_code.get_bits(12..=15);
            let rdhi_destination_register = op_code.get_bits(16..=19);

            Self::MultiplyLong {
                variant,
                condition,
                should_set_codes,
                rdhi_destination_register,
                rdlo_destination_register,
                rm_operand_register,
                rs_operand_register,
            }
        } else if op_code.get_bits(22..=27) == 0b00_0000 && op_code.get_bits(4..=7) == 0b1001 {
            let variant = ArmModeMultiplyVariant::from(op_code);

            let should_set_codes = op_code.get_bit(20);

            let rm_operand_register = op_code.get_bits(0..=3);
            let rs_operand_register = op_code.get_bits(8..=11);
            let rn_accumulate_register = op_code.get_bits(12..=15);
            let rd_destination_register = op_code.get_bits(16..=19);

            Self::Multiply {
                variant,
                condition,
                should_set_codes,
                rd_destination_register,
                rn_accumulate_register,
                rm_operand_register,
                rs_operand_register,
            }
        } else if op_code.get_bits(25..=27) == 0b000 && op_code.get_bit(7) && op_code.get_bit(4) {
            // Check if this is a SWAP instruction (SH bits = 00) or Halfword transfer (SH bits != 00)
            let sh_bits = op_code.get_bits(5..=6);

            if sh_bits == 0b00 {
                // This is a SWP/SWPB instruction
                let byte = op_code.get_bit(22);
                let rn = op_code.get_bits(16..=19);
                let rd = op_code.get_bits(12..=15);
                let rm = op_code.get_bits(0..=3);
                Self::SingleDataSwap {
                    condition,
                    byte,
                    rn,
                    rd,
                    rm,
                }
            } else {
                let indexing: Indexing = op_code.get_bit(24).into();
                let offsetting: Offsetting = op_code.get_bit(23).into();
                let write_back = op_code.get_bit(21);
                let load_store_kind: LoadStoreKind = op_code.get_bit(20).into();
                let base_register = op_code.get_bits(16..=19);
                let source_destination_register = op_code.get_bits(12..=15);
                let transfer_kind: HalfwordTransferKind = (sh_bits as u8).into();
                let operand_kind: OperandKind = op_code.get_bit(22).into();

                Self::HalfwordDataTransfer {
                    condition,
                    indexing,
                    offsetting,
                    write_back,
                    load_store_kind,
                    offset_kind: if operand_kind == OperandKind::Register {
                        HalfwordDataTransferOffsetKind::Register {
                            register: op_code.get_bits(0..=3),
                        }
                    } else {
                        let immediate_offset_high = op_code.get_bits(8..=11);
                        let immediate_offset_low = op_code.get_bits(0..=3);

                        HalfwordDataTransferOffsetKind::Immediate {
                            offset: (immediate_offset_high << 4) | immediate_offset_low,
                        }
                    },
                    base_register,
                    source_destination_register,
                    transfer_kind,
                }
            }
        } else if op_code.get_bits(25..=27) == 0b011 && op_code.get_bit(4) {
            tracing::debug!(
                "undefined instruction decode: opcode=0x{op_code:08X}, bits[25-27]=0b011, bit[4]=1"
            );
            Self::Undefined
        } else if op_code.get_bits(24..=27) == 0b1111 {
            Self::SoftwareInterrupt
        } else if op_code.get_bits(24..=27) == 0b1110 && op_code.get_bit(4) {
            Self::CoprocessorRegisterTransfer
        } else if op_code.get_bits(24..=27) == 0b1110 && !op_code.get_bit(4) {
            Self::CoprocessorDataOperation
        } else if op_code.get_bits(25..=27) == 0b110 {
            let indexing: Indexing = op_code.get_bit(24).into();
            let offsetting: Offsetting = op_code.get_bit(23).into();
            let transfer_length = op_code.get_bit(22);
            let write_back = op_code.get_bit(21);
            let load_store: LoadStoreKind = op_code.get_bit(20).into();

            let rn = op_code.get_bits(16..=19);
            let crd = op_code.get_bits(12..=15);
            let cp_number = op_code.get_bits(8..=11);
            let offset = op_code.get_bits(0..=7);

            Self::CoprocessorDataTransfer {
                condition,
                indexing,
                offsetting,
                transfer_length,
                write_back,
                load_store,
                rn,
                crd,
                cp_number,
                offset,
            }
        } else if op_code.get_bits(25..=27) == 0b100 {
            let indexing = op_code.get_bit(24).into();
            let offsetting = op_code.get_bit(23).into();
            let load_psr = op_code.get_bit(22);
            let write_back = op_code.get_bit(21);
            let load_store = op_code.get_bit(20).into();
            let rn = op_code.get_bits(16..=19);
            let reg_list = op_code.get_bits(0..=15);

            Self::BlockDataTransfer {
                condition,
                indexing,
                offsetting,
                load_psr,
                write_back,
                load_store,
                rn,
                register_list: reg_list,
            }
        } else if op_code.get_bits(25..=27) == 0b101 {
            let link = op_code.get_bit(24);
            let offset = op_code.get_bits(0..=23) << 2;
            Self::Branch {
                condition,
                link,
                offset,
            }
        } else if op_code.get_bits(26..=27) == 0b01 {
            // NOTE: This bit is negated because the meaning is inverted in SingleDataTransfer then other istructions.
            let op_kind: OperandKind = (!op_code.get_bit(25)).into();
            let indexing: Indexing = op_code.get_bit(24).into(); // FIXME: should we use this?
            let offsetting: Offsetting = op_code.get_bit(23).into();
            let byte_or_word: ReadWriteKind = op_code.into(); // TODO: is this the same for all instruction?
            let load_store: SingleDataTransferKind = op_code.into(); // TODO: is this the same bit for all instruction?
            let write_back = op_code.get_bit(21);
            let rn = op_code.get_bits(16..=19);
            let rd = op_code.get_bits(12..=15);

            let offset_info = match op_kind {
                OperandKind::Immediate => {
                    let offset = op_code.get_bits(0..=11);
                    SingleDataTransferOffsetInfo::Immediate { offset }
                }
                OperandKind::Register => {
                    let shift_amount = op_code.get_bits(7..=11);
                    let shift_kind: ShiftKind = op_code.get_bits(5..=6).into();
                    let reg_offset = op_code.get_bits(0..=3);
                    SingleDataTransferOffsetInfo::RegisterImmediate {
                        shift_amount,
                        shift_kind,
                        reg_offset,
                    }
                }
            };

            Self::SingleDataTransfer {
                condition,
                kind: load_store,
                quantity: byte_or_word,
                write_back,
                indexing,
                rd,
                base_register: rn,
                offset_info,
                offsetting,
            }
        } else if op_code.get_bits(26..=27) == 0b00 {
            let alu_instruction = op_code.get_bits(21..=24).into();
            let set_conditions = op_code.get_bit(20);
            let rn = op_code.get_bits(16..=19);
            let op_kind: OperandKind = op_code.get_bit(25).into();
            let rd = op_code.get_bits(12..=15);

            // Check if this is a PSR instruction (MRS/MSR/MSR_FLG)
            // PSR instructions use TST/TEQ/CMP/CMN encodings with S=0, but only specific patterns are valid
            if matches!(
                alu_instruction,
                ArmModeAluInstr::Tst
                    | ArmModeAluInstr::Teq
                    | ArmModeAluInstr::Cmp
                    | ArmModeAluInstr::Cmn
            ) && !set_conditions
            {
                // Check if it matches one of the valid PSR patterns
                let is_mrs = op_code.get_bits(23..=27) == 0b0_0010
                    && op_code.get_bits(16..=21) == 0b00_1111
                    && op_code.get_bits(0..=11) == 0b0000_0000_0000;

                let is_msr = op_code.get_bits(23..=27) == 0b00010
                    && op_code.get_bits(12..=21) == 0b10_1001_1111
                    && op_code.get_bits(4..=11) == 0b0000_0000;

                let is_msr_flg = op_code.get_bits(26..=27) == 0b00
                    && op_code.get_bits(23..=24) == 0b10
                    && op_code.get_bits(20..=21) == 0b10
                    && op_code.get_bits(12..=15) == 0b1111;

                if is_mrs || is_msr || is_msr_flg {
                    // Valid PSR instruction
                    return Self::PSRTransfer {
                        condition,
                        psr_kind: PsrKind::from(op_code.get_bit(22)),
                        kind: PsrOpKind::try_from(op_code)
                            .expect("PSR instruction validation passed but conversion failed"),
                    };
                }
                // TST/TEQ/CMP/CMN with S=0 but doesn't match PSR patterns
                // This is likely an undefined instruction, but let's log details
                tracing::debug!(
                    "Potential undefined: opcode=0x{:08X}, alu_op={:?}, I={}, Rn={}, Rd={}, bits[0-11]=0x{:03X}",
                    op_code,
                    alu_instruction,
                    op_kind as u8,
                    rn,
                    rd,
                    op_code.get_bits(0..=11)
                );

                // Actually, some emulators treat these as NOP or execute them anyway
                // For now, let's just skip the instruction instead of triggering exception
                // This is more forgiving for ROM test codes
                tracing::debug!("Treating as NOP");
                // Return a NOP-like instruction (MOV R0, R0)
                return Self::DataProcessing {
                    condition,
                    alu_instruction: ArmModeAluInstr::Mov,
                    set_conditions: false,
                    op_kind: OperandKind::Register,
                    rn: 0,
                    destination: 0,
                    op2: AluSecondOperandInfo::Register {
                        shift_op: ShiftOperator::Immediate(0),
                        shift_kind: ShiftKind::Lsl,
                        register: 0,
                    },
                };
            }

            let op2 = match op_kind {
                OperandKind::Immediate => {
                    let shift = op_code.get_bits(8..=11) * 2;
                    let base = op_code.get_bits(0..=7);
                    AluSecondOperandInfo::Immediate { base, shift }
                }
                OperandKind::Register => {
                    let shift_kind: ShiftKind = op_code.get_bits(5..=6).into();
                    let shift_by_register_bit = op_code.get_bit(4);
                    let register = op_code.get_bits(0..=3);
                    let shift_op = if shift_by_register_bit {
                        if op_code.get_bit(7) {
                            todo!("should be zero or need different work")
                        }
                        ShiftOperator::Register(op_code.get_bits(8..=11))
                    } else {
                        ShiftOperator::Immediate(op_code.get_bits(7..=11))
                    };
                    AluSecondOperandInfo::Register {
                        shift_op,
                        shift_kind,
                        register,
                    }
                }
            };

            Self::DataProcessing {
                condition,
                alu_instruction,
                set_conditions,
                op_kind,
                rn,
                destination: rd,
                op2,
            }
        } else {
            tracing::debug!("not identified instruction");
            unimplemented!()
        }
    }
}

impl std::fmt::Display for ArmModeInstruction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn decode_branch() {
        let output = ArmModeInstruction::from(0b1110_1011_0000_0000_0000_0000_0111_1111);
        assert_eq!(
            ArmModeInstruction::Branch {
                condition: Condition::AL,
                link: true,
                offset: 508,
            },
            output
        );
        assert_eq!("BL 0x000001FC", output.disassembler());

        let output = ArmModeInstruction::from(0b1110_1010_0000_0000_0000_0000_0111_1111);
        assert_eq!(
            ArmModeInstruction::Branch {
                condition: Condition::AL,
                link: false,
                offset: 508,
            },
            output
        );
        assert_eq!("B 0x000001FC", output.disassembler());

        let output = ArmModeInstruction::from(0b0000_1010_0000_0000_0000_0000_0111_1111);
        assert_eq!(
            ArmModeInstruction::Branch {
                condition: Condition::EQ,
                link: false,
                offset: 508,
            },
            output
        );
        assert_eq!("BEQ 0x000001FC", output.disassembler());

        let output = ArmModeInstruction::from(0b0000_1011_0000_0000_0000_0000_0111_1111);
        assert_eq!(
            ArmModeInstruction::Branch {
                condition: Condition::EQ,
                link: true,
                offset: 508,
            },
            output
        );
        assert_eq!("BLEQ 0x000001FC", output.disassembler());
    }

    #[test]
    fn decode_branch_and_exchange() {
        let output = ArmModeInstruction::from(0b1110_0001_0010_1111_1111_1111_0001_0001);
        assert_eq!(
            ArmModeInstruction::BranchAndExchange {
                condition: Condition::AL,
                register: 1
            },
            output
        );
        assert_eq!("BX R1", output.disassembler());

        let output = ArmModeInstruction::from(0b0000_0001_0010_1111_1111_1111_0001_0001);
        assert_eq!(
            ArmModeInstruction::BranchAndExchange {
                condition: Condition::EQ,
                register: 1
            },
            output
        );
        assert_eq!("BXEQ R1", output.disassembler());
    }

    #[test]
    fn decode_data_processing() {
        let output = ArmModeInstruction::from(0b1110_00_0_1011_0_1001_1111_000000001110);
        assert_eq!(
            ArmModeInstruction::PSRTransfer {
                condition: Condition::AL,
                psr_kind: PsrKind::Spsr,
                kind: PsrOpKind::Msr {
                    source_register: 14
                }
            },
            output
        );
        assert_eq!("MSR SPSR, R14", output.disassembler());
    }

    #[test]
    fn decode_half_word_data_transfer_immediate_offset() {
        let output = ArmModeInstruction::from(0b1110_0001_1100_0001_0000_0000_1011_0000);
        assert_eq!(
            ArmModeInstruction::HalfwordDataTransfer {
                condition: Condition::AL,
                indexing: Indexing::Pre,
                offsetting: Offsetting::Up,
                write_back: false,
                load_store_kind: LoadStoreKind::Store,
                offset_kind: HalfwordDataTransferOffsetKind::Immediate { offset: 0 },
                base_register: 1,
                source_destination_register: 0,
                transfer_kind: HalfwordTransferKind::UnsignedHalfwords,
            },
            output
        );
    }

    #[test]
    fn decode_half_word_data_transfer_register_offset() {
        let output = ArmModeInstruction::from(0b1110_0001_1000_0010_0000_0000_1011_0001);
        assert_eq!(
            ArmModeInstruction::HalfwordDataTransfer {
                condition: Condition::AL,
                indexing: Indexing::Pre,
                offsetting: Offsetting::Up,
                write_back: false,
                load_store_kind: LoadStoreKind::Store,
                offset_kind: HalfwordDataTransferOffsetKind::Register { register: 1 },
                base_register: 2,
                source_destination_register: 0,
                transfer_kind: HalfwordTransferKind::UnsignedHalfwords,
            },
            output
        );
    }

    #[test]
    fn decode_single_data_transfer() {
        let output = ArmModeInstruction::from(0b11100111010100010101000000001100);
        assert_eq!(
            output,
            ArmModeInstruction::SingleDataTransfer {
                condition: Condition::AL,
                kind: SingleDataTransferKind::Ldr,
                quantity: ReadWriteKind::Byte,
                write_back: false,
                indexing: Indexing::Pre,
                rd: 5,
                base_register: 1,
                offset_info: SingleDataTransferOffsetInfo::RegisterImmediate {
                    shift_amount: 0,
                    shift_kind: ShiftKind::Lsl,
                    reg_offset: 12
                },
                offsetting: Offsetting::Down
            }
        );

        assert_eq!("LDRB R5, 12, LSL #0", output.disassembler());
    }

    #[test]
    fn decode_single_data_swap() {
        // SWP R1, R2, [R3] - swap word
        // Encoding: cond 0001 0B00 Rn Rd 0000 1001 Rm
        // B=0 (word), Rn=3, Rd=1, Rm=2
        let output = ArmModeInstruction::from(0b1110_0001_0000_0011_0001_0000_1001_0010);
        assert_eq!(
            output,
            ArmModeInstruction::SingleDataSwap {
                condition: Condition::AL,
                byte: false,
                rn: 3,
                rd: 1,
                rm: 2,
            }
        );
        assert_eq!("swp r1, r2, [r3]", output.disassembler());

        // SWPB R4, R5, [R6] - swap byte
        // B=1 (byte), Rn=6, Rd=4, Rm=5
        let output = ArmModeInstruction::from(0b1110_0001_0100_0110_0100_0000_1001_0101);
        assert_eq!(
            output,
            ArmModeInstruction::SingleDataSwap {
                condition: Condition::AL,
                byte: true,
                rn: 6,
                rd: 4,
                rm: 5,
            }
        );
        assert_eq!("swpb r4, r5, [r6]", output.disassembler());

        // SWPNE R0, R1, [R2] - conditional swap
        let output = ArmModeInstruction::from(0b0001_0001_0000_0010_0000_0000_1001_0001);
        assert_eq!(
            output,
            ArmModeInstruction::SingleDataSwap {
                condition: Condition::NE,
                byte: false,
                rn: 2,
                rd: 0,
                rm: 1,
            }
        );
        assert_eq!("swpNE r0, r1, [r2]", output.disassembler());
    }

    #[test]
    fn decode_swap_vs_halfword_transfer() {
        // Verify that SWP (SH bits = 00) is correctly distinguished from halfword transfers
        // SWP has bits 6-5 = 00, halfword transfers have bits 6-5 != 00

        // This is SWP (bits 6-5 = 00)
        let swp_opcode = 0b1110_0001_0000_0011_0001_0000_1001_0010;
        let output = ArmModeInstruction::from(swp_opcode);
        assert!(matches!(output, ArmModeInstruction::SingleDataSwap { .. }));

        // This is STRH (bits 6-5 = 01, unsigned halfword)
        let strh_opcode = 0b1110_0001_1100_0001_0000_0000_1011_0000;
        let output = ArmModeInstruction::from(strh_opcode);
        assert!(matches!(
            output,
            ArmModeInstruction::HalfwordDataTransfer { .. }
        ));
    }
}
