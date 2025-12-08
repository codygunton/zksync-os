overflows 

how do compile code to execute in the os?


looks sketchy:

    let mut skip_balance_check_for_sender_and_coinbase = hardfork_version != "Cancun";

    if test_definition.transaction.max_fee_per_blob_gas.is_some() {
        // We don't support blobs yet
        skip_balance_check_for_sender_and_coinbase = true;
    }

they don't have verification yet on ethproofs or...

makes me nervous that they skip some code in some cases

eth_runner command in proving_ethereum notes doesn't work

fuzzer job never succeeds, evm job does though

how do rust assertions actually become constraints?

(CPU 14 minutes)
=== Total proving time: 7.795410318s for 2 circuits - avg: 3.897705209s
Producing proofs for delegation circuit type 1991, 1 proofs in total
Witness generation for delegation circuit type 1991 took 221.03398ms
Lookup preprocessing took 2.305971ms
Generation of stage 2 trace took 159.076758ms
Proving for delegation circuit type 1991 took 4.712287319s
=== Total delegation proving time: 4.933412757s for 1 circuits - avg: 4.933412777s
Created 0 basic proofs, 2 reduced proofs, 0 reduced (log23) proofs and 1 delegation proofs. Final proofs: 0
Stopping 1st recursion layer.
Writing proofs to "/tmp/output"


(GPU 37s)
[2025-12-04T02:40:35.385Z INFO ] BATCH[0] PROVER committed to memory and produced proofs for binary with key 1 in 0.489s
**** proofs generated in 0.489s ****
Created 0 basic proofs, 2 reduced proofs, 0 reduced (log23) proofs and 1 delegation proofs. Final proofs: 0
Stopping 1st recursion layer.
Writing proofs to "/tmp/output"
**** Total time on production critical path 25.126s ****



