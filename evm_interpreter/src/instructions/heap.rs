use crate::gas::gas_utils;

use super::*;
use core::ops::DerefMut;
use native_resource_constants::*;
use zk_ee::system::System;

impl<S: EthereumLikeTypes> Interpreter<'_, S> {
    pub fn mload(&mut self, system: &mut System<S>) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::VERYLOW, MLOAD_NATIVE_COST)?;
        let stack_top = self.stack.top_mut()?;
        let index = Self::cast_to_usize(stack_top, EvmError::InvalidOperandOOG.into())?;
        Self::resize_heap_implementation(&mut self.heap, &mut self.gas, index, 32)?;
        let mut value: ruint::Uint<256, 4> = U256::ZERO;
        unsafe {
            // Use volatile reads to work around RV64 compiler optimization bugs
            // See ai_plans/riscv-compiler-bugs.md for details
            let src = self.heap.deref_mut().as_ptr().add(index);
            let dst = value.as_le_slice_mut().as_mut_ptr();
            for i in 0..32 {
                let byte = core::ptr::read_volatile(src.add(i));
                core::ptr::write_volatile(dst.add(i), byte);
            }
            crate::utils::bytereverse_u256(&mut value);
        }

        if Self::PRINT_OPCODES {
            use core::fmt::Write;
            let _ = system
                .get_logger()
                .write_fmt(format_args!(" offset: {index}, read value: 0x{value:0x}"));
        }

        *stack_top = value;
        Ok(())
    }

    pub fn mstore(&mut self, system: &mut System<S>) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::VERYLOW, MSTORE_NATIVE_COST)?;
        let (index, value) = self.stack.pop_2()?;
        let mut le_value = *value;
        let index = Self::cast_to_usize(index, EvmError::InvalidOperandOOG.into())?;

        self.resize_heap(index, 32)?;

        unsafe {
            crate::utils::bytereverse_u256(&mut le_value);
            // Use volatile writes to work around RV64 compiler optimization bugs
            // See ai_plans/riscv-compiler-bugs.md for details
            let src = le_value.as_le_slice().as_ptr();
            let dst = self.heap().as_mut_ptr().add(index);
            for i in 0..32 {
                let byte = core::ptr::read_volatile(src.add(i));
                core::ptr::write_volatile(dst.add(i), byte);
            }
        }

        if Self::PRINT_OPCODES {
            use core::fmt::Write;
            let _ = system.get_logger().write_fmt(format_args!(
                " offset: {index}, stored value: 0x{le_value:0x}"
            ));
        }

        Ok(())
    }

    pub fn mstore8(&mut self, system: &mut System<S>) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::VERYLOW, MSTORE8_NATIVE_COST)?;
        let (index, value) = self.stack.pop_2()?;
        let index = Self::cast_to_usize(&index, EvmError::InvalidOperandOOG.into())?;
        let value = value.byte(0);
        self.resize_heap(index, 1)?;

        self.heap()[index] = value;

        if Self::PRINT_OPCODES {
            use core::fmt::Write;
            let _ = system
                .get_logger()
                .write_fmt(format_args!(" offset: {index}, stored byte: 0x{value:0x}"));
        }

        Ok(())
    }

    pub fn msize(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::BASE, MSIZE_NATIVE_COST)?;
        let len = self.memory_len();
        debug_assert!(len.next_multiple_of(32) == len);
        self.stack.push(&U256::from(len))
    }

    pub fn mcopy(&mut self) -> InstructionResult {
        let (dst_offset, src_offset, len) = self.stack.pop_3()?;

        let len = Self::cast_to_usize(&len, EvmError::InvalidOperandOOG.into())?;
        let (gas_cost, native_cost) = gas_utils::copy_cost_plus_very_low_gas(len as u64)?;
        self.gas.spend_gas_and_native(gas_cost, native_cost)?;

        if len == 0 {
            return Ok(());
        }

        let dst_offset = Self::cast_to_usize(&dst_offset, EvmError::InvalidOperandOOG.into())?;
        let src_offset = Self::cast_to_usize(&src_offset, EvmError::InvalidOperandOOG.into())?;
        self.resize_heap(core::cmp::max(dst_offset, src_offset), len)?;
        // Use volatile copy to work around RV64 compiler optimization bugs
        // Note: This handles overlapping regions by copying byte-by-byte
        // which is correct but slower than memmove for large regions
        unsafe {
            let src_ptr = self.heap().as_ptr().add(src_offset);
            let dst_ptr = self.heap().as_mut_ptr().add(dst_offset);
            if dst_offset <= src_offset {
                // Copy forward
                for i in 0..len {
                    let byte = core::ptr::read_volatile(src_ptr.add(i));
                    core::ptr::write_volatile(dst_ptr.add(i), byte);
                }
            } else {
                // Copy backward to handle overlapping regions correctly
                for i in (0..len).rev() {
                    let byte = core::ptr::read_volatile(src_ptr.add(i));
                    core::ptr::write_volatile(dst_ptr.add(i), byte);
                }
            }
        }

        Ok(())
    }
}
