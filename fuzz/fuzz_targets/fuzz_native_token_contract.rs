#![no_main]
#![allow(unused)]

use crate::arbitrary::Unstructured;
use libfuzzer_sys::fuzz_target;
use soroban_sdk::arbitrary::{arbitrary, fuzz_catch_panic, SorobanArbitrary};
use soroban_sdk::testutils::{
    Address as _, AuthorizedFunction, AuthorizedInvocation, Ledger, Logs, MockAuth, MockAuthInvoke,
};
use soroban_sdk::{
    token::{Client, StellarAssetClient},
    Address, Env, FromVal, IntoVal,
};

mod testcontract {
    soroban_sdk::contractimport!(
        file = "../target/wasm32-unknown-unknown/release/fuzzing_native_token.wasm"
    );
}

#[derive(Clone, Debug, arbitrary::Arbitrary)]
pub struct TestInput {
    admin: <Address as SorobanArbitrary>::Prototype,
    spender: <Address as SorobanArbitrary>::Prototype,
    to: <Address as SorobanArbitrary>::Prototype,
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=i128::MAX))]
    mint_amount: i128,
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=i128::MAX))]
    allowance_amount: i128,
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=i128::MAX))]
    transfer_amount: i128,
    expiration_ledger: u32,
}

fuzz_target!(|input: TestInput| {
    // todo tests:
    // - more general token contract state assertions
    // - approve allowance with non_zero amount with an expired ledger number and then transfer
    // - approve allowance amount that is larger than admin/from's balance, and then transfer?
    // - if allowance set to 0 if exipired
    // - use transfer_from 2nd time for transfer the leftover amount (allowance_amount - transfer_amount)
    // - error if transfer_from transfers the amount is more than allowance (allowance < admin's balance)
    // - call transfer_from with a new user instead of approved spender
    // - run the same transfer twice
    // - more token_client methods:
    //   - burn
    //   - burn_from
    // - more admin-client methods:
    //   - set_admin
    //   - set_authorized
    //   - clawback

    let env = Env::default();

    // input value
    let admin = Address::from_val(&env, &input.admin);
    let spender = Address::from_val(&env, &input.spender);
    let to = Address::from_val(&env, &input.to);
    let mint_amount = input.mint_amount;
    let allowance_amount = input.allowance_amount;
    let transfer_amount = input.transfer_amount;
    let expiration_ledger = input.expiration_ledger;

    // todo: arbitrary generates possibly the same addresses.
    if admin.eq(&to) || admin.eq(&spender) || spender.eq(&to) {
        return;
    }

    let token_contract_id = env.register_stellar_asset_contract(admin.clone());
    let admin_client = StellarAssetClient::new(&env, &token_contract_id);
    let token_client = Client::new(&env, &token_contract_id);

    assert_eq!(0, token_client.balance(&admin));
    assert_eq!(0, token_client.balance(&spender));
    assert_eq!(0, token_client.balance(&to));

    // mint
    {
        let panic_r = fuzz_catch_panic(|| {
            let _call_r = admin_client.mock_all_auths().try_mint(&admin, &mint_amount);
        });

        if panic_r.is_err() {
            if !env.logs().all().is_empty() {
                env.logs().print();
            }
            panic!("host panicked: {panic_r:?}");
        }

        assert_eq!(mint_amount, token_client.balance(&admin));
        assert_eq!(mint_amount, token_client.spendable_balance(&admin));
        assert_eq!(0, token_client.balance(&spender));
        assert_eq!(0, token_client.balance(&to));
    }

    // approve allowance and transfer_from
    {
        // approve allowance
        let max_entry_expiration = env.ledger().get().max_entry_expiration;
        let current_ledger_number = env.ledger().sequence();

        if expiration_ledger >= current_ledger_number
            && expiration_ledger <= max_entry_expiration
            && allowance_amount != 0
        // todo: test allowance_amount greater than from_balance
        //            && allowance_amount <= mint_amount
        {
            let panic_r = fuzz_catch_panic(|| {
                let _call_r = token_client.mock_all_auths().try_approve(
                    &admin,
                    &spender,
                    &allowance_amount,
                    &expiration_ledger,
                );
            });

            if panic_r.is_err() {
                if !env.logs().all().is_empty() {
                    env.logs().print();
                }
                panic!("host panicked: {panic_r:?}");
            }

            assert_eq!(allowance_amount, token_client.allowance(&admin, &spender));

            // transfer_from
            let panic_r = fuzz_catch_panic(|| {
                token_client.mock_all_auths().try_transfer_from(
                    &spender,
                    &admin,
                    &to,
                    &transfer_amount,
                );
            });

            if panic_r.is_err() {
                if !env.logs().all().is_empty() {
                    env.logs().print();
                }
                panic!("host panicked: {panic_r:?}");
            }

            if transfer_amount > allowance_amount || transfer_amount > mint_amount {
                assert_eq!(allowance_amount, token_client.allowance(&admin, &spender));
                assert_eq!(0, token_client.balance(&to));
                assert_eq!(mint_amount, token_client.balance(&admin));
            } else {
                assert_eq!(transfer_amount, token_client.balance(&to));
                assert_eq!(
                    mint_amount.checked_sub(transfer_amount).unwrap(),
                    token_client.balance(&admin)
                );
                assert_eq!(
                    allowance_amount.checked_sub(transfer_amount).unwrap(),
                    token_client.allowance(&admin, &spender)
                );

                // transfer_from 2nd time
                let admin_current_balance = token_client.balance(&admin);
                let to_current_balance = token_client.balance(&to);
                let current_allowance = token_client.allowance(&admin, &spender);

                let panic_r = fuzz_catch_panic(|| {
                    token_client.mock_all_auths().try_transfer_from(
                        &spender,
                        &admin,
                        &to,
                        &transfer_amount,
                    );
                });

                if panic_r.is_err() {
                    if !env.logs().all().is_empty() {
                        env.logs().print();
                    }
                    panic!("host panicked: {panic_r:?}");
                }

                println!("balance: ");
                println!("----- admin_before: {}", admin_current_balance);
                println!("------ admin_after: {}", token_client.balance(&admin));
                println!("-------- to_before: {}", to_current_balance);
                println!("--------- to_after: {}", token_client.balance(&to));
                println!("- allowance amount: {}", allowance_amount);
                println!("- allowance before: {}", current_allowance);
                println!(
                    "-- allowance after: {}",
                    token_client.allowance(&admin, &spender)
                );

                if transfer_amount <= admin_current_balance && transfer_amount <= current_allowance
                {
                    assert_eq!(
                        to_current_balance.checked_add(transfer_amount).unwrap(),
                        token_client.balance(&to),
                    );
                    assert_eq!(
                        admin_current_balance.checked_sub(transfer_amount).unwrap(),
                        token_client.balance(&admin),
                    );
                } else {
                    assert_eq!(to_current_balance, token_client.balance(&to),);
                    assert_eq!(admin_current_balance, token_client.balance(&admin),);
                }
            }

            // advance the ledger past the expiration of the allowance
            /*
            env.ledger().with_mut(|li| li.sequence_number = expiration_ledger);
            let current_ledger_number = env.ledger().sequence();
            assert_eq!(current_ledger_number, expiration_ledger);
            assert_eq!(0, token_client.allowance(&admin, &spender));
             */
        }
    }

    //    todo: wasm contract test
    //    let test_contract_id = env.register_contract_wasm(None, testcontract::WASM);
    //    let test_contract_client = testcontract::Client::new(&env, &test_contract_id);
});
