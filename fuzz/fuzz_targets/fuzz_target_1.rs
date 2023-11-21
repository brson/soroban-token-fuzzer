#![no_main]
#![allow(unused)]

use crate::arbitrary::Unstructured;
use libfuzzer_sys::{fuzz_target, Corpus};
use num_bigint::BigInt;
use soroban_ledger_snapshot::LedgerSnapshot;
use soroban_sdk::testutils::{
    arbitrary::{arbitrary, SorobanArbitrary},
    Address as _, AuthorizedFunction, AuthorizedInvocation, Ledger, LedgerInfo, Logs, MockAuth,
    MockAuthInvoke,
};
use soroban_sdk::{
    token::{Client, StellarAssetClient},
    Address, Bytes, Env, FromVal, IntoVal, String,
};
use std::vec::Vec as RustVec;

pub(crate) const DAY_IN_LEDGERS: u32 = 17280;
pub(crate) const INSTANCE_BUMP_AMOUNT: u32 = 7 * DAY_IN_LEDGERS;
pub(crate) const INSTANCE_LIFETIME_THRESHOLD: u32 = INSTANCE_BUMP_AMOUNT - DAY_IN_LEDGERS;
pub(crate) const NUMBER_OF_ADDRESSES: usize = 3;

#[derive(Clone, Debug, arbitrary::Arbitrary)]
pub struct Input {
    addresses: [<Address as SorobanArbitrary>::Prototype; NUMBER_OF_ADDRESSES],
    commands: RustVec<Command>,
}

#[derive(Clone, Debug, arbitrary::Arbitrary)]
pub enum Command {
    Mint(MintInput),
    Approve(ApproveInput),
    TransferFrom(TransferFromInput),
    Transfer(TransferInput),
    BurnFrom(BurnFromInput),
    Burn(BurnInput),
    AdvanceLedgers(AdvanceLedgersInput),
}

#[derive(Clone, Debug, arbitrary::Arbitrary)]
pub struct MintInput {
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=i128::MAX))]
    amount: i128,
}

#[derive(Clone, Debug, arbitrary::Arbitrary)]
pub struct ApproveInput {
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=i128::MAX))]
    allowance_amount: i128,
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=DAY_IN_LEDGERS * 30))]
    expiration_ledger: u32,
}

#[derive(Clone, Debug, arbitrary::Arbitrary)]
pub struct TransferFromInput {
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=i128::MAX))]
    amount: i128,
}

#[derive(Clone, Debug, arbitrary::Arbitrary)]
pub struct TransferInput {
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=i128::MAX))]
    amount: i128,
}

#[derive(Clone, Debug, arbitrary::Arbitrary)]
pub struct BurnFromInput {
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=i128::MAX))]
    amount: i128,
}

#[derive(Clone, Debug, arbitrary::Arbitrary)]
pub struct BurnInput {
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=i128::MAX))]
    amount: i128,
}

#[derive(Clone, Debug, arbitrary::Arbitrary)]
pub struct AdvanceLedgersInput {
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(1..=DAY_IN_LEDGERS))]
    ledgers: u32,
}

fuzz_target!(|input: Input| -> Corpus {
    let mut env = create_env();

    let token_contract_id_bytes: RustVec<u8>;

    // Do initial setup, including registering the contract.
    {
        let admin = Address::from_val(&env, &input.addresses[0]);
        let spender = Address::from_val(&env, &input.addresses[1]);
        let to = Address::from_val(&env, &input.addresses[2]);

        if !require_unique_addresses(&[&admin, &spender, &to]) {
            return Corpus::Reject;
        }

        if !require_contract_addresses(&[&admin, &spender, &to]) {
            return Corpus::Reject;
        }

        let token_contract_id_string = env
            .register_stellar_asset_contract(admin.clone())
            .to_string();
        let mut token_contract_id_buf = vec![0; token_contract_id_string.len() as usize];
        token_contract_id_string.copy_into_slice(&mut token_contract_id_buf);
        token_contract_id_bytes = token_contract_id_buf
    }

    let mut contract_state = ContractState::init();

    for command in input.commands {
        // The Env may be different for each step, so we need to reconstruct
        // everything that depends on it.
        env.mock_all_auths();
        let admin = Address::from_val(&env, &input.addresses[0]);
        let spender = Address::from_val(&env, &input.addresses[1]);
        let to = Address::from_val(&env, &input.addresses[2]);

        let token_contract_id =
            Address::from_string_bytes(&Bytes::from_slice(&env, &token_contract_id_bytes));
        let admin_client = StellarAssetClient::new(&env, &token_contract_id);
        let token_client = Client::new(&env, &token_contract_id);

        let current_state = CurrentState::from(admin.clone(), spender.clone(), to.clone());
        
        println!("------- command: {:#?}\n--------", command);
        match command {
            Command::Mint(input) => {
                let r = admin_client.try_mint(&admin, &input.amount);
                if r.is_ok() {
                    if r.unwrap().is_ok() {
                        contract_state.admin_balance = contract_state
                            .admin_balance
                            .checked_add(input.amount)
                            .expect("Overflow");

                        contract_state.sum_of_mints = contract_state.sum_of_mints
                            .checked_add(&BigInt::from(input.amount))
                            .expect("Overflow");
                    }
                }
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
                    }
                }
            }
            Command::TransferFrom(input) => {
                let r = token_client.try_transfer_from(&spender, &admin, &to, &input.amount);

                if r.is_ok() {
                    if r.unwrap().is_ok() {
                        contract_state.admin_balance = contract_state
                            .admin_balance
                            .checked_sub(input.amount)
                            .expect("Overflow");

                        contract_state.allowance = contract_state
                            .allowance
                            .checked_sub(input.amount)
                            .expect("Overflow");
                    }
                }
            }
            Command::Transfer(input) => {
                let r = token_client.try_transfer(&admin, &to, &input.amount);

                if r.is_ok() {
                    if r.unwrap().is_ok() {
                        contract_state.admin_balance = contract_state
                            .admin_balance
                            .checked_sub(input.amount)
                            .expect("Overflow");
                    }
                }

                let r = token_client.try_transfer(&to, &admin, &input.amount);

                if r.is_ok() {
                    if r.unwrap().is_ok() {
                        contract_state.admin_balance = contract_state
                            .admin_balance
                            .checked_add(input.amount)
                            .expect("Overflow");
                    }
                }
            }
            Command::BurnFrom(input) => {
                let r = token_client.try_burn_from(&spender, &admin, &input.amount);
                if r.is_ok() {
                    if r.unwrap().is_ok() {
                        contract_state.admin_balance = contract_state
                            .admin_balance
                            .checked_sub(input.amount)
                            .expect("Overflow");

                        contract_state.sum_of_burns = contract_state
                            .sum_of_burns
                            .checked_add(&BigInt::from(input.amount))
                            .expect("Overflow");

                        contract_state.allowance = contract_state
                            .allowance
                            .checked_sub(input.amount)
                            .expect("Overflow");
                    }
                }
            }
            Command::Burn(input) => {
                let r = token_client.try_burn(&admin, &input.amount);
                if r.is_ok() {
                    if r.unwrap().is_ok() {
                        contract_state.admin_balance = contract_state
                            .admin_balance
                            .checked_sub(input.amount)
                            .expect("Overflow");

                        contract_state.sum_of_burns = contract_state
                            .sum_of_burns
                            .checked_add(&BigInt::from(input.amount))
                            .expect("Overflow");
                    }
                }
            }
            Command::AdvanceLedgers(input) => {
                let to_ledger = env
                    .ledger()
                    .sequence()
                    .checked_add(input.ledgers)
                    .expect("end of time");

                env = advance_time_to(env, &token_contract_id_bytes, to_ledger);
                // NB: This env is reconstructed and all previous env-based objects are invalid

                // update saved allowance number after advance ledgers
                contract_state.allowance = token_client.allowance(&current_state.admin, &current_state.spender);
            }
        }

        assert_state(&token_client, &contract_state, &current_state);
        env.budget().reset_unlimited();
    }

    Corpus::Keep
});

pub struct ContractState {
    admin_balance: i128,
    allowance: i128,
    sum_of_mints: BigInt,
    sum_of_burns: BigInt,
}

impl ContractState {
    fn init() -> Self {
        ContractState {
            admin_balance: 0,
            allowance: 0,
            sum_of_mints: BigInt::default(),
            sum_of_burns: BigInt::default(),
        }
    }
}

struct CurrentState {
    admin: Address,
    spender: Address,
    to: Address,
}

impl CurrentState {
    fn from(
        admin: Address,
        spender: Address,
        to: Address,
    ) -> Self {
        CurrentState {
            admin,
            spender,
            to,
        }
    }
}

fn assert_state(token_client: &Client, contract: &ContractState, current: &CurrentState) {
    assert_eq!(
        contract.admin_balance,
        token_client.balance(&current.admin)
    );

    assert_eq!(
        contract.allowance,
        token_client.allowance(&current.admin, &current.spender)
    );

    let sum_of_balances_0 = contract.sum_of_mints.checked_sub(&contract.sum_of_burns).expect("Overflow");
    let sum_of_balances_1 = BigInt::from(token_client.balance(&current.admin))
        .checked_add(&BigInt::from(token_client.balance(&current.spender)))
        .expect("Overflow")
        .checked_add(&BigInt::from(token_client.balance(&current.to)))
        .expect("Overflow");
    
    assert_eq!(
        sum_of_balances_0,
        sum_of_balances_1,
    );
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

fn require_contract_addresses(addrs: &[&Address]) -> bool {
    use stellar_strkey::*;
    for addr in addrs {
        let addr_string = addr.to_string();
        let mut addr_buf = vec![0; addr_string.len() as usize];
        addr_string.copy_into_slice(&mut addr_buf);
        let addr_string = std::str::from_utf8(&addr_buf).unwrap();
        let strkey = Strkey::from_string(&addr_string).unwrap();
        match strkey {
            Strkey::Contract(_) => {}
            _ => {
                return false;
            }
        }
    }
    true
}

fn create_env() -> Env {
    Env::default()
}

fn advance_env(prev_env: Env, ledgers: u32) -> Env {
    use soroban_sdk::testutils::Ledger as _;

    let secs_per_ledger = {
        let secs_per_day = 60 * 60 * 24;
        let ledgers_per_day = DAY_IN_LEDGERS as u64;
        secs_per_day / ledgers_per_day
    };
    let ledger_time = secs_per_ledger
        .checked_mul(ledgers as u64)
        .expect("end of time");

    let mut env = prev_env.clone();
    env.ledger().with_mut(|ledger| {
        ledger.sequence_number = ledger
            .sequence_number
            .checked_add(ledgers)
            .expect("end of time");
        ledger.timestamp = ledger
            .timestamp
            .checked_add(ledger_time)
            .expect("end of time");
    });

    env

    /*
    let mut snapshot = prev_env.to_snapshot();
    snapshot.sequence_number = snapshot.sequence_number.checked_add(ledgers).expect("end of time");
    snapshot.timestamp = snapshot.timestamp.checked_add(ledger_time).expect("end of time");

    let env = Env::from_snapshot(snapshot);

    env*/
}

/// Advance time, but do it in increments, periodically pinging the contract to
/// keep it alive.
fn advance_time_to(mut env: Env, token_contract_id_bytes: &[u8], to_ledger: u32) -> Env {
    loop {
        let curr_ledger = env.ledger().get().sequence_number;
        assert!(curr_ledger < to_ledger);

        let next_ledger = curr_ledger
            .checked_add(DAY_IN_LEDGERS)
            .expect("end of time");
        let next_ledger = next_ledger.min(to_ledger);

        let advance_ledgers = next_ledger - curr_ledger;

        env = advance_env(env, advance_ledgers);

        if next_ledger == to_ledger {
            break;
        } else {
            // Keep the contract alive
            let token_contract_id =
                Address::from_string_bytes(&Bytes::from_slice(&env, &token_contract_id_bytes));
            let token_client = Client::new(&env, &token_contract_id);
            let _ = token_client.try_allowance(&Address::generate(&env), &Address::generate(&env));
        }
    }

    env
}
