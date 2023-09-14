#![no_main]
#![allow(unused)]

use crate::arbitrary::Unstructured;
use libfuzzer_sys::fuzz_target;
use soroban_sdk::arbitrary::{arbitrary, SorobanArbitrary};
use soroban_sdk::testutils::{
    Address as _, AuthorizedFunction, AuthorizedInvocation, Ledger, Logs, MockAuth, MockAuthInvoke,
};
use soroban_sdk::{
    token::{Client, StellarAssetClient},
    Address, Env, FromVal, IntoVal, String,
};

mod testcontract {
    soroban_sdk::contractimport!(
        file = "../target/wasm32-unknown-unknown/release/fuzzing_native_token.wasm"
    );
}

#[derive(Clone, Debug, arbitrary::Arbitrary)]
pub struct TestInput {
    admin: <Address as SorobanArbitrary>::Prototype,
    authorized_user: <Address as SorobanArbitrary>::Prototype,
    spender: <Address as SorobanArbitrary>::Prototype,
    to_0: <Address as SorobanArbitrary>::Prototype,
    to_1: <Address as SorobanArbitrary>::Prototype,
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=i128::MAX))]
    mint_amount: i128,
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=i128::MAX))]
    allowance_amount: i128,
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=i128::MAX))]
    transfer_amount: i128,
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=i128::MAX))]
    burn_amount: i128,
    expiration_ledger: u32,
}

fuzz_target!(|input: TestInput| {
    let env = Env::default();

    // input value
    let admin = Address::from_val(&env, &input.admin);
    let authorized_user = Address::from_val(&env, &input.authorized_user);
    let spender = Address::from_val(&env, &input.spender);
    let to_0 = Address::from_val(&env, &input.to_0);
    let to_1 = Address::from_val(&env, &input.to_1);
    let mint_amount = input.mint_amount;
    let allowance_amount = input.allowance_amount;
    let transfer_amount = input.transfer_amount;
    let burn_amount = input.burn_amount;
    let expiration_ledger = input.expiration_ledger;

    // todo: arbitrary generates possibly the same addresses.
    if admin.eq(&authorized_user) || admin.eq(&spender) {
        return;
    }

    let token_contract_id = env.register_stellar_asset_contract(admin.clone());
    let admin_client = StellarAssetClient::new(&env, &token_contract_id);
    let token_client = Client::new(&env, &token_contract_id);

    assert_eq!(7, token_client.decimals());

    // todo:
    // - name
    // - symbol

    // mint
    {
        let r = admin_client.mock_all_auths().try_mint(&admin, &mint_amount);
        if r.is_err() {
            assert_eq!(0, token_client.balance(&admin));
        } else {
            assert_eq!(mint_amount, token_client.balance(&admin));
            assert_eq!(mint_amount, token_client.spendable_balance(&admin));
        }
    }

    // approve allowance and transfer_from
    {
        let max_entry_expiration = env.ledger().get().max_entry_expiration;
        let current_ledger_number = env.ledger().sequence();

        // approve allowance
        let r = token_client.mock_all_auths().try_approve(
            &admin,
            &spender,
            &allowance_amount,
            &expiration_ledger,
        );

        if r.is_err() {
            assert_eq!(0, token_client.allowance(&admin, &spender));
        } else {
            assert_eq!(allowance_amount, token_client.allowance(&admin, &spender));
        }

        // transfer_from
        let r = token_client.mock_all_auths().try_transfer_from(
            &spender,
            &admin,
            &to_0,
            &transfer_amount,
        );

        if r.is_err() {
            assert_eq!(0, token_client.balance(&to_0));
            assert_eq!(mint_amount, token_client.balance(&admin));
        } else {
            assert_eq!(transfer_amount, token_client.balance(&to_0));
            assert_eq!(
                mint_amount.checked_sub(transfer_amount).unwrap(),
                token_client.balance(&admin)
            );

            // transfer_from 2nd time
            let admin_pre_balance = token_client.balance(&admin);
            let to_pre_balance = token_client.balance(&to_0);
            let pre_allowance = token_client.allowance(&admin, &spender);

            let r = token_client.mock_all_auths().try_transfer_from(
                &spender,
                &admin,
                &to_0,
                &transfer_amount,
            );

            if r.is_err() {
                assert_eq!(to_pre_balance, token_client.balance(&to_0));
                assert_eq!(admin_pre_balance, token_client.balance(&admin));
            } else {
                assert_eq!(
                    to_pre_balance.checked_add(transfer_amount).unwrap(),
                    token_client.balance(&to_0),
                );
                assert_eq!(
                    admin_pre_balance.checked_sub(transfer_amount).unwrap(),
                    token_client.balance(&admin),
                );
            }
        }

        // transfer_from with unapproved user
        {
            let fake_spender = Address::random(&env);

            let admin_pre_balance = token_client.balance(&admin);
            let receiver_pre_balance = token_client.balance(&fake_spender);
            let spender_pre_allowance = token_client.allowance(&admin, &spender);

            token_client.mock_all_auths().try_transfer_from(
                &fake_spender,
                &admin,
                &fake_spender,
                &transfer_amount,
            );

            assert_eq!(0, token_client.balance(&fake_spender));
            assert_eq!(admin_pre_balance, token_client.balance(&admin));
            assert_eq!(receiver_pre_balance, token_client.balance(&fake_spender));
            assert_eq!(
                spender_pre_allowance,
                token_client.allowance(&admin, &spender)
            );
        }
    }

    // transfer
    {
        let admin_pre_balance = token_client.balance(&admin);
        let to_pre_balance = token_client.balance(&to_0);

        let r = token_client
            .mock_all_auths()
            .try_transfer(&admin, &to_0, &transfer_amount);

        if r.is_err() {
            assert_eq!(admin_pre_balance, token_client.balance(&admin));
            assert_eq!(to_pre_balance, token_client.balance(&to_0));
        } else {
            assert_eq!(
                admin_pre_balance.checked_sub(transfer_amount).unwrap(),
                token_client.balance(&admin),
            );
            assert_eq!(
                to_pre_balance.checked_add(transfer_amount).unwrap(),
                token_client.balance(&to_0),
            );
        }
    }

    // burn_from
    {
        // approve allowance
        token_client.mock_all_auths().try_approve(
            &admin,
            &spender,
            &allowance_amount,
            &expiration_ledger,
        );

        let admin_pre_balance = token_client.balance(&admin);
        let pre_allowance = token_client.allowance(&admin, &spender);
        let r = token_client
            .mock_all_auths()
            .try_burn_from(&spender, &admin, &burn_amount);

        if r.is_err() {
            assert_eq!(admin_pre_balance, token_client.balance(&admin));
        } else {
            assert_eq!(
                admin_pre_balance.checked_sub(burn_amount).unwrap(),
                token_client.balance(&admin)
            );
            assert_eq!(
                pre_allowance.checked_sub(burn_amount).unwrap(),
                token_client.allowance(&admin, &spender),
            );
        }
    }

    // burn
    {
        let admin_pre_balance = token_client.balance(&admin);

        let r = token_client.mock_all_auths().try_burn(&admin, &burn_amount);

        if r.is_err() {
            assert_eq!(admin_pre_balance, token_client.balance(&admin));
        } else {
            assert_eq!(
                admin_pre_balance.checked_sub(burn_amount).unwrap(),
                token_client.balance(&admin)
            );
        }
    }

    // set_admin
    {
        let admin_before = admin_client.admin();
        let new_admin = Address::random(&env);

        let r = admin_client.mock_all_auths().try_set_admin(&new_admin);
        if r.is_err() {
            assert_eq!(admin_before, admin_client.admin());
        } else {
            assert_eq!(new_admin, admin_client.admin());
        }
    }

    // set_authorized
    {
        admin_client
            .mock_all_auths()
            .try_set_authorized(&authorized_user, &false);
        println!(
            "------------------------------- authorized: {}",
            admin_client.authorized(&authorized_user)
        );
        println!("- admin: {:?}", admin);
        println!("- authorized_user: {:?}", authorized_user);
        //        assert_eq!(false, admin_client.authorized(&authorized_user));

        let r = admin_client
            .mock_all_auths()
            .try_set_authorized(&authorized_user, &true);
        if r.is_err() {
            assert_eq!(false, admin_client.authorized(&authorized_user));
        } else {
            assert_eq!(true, admin_client.authorized(&authorized_user));
        }

        // transfer_from with authorized user
        let admin_pre_balance = token_client.balance(&admin);

        let r = token_client.mock_all_auths().try_transfer_from(
            &authorized_user,
            &admin,
            &to_1,
            &transfer_amount,
        );

        if r.is_err() {
            assert_eq!(admin_pre_balance, token_client.balance(&admin));
            assert_eq!(0, token_client.balance(&to_1));
        } else {
            assert_eq!(transfer_amount, token_client.balance(&to_1));
            assert_eq!(
                admin_pre_balance.checked_sub(transfer_amount).unwrap(),
                token_client.balance(&admin),
            );
        }

        let r = admin_client
            .mock_all_auths()
            .try_set_authorized(&authorized_user, &false);
        if r.is_err() {
            assert_eq!(true, admin_client.authorized(&authorized_user));
        } else {
            assert_eq!(false, admin_client.authorized(&authorized_user));
        }

        // transfer_from after setting authorized `false`
        let admin_pre_balance = token_client.balance(&admin);
        let to_pre_balance = token_client.balance(&to_1);

        let r = token_client.mock_all_auths().try_transfer_from(
            &authorized_user,
            &admin,
            &to_1,
            &transfer_amount,
        );

        assert!(r.is_err());

        assert_eq!(admin_pre_balance, token_client.balance(&admin));
        assert_eq!(to_pre_balance, token_client.balance(&to_1));
    }

    // todo: advance the ledger past the expiration of the allowance
    // thread '<unnamed>' panicked at rs-soroban-env/soroban-env-host/src/host.rs:1039:9:
    // HostError: Error(Storage, InternalError)
    // 0: [Diagnostic Event] topics:[error, Error(Storage, InternalError)], data:"escalating error to panic"
    // 1: [Diagnostic Event] topics:[error, Error(Storage, InternalError)], data:["contract try_call failed", allowance, [Address(Contract(fbefafafafafaf50af5050505050501c501c1c1c1c1c50505052505058500a50)), Address(Contract(5050505058500a50505050505050505250500a50505050505050505050505050))]]
    // 2: [Failed Diagnostic Event (not emitted)] contract:405bf28e12fa3d9188de103aa043dfb5847208759932aa39dd2dc4f2000cbc69, topics:[error, Error(Storage, InternalError)], data:["accessing expired entry", 120960, 5263439]
    /*
    {
        env.ledger().with_mut(|li| li.sequence_number = expiration_ledger.checked_add(1).unwrap());

        println!("******************************* after advancing ledger *******************************");
        let r = token_client.try_allowance(&admin, &spender);
        if r.is_err() {
            println!("error ----------------------------");
        } else {
            println!("---------------------- allowance_amount: {:?}", r.unwrap());
        }
    }
    */
});
