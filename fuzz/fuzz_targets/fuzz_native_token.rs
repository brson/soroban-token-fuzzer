#![no_main]

use libfuzzer_sys::{fuzz_target, Corpus};
use soroban_token_fuzzer::*;

fuzz_target!(|input: Input| -> Corpus {
    let config = Config::native();
    fuzz_token(config, input)
});
