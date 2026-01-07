use super::*;
use native_resource_constants::*;

impl<S: EthereumLikeTypes> Interpreter<'_, S> {
    pub fn jump(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::MID, JUMP_NATIVE_COST)?;
        let dest = self.stack.pop_1()?;
        let dest = Self::cast_to_usize(dest, EvmError::InvalidJump.into())?;
        if self.bytecode_preprocessing.is_valid_jumpdest(dest) {
            self.instruction_pointer = dest;
            Ok(())
        } else {
            Err(EvmError::InvalidJump.into())
        }
    }

    pub fn jumpi(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::HIGH, JUMPI_NATIVE_COST)?;
        let (dest, value) = self.stack.pop_2()?;
        if *value != U256::ZERO {
            let dest = Self::cast_to_usize(dest, EvmError::InvalidJump.into())?;
            if self.bytecode_preprocessing.is_valid_jumpdest(dest) {
                self.instruction_pointer = dest;
            } else {
                return Err(EvmError::InvalidJump.into());
            }
        }
        Ok(())
    }

    pub fn jumpdest(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::JUMPDEST, JUMPDEST_NATIVE_COST)?;
        Ok(())
    }

    pub fn pc(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::BASE, PC_NATIVE_COST)?;
        self.stack.push(&U256::from(self.instruction_pointer - 1))?;
        Ok(())
    }

    pub fn ret(&mut self) -> InstructionResult {
        self.gas.spend_gas_and_native(0, RETURN_NATIVE_COST)?;
        let (offset, len) = self.stack.pop_2()?;
        let len = Self::cast_to_usize(len, EvmError::InvalidOperandOOG.into())?;
        if len == 0 {
            self.returndata_location = 0..0;
        } else {
            let offset = Self::cast_to_usize(&offset, EvmError::InvalidOperandOOG.into())?;
            self.resize_heap(offset, len)?;
            let (end, of) = offset.overflowing_add(len);
            if of {
                return Err(EvmError::InvalidOperandOOG.into());
            }
            self.returndata_location = offset..end;
        }
        Err(ExitCode::Return)
    }

    // RV64 UART helpers for REVERT logging
    #[cfg(target_arch = "riscv64")]
    fn revert_uart_byte(b: u8) {
        unsafe {
            core::ptr::write_volatile(0xa000_0200u64 as *mut u8, b);
        }
    }
    #[cfg(target_arch = "riscv64")]
    fn revert_uart_str(s: &str) {
        for b in s.bytes() {
            Self::revert_uart_byte(b);
        }
    }
    #[cfg(target_arch = "riscv64")]
    fn revert_uart_hex_byte(b: u8) {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        Self::revert_uart_byte(HEX[(b >> 4) as usize]);
        Self::revert_uart_byte(HEX[(b & 0xf) as usize]);
    }

    pub fn revert(&mut self) -> InstructionResult {
        self.gas.spend_gas_and_native(0, REVERT_NATIVE_COST)?;

        let (offset, len) = self.stack.pop_2()?;

        // Log REVERT for target contracts - AFTER we have offset/len
        #[cfg(target_arch = "riscv64")]
        {
            let addr_bytes = self.address.to_be_bytes::<20>();
            let is_target =
                (addr_bytes[0] == 0xa1 && addr_bytes[1] == 0x3b && addr_bytes[2] == 0xaf) ||
                (addr_bytes[0] == 0x0d && addr_bytes[1] == 0x7e && addr_bytes[2] == 0x90);
            if is_target {
                Self::revert_uart_str("[REVERT] addr=");
                for i in 0..4 {
                    Self::revert_uart_hex_byte(addr_bytes[i]);
                }
                Self::revert_uart_str(" len=");
                // Print len as decimal
                let len_u64 = len.as_limbs()[0];
                if len_u64 == 0 {
                    Self::revert_uart_byte(b'0');
                } else {
                    let mut digits = [0u8; 20];
                    let mut n = len_u64;
                    let mut i = 0;
                    while n > 0 {
                        digits[i] = (n % 10) as u8 + b'0';
                        n /= 10;
                        i += 1;
                    }
                    while i > 0 {
                        i -= 1;
                        Self::revert_uart_byte(digits[i]);
                    }
                }
                // Print first 36 bytes of revert data (selector + first arg)
                if let Some(off) = offset.as_limbs()[0].checked_add(0) {
                    let off = off as usize;
                    let data_len = core::cmp::min(len_u64 as usize, 36);
                    if off + data_len <= self.heap.len() {
                        Self::revert_uart_str(" data=");
                        for i in 0..data_len {
                            Self::revert_uart_hex_byte(self.heap[off + i]);
                        }
                    }
                }
                Self::revert_uart_str("\n");
            }
        }
        let len = Self::cast_to_usize(len, EvmError::InvalidOperandOOG.into())?;
        if len == 0 {
            self.returndata_location = 0..0;
        } else {
            let offset = Self::cast_to_usize(&offset, EvmError::InvalidOperandOOG.into())?;
            self.resize_heap(offset, len)?;
            let (end, of) = offset.overflowing_add(len);
            if of {
                return Err(EvmError::InvalidOperandOOG.into());
            }
            self.returndata_location = offset..end;
        }
        Err(EvmError::Revert.into())
    }
}
