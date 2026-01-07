//!
//! This module contains flat(aka new ZKsyncOS) storage model implementation.
//!
//! It's fixed height merkle tree with linked list in the leaves sorted by storage keys.
//! Account data hashes stored in this tree and published separately.
//!
pub mod account_cache;
mod account_cache_entry;
pub mod cost_constants;
pub mod preimage_cache;
mod simple_growable_storage;
pub mod storage_cache;

pub use self::account_cache::*;
pub use self::account_cache_entry::*;
pub use self::preimage_cache::*;
pub use self::simple_growable_storage::*;
pub use self::storage_cache::*;
use core::alloc::Allocator;
use crypto::MiniDigest;
use ruint::aliases::B160;
use storage_models::common_structs::snapshottable_io::SnapshottableIo;
use storage_models::common_structs::StorageCacheModel;
use storage_models::common_structs::StorageModel;
use zk_ee::common_structs::{derive_flat_storage_key_with_hasher, ValueDiffCompressionStrategy};
use zk_ee::internal_error;
use zk_ee::system::errors::internal::InternalError;
use zk_ee::system::BalanceSubsystemError;
use zk_ee::system::DeconstructionSubsystemError;
use zk_ee::system::NonceSubsystemError;
use zk_ee::system::Resources;
use zk_ee::utils::write_bytes::WriteBytes;
use zk_ee::{
    common_structs::{
        history_map::CacheSnapshotId, state_root_view::StateRootView, WarmStorageKey,
    },
    execution_environment_type::ExecutionEnvironmentType,
    memory::stack_trait::StackFactory,
    oracle::IOOracle,
    system::{
        errors::system::SystemError, logger::Logger, AccountData, AccountDataRequest,
        IOResultKeeper, Maybe,
    },
    types_config::{EthereumIOTypesConfig, SystemIOTypesConfig},
    utils::Bytes32,
};

pub fn address_into_special_storage_key(address: &B160) -> Bytes32 {
    let mut key = Bytes32::zero();
    key.as_u8_array_mut()[12..].copy_from_slice(&address.to_be_bytes::<{ B160::BYTES }>());

    key
}

pub const TREE_HEIGHT: usize = 64;

/// Subspace mask for flat storage oracle queries within the system
pub const FLAT_STORAGE_SUBSPACE_MASK: u32 = 0x00_00_f0_00;

// This model only touches storage related things, even though preimages cache can be reused
// by "signals" in theory, but we do not expect that in practice

pub struct FlatTreeWithAccountsUnderHashesStorageModel<
    A: Allocator + Clone,
    R: Resources,
    P: StorageAccessPolicy<R, Bytes32>,
    SF: StackFactory<M>,
    const M: usize,
    const PROOF_ENV: bool,
> {
    pub(crate) storage_cache: NewStorageWithAccountPropertiesUnderHash<A, SF, M, R, P>,
    pub(crate) preimages_cache: BytecodeAndAccountDataPreimagesStorage<R, A>,
    pub(crate) account_data_cache: NewModelAccountCache<A, R, P, SF, M>,
    pub(crate) allocator: A,
}

pub struct FlatTreeWithAccountsUnderHashesStorageModelStateSnapshot {
    storage: StorageSnapshotId,
    account_data: CacheSnapshotId,
    preimages: CacheSnapshotId,
}

impl<
        A: Allocator + Clone + Default,
        R: Resources,
        P: StorageAccessPolicy<R, Bytes32>,
        SF: StackFactory<M>,
        const M: usize,
        const PROOF_ENV: bool,
    > StorageModel for FlatTreeWithAccountsUnderHashesStorageModel<A, R, P, SF, M, PROOF_ENV>
{
    type Allocator = A;
    type Resources = R;
    type StorageCommitment = FlatStorageCommitment<TREE_HEIGHT>;

    type IOTypes = EthereumIOTypesConfig;
    type InitData = P;

    fn construct(init_data: Self::InitData, allocator: Self::Allocator) -> Self {
        let resources_policy = init_data;
        let storage_cache = NewStorageWithAccountPropertiesUnderHash::<A, SF, M, R, P>(
            GenericPubdataAwarePlainStorage::new_from_parts(allocator.clone(), resources_policy),
        );
        let preimages_cache =
            BytecodeAndAccountDataPreimagesStorage::<R, A>::new_from_parts(allocator.clone());
        let account_data_cache =
            NewModelAccountCache::<A, R, P, SF, M>::new_from_parts(allocator.clone());

        Self {
            storage_cache,
            preimages_cache,
            account_data_cache,
            allocator,
        }
    }

    fn pubdata_used_by_tx(&self) -> u32 {
        self.account_data_cache.calculate_pubdata_used_by_tx()
            + self.storage_cache.calculate_pubdata_used_by_tx()
    }

    /// Standard finish method that completes storage model processing.
    fn finish<T: WriteBytes + ?Sized>(
        self,
        oracle: &mut impl IOOracle,
        state_commitment: Option<&mut Self::StorageCommitment>,
        pubdata_dst: &mut T,
        result_keeper: &mut impl IOResultKeeper<Self::IOTypes>,
        logger: &mut impl Logger,
    ) -> Result<(), InternalError> {
        // Complete the finalization but discard the returned storage cache
        let _ =
            self.finish_internal(oracle, state_commitment, pubdata_dst, result_keeper, logger)?;

        Ok(())
    }

    /// This method extracts the final state changes after finishing the storage model
    /// and computes a deterministic hash over all storage key-value pairs that were modified.
    /// Can be used to validate that forward execution and RISC-V
    /// proof execution produce identical state changes.
    fn finish_and_calculate_state_diffs_hash<T: WriteBytes + ?Sized>(
        self,
        oracle: &mut impl IOOracle,
        state_commitment: Option<&mut Self::StorageCommitment>,
        pubdata_dst: &mut T,
        result_keeper: &mut impl IOResultKeeper<Self::IOTypes>,
        logger: &mut impl Logger,
    ) -> Result<Bytes32, InternalError> {
        // First complete the normal storage finalization process
        let storage_cache =
            self.finish_internal(oracle, state_commitment, pubdata_dst, result_keeper, logger)?;

        let mut hasher = crypto::blake2s::Blake2s256::new();
        let mut state_diffs_hasher = crypto::blake2s::Blake2s256::new();

        // Iterate through all modified storage entries and hash them deterministically
        // RV64 diagnostic: count entries and track skipped ones
        #[cfg(target_arch = "riscv64")]
        let mut _total_count: usize = 0;
        #[cfg(target_arch = "riscv64")]
        let mut _skip_count: usize = 0;
        #[cfg(target_arch = "riscv64")]
        let mut _included_count: usize = 0;

        // RV64 diagnostic helpers
        #[cfg(target_arch = "riscv64")]
        const UART_ADDR: u64 = 0xa000_0200;
        #[cfg(target_arch = "riscv64")]
        fn uart_byte(b: u8) {
            unsafe { core::ptr::write_volatile(UART_ADDR as *mut u8, b); }
        }
        #[cfg(target_arch = "riscv64")]
        fn uart_str(s: &str) {
            for b in s.bytes() { uart_byte(b); }
        }
        #[cfg(target_arch = "riscv64")]
        fn uart_hex8(val: u8) {
            const HEX: &[u8; 16] = b"0123456789abcdef";
            uart_byte(HEX[(val >> 4) as usize]);
            uart_byte(HEX[(val & 0xf) as usize]);
        }
        #[cfg(target_arch = "riscv64")]
        fn uart_hex_bytes(bytes: &[u8], max: usize) {
            for i in 0..max.min(bytes.len()) { uart_hex8(bytes[i]); }
        }
        #[cfg(target_arch = "riscv64")]
        fn uart_usize(val: usize) {
            for i in (0..16).rev() { uart_hex8(((val >> (i*4)) & 0xf) as u8); }
        }

        storage_cache
            .0
            .cache
            .apply_to_all_updated_elements::<_, ()>(|l, r, k| {
                #[cfg(target_arch = "riscv64")]
                {
                    _total_count += 1;
                }

                // Get the derived key for this entry to check against known missing keys
                #[cfg(target_arch = "riscv64")]
                let derived_key_for_check = derive_flat_storage_key_with_hasher(&k.address, &k.key, &mut crypto::blake2s::Blake2s256::new());

                // RV64: Log EVERY entry with initial vs current for debugging
                #[cfg(target_arch = "riscv64")]
                {
                    // Check if this is one of the 4 known missing keys
                    let dk_bytes = derived_key_for_check.as_u8_ref();
                    let is_missing_key =
                        // 8908e55383727f855f11109219d27468fa62def21dd6bc23e7f85cb7e4697f51
                        (dk_bytes[0] == 0x89 && dk_bytes[1] == 0x08 && dk_bytes[2] == 0xe5) ||
                        // 2922c8a9a89a4cb23e2396657c4ade6108f800ee859c1a8c88b403ecbad8f89c
                        (dk_bytes[0] == 0x29 && dk_bytes[1] == 0x22 && dk_bytes[2] == 0xc8) ||
                        // 3a3598499bbe8a9d7414cbf8ec28568df5c9d164da388b3bdb68a52d53e184e8
                        (dk_bytes[0] == 0x3a && dk_bytes[1] == 0x35 && dk_bytes[2] == 0x98) ||
                        // dca25cf9f452b663b4f8c41c6a362a2cc8c5f65a2dd605b626670978ab71f3ba
                        (dk_bytes[0] == 0xdc && dk_bytes[1] == 0xa2 && dk_bytes[2] == 0x5c);

                    if is_missing_key {
                        // Log detailed info for the 4 known missing keys
                        uart_str("[MISSING_KEY] dk=");
                        uart_hex_bytes(dk_bytes, 8);
                        uart_str("\n  init=");
                        uart_hex_bytes(l.value().as_u8_ref(), 32);
                        uart_str("\n  curr=");
                        uart_hex_bytes(r.value().as_u8_ref(), 32);
                        uart_str("\n  eq=");
                        uart_byte(if l.value() == r.value() { b'T' } else { b'F' });
                        uart_byte(b'\n');

                        // Also log raw usize values for comparison
                        uart_str("  init_raw: ");
                        let init_arr = l.value().as_u64_array_ref();
                        for i in 0..4 {
                            uart_usize(init_arr[i] as usize);
                            uart_byte(b' ');
                        }
                        uart_byte(b'\n');
                        uart_str("  curr_raw: ");
                        let curr_arr = r.value().as_u64_array_ref();
                        for i in 0..4 {
                            uart_usize(curr_arr[i] as usize);
                            uart_byte(b' ');
                        }
                        uart_byte(b'\n');

                        // Check byte-by-byte equality
                        let init_bytes = l.value().as_u8_ref();
                        let curr_bytes = r.value().as_u8_ref();
                        let mut diff_count = 0;
                        for i in 0..32 {
                            if init_bytes[i] != curr_bytes[i] {
                                diff_count += 1;
                            }
                        }
                        uart_str("  diff_bytes=");
                        uart_hex8(diff_count);
                        uart_byte(b'\n');
                    }
                }

                // Skip entries where the value didn't actually change
                if l.value() == r.value() {
                    #[cfg(target_arch = "riscv64")]
                    {
                        _skip_count += 1;
                    }
                    return Ok(());
                }

                #[cfg(target_arch = "riscv64")]
                {
                    _included_count += 1;
                }
                let derived_key =
                    derive_flat_storage_key_with_hasher(&k.address, &k.key, &mut hasher);

                logger
                    .write_fmt(format_args!(
                        "State diffs hash - key: {:?}, new value: {:?}\n",
                        derived_key,
                        r.value()
                    ))
                    .ok();

                // Hash the derived key and new value together to create deterministic state diff hash
                state_diffs_hasher.update(derived_key.as_u8_ref());
                state_diffs_hasher.update(r.value().as_u8_ref());
                Ok(())
            })
            .map_err(|_| internal_error!("Failed to compute state diffs hash"))?;

        Ok(state_diffs_hasher.finalize().into())
    }

    fn storage_read(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        key: &<Self::IOTypes as SystemIOTypesConfig>::StorageKey,
        oracle: &mut impl IOOracle,
    ) -> Result<<Self::IOTypes as SystemIOTypesConfig>::StorageKey, SystemError> {
        self.storage_cache
            .read(ee_type, resources, address, key, oracle)
    }

    fn storage_touch(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        key: &<Self::IOTypes as SystemIOTypesConfig>::StorageKey,
        oracle: &mut impl IOOracle,
        is_access_list: bool,
    ) -> Result<(), SystemError> {
        self.storage_cache
            .touch(ee_type, resources, address, key, oracle, is_access_list)
    }

    fn storage_write(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        key: &<Self::IOTypes as SystemIOTypesConfig>::StorageKey,
        new_value: &<Self::IOTypes as SystemIOTypesConfig>::StorageValue,
        oracle: &mut impl IOOracle,
    ) -> Result<<Self::IOTypes as SystemIOTypesConfig>::StorageValue, SystemError> {
        self.storage_cache
            .write(ee_type, resources, address, key, new_value, oracle)
    }

    fn read_account_properties<
        EEVersion: Maybe<u8>,
        ObservableBytecodeHash: Maybe<<Self::IOTypes as SystemIOTypesConfig>::BytecodeHashValue>,
        ObservableBytecodeLen: Maybe<u32>,
        Nonce: Maybe<u64>,
        BytecodeHash: Maybe<<Self::IOTypes as SystemIOTypesConfig>::BytecodeHashValue>,
        BytecodeLen: Maybe<u32>,
        ArtifactsLen: Maybe<u32>,
        NominalTokenBalance: Maybe<<Self::IOTypes as SystemIOTypesConfig>::NominalTokenValue>,
        Bytecode: Maybe<&'static [u8]>,
        CodeVersion: Maybe<u8>,
        IsDelegated: Maybe<bool>,
    >(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        request: AccountDataRequest<
            AccountData<
                EEVersion,
                ObservableBytecodeHash,
                ObservableBytecodeLen,
                Nonce,
                BytecodeHash,
                BytecodeLen,
                ArtifactsLen,
                NominalTokenBalance,
                Bytecode,
                CodeVersion,
                IsDelegated,
            >,
        >,
        oracle: &mut impl IOOracle,
    ) -> Result<
        AccountData<
            EEVersion,
            ObservableBytecodeHash,
            ObservableBytecodeLen,
            Nonce,
            BytecodeHash,
            BytecodeLen,
            ArtifactsLen,
            NominalTokenBalance,
            Bytecode,
            CodeVersion,
            IsDelegated,
        >,
        SystemError,
    > {
        self.account_data_cache
            .read_account_properties::<PROOF_ENV, _, _, _, _, _, _, _, _, _, _, _>(
                ee_type,
                resources,
                address,
                request,
                &mut self.storage_cache,
                &mut self.preimages_cache,
                oracle,
            )
    }

    fn touch_account(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        oracle: &mut impl IOOracle,
        is_access_list: bool,
    ) -> Result<(), SystemError> {
        self.account_data_cache.touch_account::<PROOF_ENV>(
            ee_type,
            resources,
            address,
            &mut self.storage_cache,
            &mut self.preimages_cache,
            oracle,
            is_access_list,
        )
    }

    fn get_selfbalance(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
    ) -> Result<<Self::IOTypes as SystemIOTypesConfig>::NominalTokenValue, SystemError> {
        self.account_data_cache
            .read_account_balance_assuming_warm(ee_type, resources, address)
    }

    fn deploy_code(
        &mut self,
        from_ee: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        at_address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        bytecode: &[u8],
        oracle: &mut impl IOOracle,
    ) -> Result<
        (
            &'static [u8],
            <Self::IOTypes as SystemIOTypesConfig>::BytecodeHashValue,
            u32,
        ),
        SystemError,
    > {
        self.account_data_cache.deploy_code::<PROOF_ENV>(
            from_ee,
            resources,
            at_address,
            bytecode,
            &mut self.storage_cache,
            &mut self.preimages_cache,
            oracle,
        )
    }

    fn set_bytecode_details(
        &mut self,
        resources: &mut R,
        at_address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        ee: ExecutionEnvironmentType,
        bytecode_hash: Bytes32,
        bytecode_len: u32,
        artifacts_len: u32,
        observable_bytecode_hash: Bytes32,
        observable_bytecode_len: u32,
        oracle: &mut impl IOOracle,
    ) -> Result<(), SystemError> {
        self.account_data_cache.set_bytecode_details::<PROOF_ENV>(
            resources,
            at_address,
            ee,
            bytecode_hash,
            bytecode_len,
            artifacts_len,
            observable_bytecode_hash,
            observable_bytecode_len,
            &mut self.storage_cache,
            &mut self.preimages_cache,
            oracle,
        )
    }

    fn set_delegation(
        &mut self,
        resources: &mut R,
        at_address: &B160,
        delegate: &B160,
        oracle: &mut impl IOOracle,
    ) -> Result<(), SystemError> {
        self.account_data_cache.set_delegation::<PROOF_ENV>(
            resources,
            at_address,
            delegate,
            &mut self.storage_cache,
            &mut self.preimages_cache,
            oracle,
        )
    }

    fn mark_for_deconstruction(
        &mut self,
        from_ee: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        at_address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        nominal_token_beneficiary: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        oracle: &mut impl IOOracle,
        in_constructor: bool,
    ) -> Result<
        <Self::IOTypes as SystemIOTypesConfig>::NominalTokenValue,
        DeconstructionSubsystemError,
    > {
        self.account_data_cache
            .mark_for_deconstruction::<PROOF_ENV>(
                from_ee,
                resources,
                at_address,
                nominal_token_beneficiary,
                &mut self.storage_cache,
                &mut self.preimages_cache,
                oracle,
                in_constructor,
            )
    }

    fn increment_nonce(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        increment_by: u64,
        oracle: &mut impl IOOracle,
    ) -> Result<u64, NonceSubsystemError> {
        self.account_data_cache.increment_nonce::<PROOF_ENV>(
            ee_type,
            resources,
            address,
            increment_by,
            &mut self.storage_cache,
            &mut self.preimages_cache,
            oracle,
        )
    }

    fn transfer_nominal_token_value(
        &mut self,
        from_ee: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        from: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        to: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        amount: &<Self::IOTypes as SystemIOTypesConfig>::NominalTokenValue,
        oracle: &mut impl IOOracle,
    ) -> Result<(), BalanceSubsystemError> {
        self.account_data_cache
            .transfer_nominal_token_value::<PROOF_ENV>(
                from_ee,
                resources,
                from,
                to,
                amount,
                &mut self.storage_cache,
                &mut self.preimages_cache,
                oracle,
            )
    }

    fn update_nominal_token_value(
        &mut self,
        from_ee: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        update_fn: impl FnOnce(
            &<Self::IOTypes as SystemIOTypesConfig>::NominalTokenValue,
        ) -> Result<
            <Self::IOTypes as SystemIOTypesConfig>::NominalTokenValue,
            BalanceSubsystemError,
        >,
        oracle: &mut impl IOOracle,
    ) -> Result<<Self::IOTypes as SystemIOTypesConfig>::NominalTokenValue, BalanceSubsystemError>
    {
        self.account_data_cache
            .update_nominal_token_value::<PROOF_ENV>(
                from_ee,
                resources,
                address,
                update_fn,
                &mut self.storage_cache,
                &mut self.preimages_cache,
                oracle,
            )
    }

    fn get_refund_counter(&self) -> u32 {
        *self
            .storage_cache
            .0
            .evm_refunds_counter
            .value()
            .unwrap_or(&0)
    }

    // Add EVM refund to counter
    fn add_evm_refund(&mut self, refund: u32) -> Result<(), SystemError> {
        let mut gas_refunds = self
            .storage_cache
            .0
            .evm_refunds_counter
            .value()
            .copied()
            .unwrap_or_default();
        gas_refunds += refund;
        self.storage_cache.0.evm_refunds_counter.update(gas_refunds);
        Ok(())
    }
}

impl<
        A: Allocator + Clone + Default,
        R: Resources,
        P: StorageAccessPolicy<R, Bytes32>,
        SF: StackFactory<M>,
        const M: usize,
        const PROOF_ENV: bool,
    > FlatTreeWithAccountsUnderHashesStorageModel<A, R, P, SF, M, PROOF_ENV>
{
    /// Internal implementation shared by both `finish` and `finish_state_diffs_hash`.
    ///
    /// This method performs the complete storage finalization process:
    /// 1. Persists account changes to storage cache
    /// 2. Returns uncompressed state diffs to the result keeper
    /// 3. Computes and commits compressed pubdata
    /// 4. Verifies and applies all storage reads/writes to the state commitment
    ///
    /// Returns the final storage cache for further processing by the caller.
    fn finish_internal<T: WriteBytes + ?Sized>(
        self,
        oracle: &mut impl IOOracle,
        state_commitment: Option<&mut <Self as StorageModel>::StorageCommitment>,
        pubdata_dst: &mut T,
        result_keeper: &mut impl IOResultKeeper<<Self as StorageModel>::IOTypes>,
        logger: &mut impl Logger,
    ) -> Result<NewStorageWithAccountPropertiesUnderHash<A, SF, M, R, P>, InternalError> {
        let Self {
            mut storage_cache,
            mut preimages_cache,
            mut account_data_cache,
            allocator,
        } = self;
        // flush accounts into storage
        account_data_cache
            .persist_changes(
                &mut storage_cache,
                &mut preimages_cache,
                oracle,
                result_keeper,
            )
            .expect("must persist changes from account cache");

        // 1. Return uncompressed state diffs for sequencer
        result_keeper.storage_diffs(storage_cache.net_diffs_iter().map(|(k, v)| {
            let WarmStorageKey { address, key } = k;
            let value = v.current_value;
            (address, key, value)
        }));
        preimages_cache.report_new_preimages(result_keeper)?;

        // 2. Commit to/return compressed pubdata
        let encdoded_state_diffs_count =
            (storage_cache.net_diffs_iter().count() as u32).to_be_bytes();
        pubdata_dst.write(&encdoded_state_diffs_count);
        result_keeper.pubdata(&encdoded_state_diffs_count);

        let mut hasher = crypto::blake2s::Blake2s256::new();
        storage_cache
            .0
            .cache
            .apply_to_all_updated_elements::<_, ()>(|l, r, k| {
                // Skip on empty diff
                if l.value() == r.value() {
                    return Ok(());
                }
                // TODO(EVM-1074): use tree index instead of key for repeated writes
                let derived_key =
                    derive_flat_storage_key_with_hasher(&k.address, &k.key, &mut hasher);
                pubdata_dst.write(derived_key.as_u8_ref());
                result_keeper.pubdata(derived_key.as_u8_ref());

                // we publish preimages for account details
                if k.address == ACCOUNT_PROPERTIES_STORAGE_ADDRESS {
                    let account_address = B160::try_from_be_slice(&k.key.as_u8_ref()[12..])
                        .unwrap()
                        .into();
                    let cache_item = account_data_cache.cache.get(&account_address).ok_or(())?;
                    let (l, r) = cache_item.get_initial_and_last_values().ok_or(())?;
                    AccountProperties::diff_compression::<PROOF_ENV, _, _, _>(
                        l.value(),
                        r.value(),
                        r.metadata().not_publish_bytecode,
                        pubdata_dst,
                        result_keeper,
                        &mut preimages_cache,
                        oracle,
                    )
                    .map_err(|_| ())?;
                } else {
                    ValueDiffCompressionStrategy::optimal_compression(
                        l.value(),
                        r.value(),
                        pubdata_dst,
                        result_keeper,
                    );
                }
                Ok(())
            })
            .map_err(|_| internal_error!("Failed to compute pubdata"))?;

        // 3. Verify/apply reads and writes
        cycle_marker::wrap!("verify_and_apply_batch", {
            if let Some(state_commitment) = state_commitment {
                let it = storage_cache.net_accesses_iter();
                state_commitment.verify_and_apply_batch(oracle, it, allocator, logger)
            } else {
                Ok(())
            }
        })?;

        Ok(storage_cache)
    }
}

impl<
        A: Allocator + Clone + Default,
        R: Resources,
        P: StorageAccessPolicy<R, Bytes32>,
        SF: StackFactory<M>,
        const M: usize,
        const PROOF_ENV: bool,
    > SnapshottableIo for FlatTreeWithAccountsUnderHashesStorageModel<A, R, P, SF, M, PROOF_ENV>
{
    type StateSnapshot = FlatTreeWithAccountsUnderHashesStorageModelStateSnapshot;

    fn begin_new_tx(&mut self) {
        self.storage_cache.begin_new_tx();
        self.preimages_cache.begin_new_tx();
        self.account_data_cache.begin_new_tx();
    }

    fn finish_tx(&mut self) -> Result<(), zk_ee::system::errors::internal::InternalError> {
        self.account_data_cache.finish_tx(&mut self.storage_cache)?;
        self.storage_cache.finish_tx()?;
        self.preimages_cache.finish_tx()
    }

    fn start_frame(&mut self) -> Self::StateSnapshot {
        let storage_handle = self.storage_cache.start_frame();
        let preimages_handle = self.preimages_cache.start_frame();
        let account_handle = self.account_data_cache.start_frame();

        FlatTreeWithAccountsUnderHashesStorageModelStateSnapshot {
            storage: storage_handle,
            preimages: preimages_handle,
            account_data: account_handle,
        }
    }

    fn finish_frame(
        &mut self,
        rollback_handle: Option<&Self::StateSnapshot>,
    ) -> Result<(), InternalError> {
        self.storage_cache
            .finish_frame(rollback_handle.map(|x| &x.storage))?;
        self.preimages_cache
            .finish_frame(rollback_handle.map(|x| &x.preimages))?;
        self.account_data_cache
            .finish_frame(rollback_handle.map(|x| &x.account_data))?;

        Ok(())
    }
}
