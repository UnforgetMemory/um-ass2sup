#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Exercise the PGS SUP decoder with arbitrary bytes.
    //
    // decode_sup parses PGS binary data into display sets. The decoder
    // should:
    //  - Return Err for malformed input (NO PANIC)
    //  - Return Ok for valid input
    //  - Never hang, never OOM, never use-after-free
    let _ = pgs_encoder::decoder::decode_sup(data);
});
