#![no_main]
#![allow(unused)]

use crate::arbitrary::Unstructured;
use libfuzzer_sys::{fuzz_target, Corpus};
use soroban_sdk::arbitrary::{arbitrary, SorobanArbitrary};
use soroban_sdk::testutils::{
    Address as _, AuthorizedFunction, AuthorizedInvocation, Ledger, Logs, MockAuth, MockAuthInvoke, LedgerInfo,
};
use soroban_sdk::{
    token::{Client, StellarAssetClient},
    Address, Env, FromVal, IntoVal, String, 
};
use soroban_ledger_snapshot::LedgerSnapshot;
    
pub(crate) const DAY_IN_LEDGERS: u32 = 17280;
pub(crate) const INSTANCE_BUMP_AMOUNT: u32 = 7 * DAY_IN_LEDGERS;
pub(crate) const INSTANCE_LIFETIME_THRESHOLD: u32 = INSTANCE_BUMP_AMOUNT - DAY_IN_LEDGERS;


#[derive(Clone, Debug, arbitrary::Arbitrary)]
pub struct Input {
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
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=DAY_IN_LEDGERS * 30))]
    expiration_ledger: u32,
}

fuzz_target!(|input: Input| -> Corpus {
    let mut prev_env = Config::setup();

    let advance_time = 10_000;
    let curr_env = {
        let mut snapshot = prev_env.to_snapshot();
        snapshot.sequence_number += 1;
        snapshot.timestamp = snapshot.timestamp.saturating_add(advance_time);
        let env = Env::from_snapshot(snapshot);
        env.budget().reset_unlimited();
        env
    };

    let token_admin = Address::from_val(&curr_env, &input.admin);

    let authorized_user = Address::from_val(&curr_env, &input.authorized_user);
    let spender = Address::from_val(&curr_env, &input.spender);
    let to_0 = Address::from_val(&curr_env, &input.to_0);
    let to_1 = Address::from_val(&curr_env, &input.to_1);

    if !require_unique_addresses(&[
        &token_admin, &authorized_user, &spender, &to_0, &to_1,
    ]) {
        return Corpus::Reject;
    }        

    
    let token_contract_id = curr_env.register_stellar_asset_contract(token_admin.clone());
    let admin_client = StellarAssetClient::new(&curr_env, &token_contract_id);
    let token_client = Client::new(&curr_env, &token_contract_id);



    Corpus::Keep
});

fn require_unique_addresses(addrs: &[&Address]) -> bool {
    for addr1 in addrs {
        let count = addrs.iter().filter(|a| a == &addr1).count();
        if count > 1 {
            return false;
        }
    }
    true
}

fn advance_time_to(
    env: &Env,
    token_client: &Client,
    to_ledger: u32,
) {
    loop {
        let next_ledger = env.ledger().get().sequence_number.saturating_add(DAY_IN_LEDGERS);
        let next_ledger = next_ledger.min(to_ledger);
        env.ledger().with_mut(|li| {
            li.sequence_number = next_ledger;
        });
        env.budget().reset_default();
        // Keep the contract alive
        let _ = token_client.try_allowance(&Address::random(env), &Address::random(env));
        if next_ledger == to_ledger {
            break;
        }
    }
}

struct Config {}
    
impl Config {
    fn setup() -> Env {
        let snapshot = {
            let init_ledger = LedgerInfo {
                protocol_version: 1,
                sequence_number: 10,
                timestamp: 12345,
                network_id: Default::default(),
                base_reserve: 10,
                min_temp_entry_ttl: u32::MAX,
                min_persistent_entry_ttl: u32::MAX,
                max_entry_ttl: u32::MAX,
            };

            LedgerSnapshot::from(init_ledger, None)
        };

        let env = Env::from_snapshot(snapshot);
        env.mock_all_auths();

        env
    }
}
