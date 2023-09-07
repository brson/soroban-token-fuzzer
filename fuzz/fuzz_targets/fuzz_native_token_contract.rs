#![no_main]

use libfuzzer_sys::fuzz_target;
use soroban_sdk::arbitrary::arbitrary;
use soroban_sdk::arbitrary::fuzz_catch_panic;
use soroban_sdk::arbitrary::SorobanArbitrary;
use soroban_sdk::testutils::Logs;
use soroban_sdk::token;
use soroban_sdk::{Address, Bytes, Vec};
use soroban_sdk::{Env, FromVal, IntoVal, Map, String, Symbol, Val};
use testcontract::*;

mod testcontract {
    soroban_sdk::contractimport!(
        file = "../target/wasm32-unknown-unknown/release/fuzzing_native_token.wasm"
    );
}

fuzz_target!(|input: <Vec<Address> as SorobanArbitrary>::Prototype| {
    let env = Env::default();

    let addresses: Vec<Address> = input.into_val(&env);
    if addresses.len() < 3 {
        return;
    }

    let admin = addresses.get(0).unwrap();
    let from = addresses.get(1).unwrap();
    let spender = addresses.get(2).unwrap();

    let token_contract_id = env.register_stellar_asset_contract(admin);
    //    let contract_id = env.register_contract(None, TestContract);
    //    let client = TestContractClient::new(&env, &contract_id);

    let contract_id = env.register_contract_wasm(None, testcontract::WASM);

    let client = testcontract::Client::new(&env, &contract_id);
    client.init(&token_contract_id);

    let token_client = token::Client::new(&env, &client.get_token());
    assert_eq!(token_client.decimals(), 7);

    /*
    // Returning an error is ok; panicking is not.
    let panic_r = fuzz_catch_panic(|| {
        let _call_r = client.try_run(&fuzz_instruction);
    });

    if panic_r.is_err() {
        if !env.logs().all().is_empty() {
            env.logs().print();
        }
        panic!("host panicked: {panic_r:?}");
    }*/
});
