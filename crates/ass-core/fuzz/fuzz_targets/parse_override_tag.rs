#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    ass_core::override_tag::parse_tags(data);
});
