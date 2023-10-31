#![no_main]
#![allow(unused)]

use crate::arbitrary::Unstructured;
use libfuzzer_sys::{fuzz_target, Corpus};
use num_bigint::BigInt;
use soroban_ledger_snapshot::LedgerSnapshot;
use soroban_sdk::arbitrary::{arbitrary, SorobanArbitrary};
use soroban_sdk::testutils::{
    Address as _, AuthorizedFunction, AuthorizedInvocation, Ledger, LedgerInfo, Logs, MockAuth,
    MockAuthInvoke,
};
use soroban_sdk::{
    token::{Client, StellarAssetClient},
    Address, Env, FromVal, IntoVal, String,
};

pub(crate) const DAY_IN_LEDGERS: u32 = 17280;
pub(crate) const INSTANCE_BUMP_AMOUNT: u32 = 7 * DAY_IN_LEDGERS;
pub(crate) const INSTANCE_LIFETIME_THRESHOLD: u32 = INSTANCE_BUMP_AMOUNT - DAY_IN_LEDGERS;

#[derive(Clone, Debug, arbitrary::Arbitrary)]
pub struct Input {
    addresses: [<Address as SorobanArbitrary>::Prototype; 3],
    commands: Vec<Command>,
}

#[derive(Clone, Debug, arbitrary::Arbitrary)]
pub enum Command {
    Mint(#[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=i128::MAX))] i128),
    Approve(ApproveInput),
    TransferFrom(#[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=i128::MAX))] i128),
    Transfer(#[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=i128::MAX))] i128),
    BurnFrom(#[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=i128::MAX))] i128),
    Burn(#[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=i128::MAX))] i128),
    // todo: adjust the range
    AdvanceTimeToLedger(
        #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=DAY_IN_LEDGERS))] u32,
    ),
}

#[derive(Clone, Debug, arbitrary::Arbitrary)]
pub struct ApproveInput {
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=i128::MAX))]
    allowance_amount: i128,
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=DAY_IN_LEDGERS * 30))]
    expiration_ledger: u32,
}

fuzz_target!(|input: Input| -> Corpus {
    let prev_env = Config::setup();
    let mut env = reset_env(&prev_env);

    let admin = Address::from_val(&env, &input.addresses[0]);
    let spender = Address::from_val(&env, &input.addresses[1]);
    let to = Address::from_val(&env, &input.addresses[2]);

    if !require_unique_addresses(&[&admin, &spender, &to]) {
        return Corpus::Reject;
    }

    let token_contract_id = env.register_stellar_asset_contract(admin.clone());
    let admin_client = StellarAssetClient::new(&env, &token_contract_id);
    let token_client = Client::new(&env, &token_contract_id);

    let mut contract_state = ContractState::init();

    println!("commands: {:#?}", input.commands);
    for command in input.commands {
        env = reset_env(&env);

        match command {
            Command::Mint(amount) => {
                let r = admin_client.try_mint(&admin, &amount);
                if r.is_ok() {
                    if r.unwrap().is_ok() {
                        contract_state.admin_balance = contract_state
                            .admin_balance
                            .checked_add(amount)
                            .expect("Overflow");

                        contract_state.sum_of_mints = contract_state
                            .sum_of_mints
                            .checked_add(&BigInt::from(amount))
                            .expect("Overflow");
                    }
                }
                assert_eq!(contract_state.admin_balance, token_client.balance(&admin));
            }
            Command::Approve(input) => {
                let r = token_client.try_approve(
                    &admin,
                    &spender,
                    &input.allowance_amount,
                    &input.expiration_ledger,
                );

                if r.is_ok() {
                    if r.unwrap().is_ok() {
                        contract_state.allowance = input.allowance_amount;
                        contract_state.expiration_ledger = input.expiration_ledger;
                    }
                }

                assert_eq!(
                    contract_state.allowance,
                    token_client.allowance(&admin, &spender)
                );
            }
            Command::TransferFrom(amount) => {
                let sum_of_balances_before = BigInt::from(token_client.balance(&admin))
                    .checked_add(&BigInt::from(token_client.balance(&to)))
                    .expect("Overflow");

                let r = token_client.try_transfer_from(&spender, &admin, &to, &amount);
                if r.is_ok() {
                    if r.unwrap().is_ok() {
                        contract_state.admin_balance = contract_state
                            .admin_balance
                            .checked_sub(amount)
                            .expect("Overflow");

                        contract_state.allowance = contract_state
                            .allowance
                            .checked_sub(amount)
                            .expect("Overflow");
                    }
                }

                let sum_of_balances_after = BigInt::from(token_client.balance(&admin))
                    .checked_add(&BigInt::from(token_client.balance(&to)))
                    .expect("Overflow");

                assert_eq!(sum_of_balances_before, sum_of_balances_after);
                assert_eq!(contract_state.admin_balance, token_client.balance(&admin));
                assert_eq!(
                    contract_state.allowance,
                    token_client.allowance(&admin, &spender)
                );
            }
            Command::Transfer(amount) => {
                let sum_of_balances_before = BigInt::from(token_client.balance(&admin))
                    .checked_add(&BigInt::from(token_client.balance(&to)))
                    .expect("Overflow");

                let r = token_client.try_transfer(&admin, &to, &amount);
                if r.is_ok() {
                    if r.unwrap().is_ok() {
                        contract_state.admin_balance = contract_state
                            .admin_balance
                            .checked_sub(amount)
                            .expect("Overflow");
                    }
                }

                let sum_of_balances_after = BigInt::from(token_client.balance(&admin))
                    .checked_add(&BigInt::from(token_client.balance(&to)))
                    .expect("Overflow");

                assert_eq!(sum_of_balances_before, sum_of_balances_after);
                assert_eq!(contract_state.admin_balance, token_client.balance(&admin));
            }
            Command::BurnFrom(amount) => {
                let r = token_client.try_burn_from(&spender, &admin, &amount);
                if r.is_ok() {
                    if r.unwrap().is_ok() {
                        contract_state.admin_balance = contract_state
                            .admin_balance
                            .checked_sub(amount)
                            .expect("Overflow");

                        contract_state.sum_of_burns = contract_state
                            .sum_of_burns
                            .checked_add(&BigInt::from(amount))
                            .expect("Overflow");

                        contract_state.allowance = contract_state
                            .allowance
                            .checked_sub(amount)
                            .expect("Overflow");
                    }
                }
                assert_eq!(contract_state.admin_balance, token_client.balance(&admin));
                assert_eq!(
                    contract_state.allowance,
                    token_client.allowance(&admin, &spender)
                );
            }
            Command::Burn(amount) => {
                let r = token_client.try_burn(&admin, &amount);
                if r.is_ok() {
                    if r.unwrap().is_ok() {
                        contract_state.admin_balance = contract_state
                            .admin_balance
                            .checked_sub(amount)
                            .expect("Overflow");

                        contract_state.sum_of_burns = contract_state
                            .sum_of_burns
                            .checked_add(&BigInt::from(amount))
                            .expect("Overflow");
                    }
                }
                assert_eq!(contract_state.admin_balance, token_client.balance(&admin));
            }
            Command::AdvanceTimeToLedger(time) => {
                if time > env.ledger().sequence() {
                    advance_time_to(&env, &token_client, time);
                    println!(
                        "-- ledger after advance_time_to: {}",
                        env.ledger().sequence()
                    );
                    println!(
                        "-- contract_state.expiration_ledger: {}",
                        contract_state.expiration_ledger
                    );
                    println!("-        state allowance: {}", contract_state.allowance);
                    println!(
                        "- token client allowance: {}",
                        token_client.allowance(&admin, &spender)
                    );

                    if env.ledger().sequence() > contract_state.expiration_ledger {
                        contract_state.allowance = 0;
                    }
                }
            }
        }
        env = reset_env(&env);
    }

    Corpus::Keep
});

pub struct ContractState {
    admin_balance: i128,
    allowance: i128,
    expiration_ledger: u32,
    sum_of_mints: BigInt,
    sum_of_burns: BigInt,
}

impl ContractState {
    fn init() -> Self {
        ContractState {
            admin_balance: 0,
            allowance: 0,
            expiration_ledger: 0,
            sum_of_mints: BigInt::default(),
            sum_of_burns: BigInt::default(),
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

fn reset_env(prev_env: &Env) -> Env {
    let mut snapshot = prev_env.to_snapshot();
    snapshot.sequence_number += 1;
    //    snapshot.timestamp = snapshot.timestamp.saturating_add(advance_time);

    let env = Env::from_snapshot(snapshot);
    env.budget().reset_unlimited();

    env
}

fn require_unique_addresses(addrs: &[&Address]) -> bool {
    for addr1 in addrs {
        let count = addrs.iter().filter(|a| a == &addr1).count();
        if count > 1 {
            return false;
        }
    }
    true
}

fn advance_time_to(env: &Env, token_client: &Client, to_ledger: u32) {
    loop {
        let next_ledger = env
            .ledger()
            .get()
            .sequence_number
            .saturating_add(DAY_IN_LEDGERS);
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
