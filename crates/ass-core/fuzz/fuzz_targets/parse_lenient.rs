#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    let _ = ass_core::SubtitleDocument::parse_with_recovery(data);
});
