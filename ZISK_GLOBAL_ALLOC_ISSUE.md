# ZisK Global Allocation Issue

## Summary

The ZisK benchmark fails with a "global alloc not allowed" panic during execution of Ethereum block 24198369.

## Error Details

```
[GUEST] Aborting: line 130, file /src/home/cody/zisk/zksync-os/proof_running_system/src/system/bootloader.rs
[GUEST] line 130, file /src/home/cody/zisk/zksync-os/proof_running_system/src/system/bootloader.rs: global alloc not allowed
Emu::run_fast() finished with error at step=653585839 pc=0x800c6b78
```

## Root Cause

The ZisK build is compiled without the `global-alloc` feature. When this feature is disabled, the `OptionalGlobalAllocator` in `proof_running_system/src/system/bootloader.rs` panics on any allocation attempt:

```rust
#[cfg(not(feature = "global-alloc"))]
unsafe impl GlobalAlloc for OptionalGlobalAllocator {
    unsafe fn alloc(&self, _layout: core::alloc::Layout) -> *mut u8 {
        panic!("global alloc not allowed");  // Line 130
    }
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: core::alloc::Layout) {
        panic!("global alloc not allowed");
    }
}
```

## Context

The panic occurs during execution of a BLS precompile call (address 0x0a - BLS G1 multiply). The call stack shows:
- External call to ecrecover (0x01) - succeeds
- External call to BLS G1 mul (0x0a) - triggers global allocation

## Build Configuration

The ZisK binary is built via `setup.sh` with features:
```bash
FEATURES="proving,print_debug_info,delegation" ./build.sh --machine zisk
```

Note: `global-alloc` is NOT included in the feature list.

## Comparison with Airbender

The Airbender build also does not include `global-alloc`:
```bash
FEATURES="proving,unlimited_native,disable_system_contracts,prevrandao,evm_refunds,print_debug_info,cycle_marker" ./build.sh --machine airbender
```

However, Airbender completes successfully. This suggests:
1. The code path that triggers global allocation may be specific to 64-bit (ZisK) vs 32-bit (Airbender)
2. Or the ZisK emulator handles memory differently, triggering allocation paths not hit by Airbender

## Potential Solutions

1. **Enable `global-alloc` feature for ZisK build**: Add `global-alloc` to the features in `setup.sh`
   ```bash
   FEATURES="proving,print_debug_info,delegation,global-alloc" ./build.sh --machine zisk
   ```

2. **Investigate the allocation source**: Determine what code is triggering global allocation during BLS precompile execution and refactor to use the bootloader allocator instead.

3. **Check 64-bit specific code paths**: The issue may be in architecture-specific code that behaves differently on RV64 vs RV32.

## Reproduction

```bash
cd /home/cody/zisk
SETUP=0 ./bench-zisk-cycles.sh 24198369
```

Check `/tmp/zisk-bench.log` for full output.

## Related Files

- `proof_running_system/src/system/bootloader.rs` - Contains the panicking allocator
- `setup.sh` - ZisK build script
- `bench-zisk-cycles.sh` - ZisK benchmark script
