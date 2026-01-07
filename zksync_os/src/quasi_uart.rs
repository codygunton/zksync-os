// UART output for guest logging
//
// Two modes based on target architecture:
// - 32-bit (Airbender): CSR-based QuasiUART protocol
// - 64-bit (Zisk): Memory-mapped UART at 0xa0000200

#[cfg(target_pointer_width = "32")]
use riscv_common::{csr_read_word, csr_write_word};

/// Memory-mapped UART address for Zisk (64-bit)
#[cfg(target_pointer_width = "64")]
const UART_ADDR: u64 = 0xa000_0200;

#[derive(Default)]
pub struct QuasiUART {
    #[cfg(target_pointer_width = "32")]
    buffer: [u8; 4],
    #[cfg(target_pointer_width = "32")]
    len: usize,
}

impl QuasiUART {
    #[cfg(target_pointer_width = "32")]
    const HELLO_MARKER: u32 = u32::MAX;

    #[inline(never)]
    pub const fn new() -> Self {
        Self {
            #[cfg(target_pointer_width = "32")]
            buffer: [0u8; 4],
            #[cfg(target_pointer_width = "32")]
            len: 0,
        }
    }

    // === 32-bit (Airbender) CSR-based implementation ===

    #[cfg(target_pointer_width = "32")]
    #[inline(never)]
    pub fn write_entry_sequence(&mut self, message_len: usize) {
        csr_write_word(Self::HELLO_MARKER as usize);
        // now write length is words for query
        csr_write_word(message_len.next_multiple_of(4) / 4 + 1);
        csr_write_word(message_len);
    }

    #[cfg(target_pointer_width = "32")]
    #[inline(never)]
    pub fn write_word(&self, word: u32) {
        csr_write_word(word as usize);
    }

    #[cfg(target_pointer_width = "32")]
    #[inline(never)]
    pub fn read_word(&self) -> usize {
        csr_read_word() as usize
    }

    #[cfg(target_pointer_width = "32")]
    #[inline(never)]
    fn write_byte(&mut self, byte: u8) {
        self.buffer[self.len] = byte;
        self.len += 1;
        if self.len == 4 {
            self.len = 0;
            let word = u32::from_le_bytes(self.buffer);
            self.write_word(word);
        }
    }

    #[cfg(target_pointer_width = "32")]
    fn flush(&mut self) {
        if self.len == 0 {
            // cleanup and return
            for dst in self.buffer.iter_mut() {
                *dst = 0;
            }
            return;
        }
        for i in self.len..4 {
            self.buffer[i] = 0u8;
        }
        self.len = 0;
        csr_write_word(u32::from_le_bytes(self.buffer) as usize);
    }

    // === 64-bit (Zisk) memory-mapped UART implementation ===

    #[cfg(target_pointer_width = "64")]
    #[inline(never)]
    fn write_byte_to_uart(byte: u8) {
        unsafe {
            core::ptr::write_volatile(UART_ADDR as *mut u8, byte);
        }
    }

    #[inline(never)]
    pub fn write_debug<T: core::fmt::Debug>(value: &T) {
        use core::fmt::Write;
        let mut writer = Self::new();
        let mut string = heapless::String::<64>::new(); // 64 byte string buffer
        let Ok(_) = write!(string, "{:?}", value) else {
            let _ = writer.write_str("too long debug");
            return;
        };
        let _ = writer.write_str(&string);
    }
}

impl core::fmt::Write for QuasiUART {
    #[cfg(target_pointer_width = "32")]
    fn write_str(&mut self, s: &str) -> Result<(), core::fmt::Error> {
        self.write_entry_sequence(s.len());
        for c in s.bytes() {
            self.write_byte(c);
        }
        self.flush();

        Ok(())
    }

    #[cfg(target_pointer_width = "64")]
    fn write_str(&mut self, s: &str) -> Result<(), core::fmt::Error> {
        for c in s.bytes() {
            Self::write_byte_to_uart(c);
        }
        Ok(())
    }
}

impl proof_running_system::zk_ee::system::logger::Logger for QuasiUART {
    #[cfg(target_pointer_width = "32")]
    fn log_data(&mut self, src: impl ExactSizeIterator<Item = u8>) -> core::fmt::Result {
        let expected_len = src.len() * 2;
        self.write_entry_sequence(expected_len);
        let mut string = heapless::String::<4>::new();
        for byte in src {
            use core::fmt::Write;
            let _ = write!(&mut string, "{:02x}", byte);
            for c in string.bytes() {
                self.write_byte(c);
            }
            string.clear();
        }
        self.flush();

        Ok(())
    }

    #[cfg(target_pointer_width = "64")]
    fn log_data(&mut self, src: impl ExactSizeIterator<Item = u8>) -> core::fmt::Result {
        let mut string = heapless::String::<4>::new();
        for byte in src {
            use core::fmt::Write;
            let _ = write!(&mut string, "{:02x}", byte);
            for c in string.bytes() {
                Self::write_byte_to_uart(c);
            }
            string.clear();
        }
        Ok(())
    }
}
