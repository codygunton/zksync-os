use zk_ee::{
    kv_markers::{UsizeDeserializable, UsizeSerializable},
    system::errors::internal::InternalError,
    system_io_oracle::*,
};

pub trait NonDeterminismCSRSourceImplementation: 'static + Clone + Copy + core::fmt::Debug {
    fn csr_read_impl() -> usize;
    fn csr_write_impl(value: usize);
}

#[derive(Clone, Copy, Debug)]
pub struct CsrBasedIOOracle<I: NonDeterminismCSRSourceImplementation> {
    _marker: core::marker::PhantomData<I>,
}

pub struct CsrBasedIOOracleIterator<I: NonDeterminismCSRSourceImplementation> {
    remaining: usize,
    _marker: core::marker::PhantomData<I>,
}

impl<I: NonDeterminismCSRSourceImplementation> Iterator for CsrBasedIOOracleIterator<I> {
    type Item = usize;
    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            None
        } else {
            self.remaining -= 1;
            cfg_if::cfg_if! {
                if #[cfg(target_pointer_width = "32")] {
                    Some(I::csr_read_impl())
                } else if #[cfg(target_pointer_width = "64")] {
                    // On 64-bit, oracle provides u32 values (same as 32-bit witness)
                    // Read two u32s and combine into one u64
                    let low = I::csr_read_impl() as u32;
                    let high = I::csr_read_impl() as u32;
                    Some(((high as usize) << 32) | (low as usize))
                }
            }
        }
    }
}

impl<I: NonDeterminismCSRSourceImplementation> ExactSizeIterator for CsrBasedIOOracleIterator<I> {
    fn len(&self) -> usize {
        self.remaining
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DummyCSRImpl;

impl NonDeterminismCSRSourceImplementation for DummyCSRImpl {
    fn csr_read_impl() -> usize {
        0
    }
    fn csr_write_impl(_value: usize) {}
}
impl<I: NonDeterminismCSRSourceImplementation> CsrBasedIOOracle<I> {
    pub fn init() -> Self {
        Self {
            _marker: core::marker::PhantomData,
        }
    }
}

impl<NDS: NonDeterminismCSRSourceImplementation> IOOracle for CsrBasedIOOracle<NDS> {
    type RawIterator<'a> = CsrBasedIOOracleIterator<NDS>;

    fn raw_query<'a, I: UsizeSerializable + UsizeDeserializable>(
        &'a mut self,
        query_type: u32,
        input: &I,
    ) -> Result<Self::RawIterator<'a>, InternalError> {
        NDS::csr_write_impl(query_type as usize);
        let iter_to_write = UsizeSerializable::iter(input);
        // write length
        let iterator_len = iter_to_write.len();
        assert!(iterator_len == <I as UsizeSerializable>::USIZE_LEN);
        cfg_if::cfg_if! {
            if #[cfg(target_pointer_width = "32")] {
                NDS::csr_write_impl(iterator_len);
            } else if #[cfg(target_pointer_width = "64")] {
                // On 64-bit, write length as count of u32s (doubled)
                NDS::csr_write_impl(iterator_len * 2);
            }
        }
        // write content
        let mut remaining_len = iterator_len;
        for value in iter_to_write {
            assert!(remaining_len != 0);
            cfg_if::cfg_if! {
                if #[cfg(target_pointer_width = "32")] {
                    NDS::csr_write_impl(value);
                } else if #[cfg(target_pointer_width = "64")] {
                    // On 64-bit, split each u64 into two u32 writes
                    NDS::csr_write_impl((value as u32) as usize);
                    NDS::csr_write_impl(((value >> 32) as u32) as usize);
                }
            }
            remaining_len -= 1;
        }
        assert!(remaining_len == 0);
        // we can expect that length of the result is returned via read
        cfg_if::cfg_if! {
            if #[cfg(target_pointer_width = "32")] {
                let remaining_len = NDS::csr_read_impl();
            } else if #[cfg(target_pointer_width = "64")] {
                // On 64-bit, oracle returns u32 count, divide by 2 for usize count
                let remaining_len = NDS::csr_read_impl() / 2;
            }
        }
        let it = CsrBasedIOOracleIterator::<NDS> {
            remaining: remaining_len,
            _marker: core::marker::PhantomData,
        };

        Ok(it)
    }
}
