use crate::input::*;
use crate::config::*;
use crate::DAY_IN_LEDGERS;
use ed25519_dalek::SigningKey;
use itertools::Itertools;
use libfuzzer_sys::Corpus;
use num_bigint::BigInt;
use soroban_sdk::testutils::{Address as _, Events, Ledger};
use soroban_sdk::xdr::{
    AccountEntry, AccountEntryExt, AccountId, AlphaNum4, AssetCode4, Hash, LedgerEntry,
    LedgerEntryData, LedgerEntryExt, LedgerKey, LedgerKeyAccount, LedgerKeyTrustLine, PublicKey,
    ScAddress, ScErrorCode, ScErrorType, SequenceNumber, Signer, SignerKey, Thresholds,
    TrustLineAsset, TrustLineEntry, TrustLineEntryExt, TrustLineFlags, Uint256,
};
use soroban_sdk::{
    token::Client, Address, Bytes, Env, Error, InvokeError, String, TryFromVal, Val,
};
use std::collections::BTreeMap;
use std::rc::Rc;
use std::vec::Vec as RustVec;

// Don't know where this number comes from.
const MAX_LEDGERS_TO_ADVANCE: u32 = 4095;

type TokenContractResult =
    Result<Result<(), <() as TryFromVal<Env, Val>>::Error>, Result<Error, InvokeError>>;

pub fn fuzz_token(config: Config, input: Input) -> Corpus {
    if input.commands.is_empty() {
        return Corpus::Reject;
    }

    //eprintln!("input: {input:#?}");
    let mut env = Env::default();

    let token_contract_id_bytes: RustVec<u8>;

    {
        // Do initial setup, including registering the contract.
        let address_pairs =
            generate_addresses_from_seed(&env, input.address_seed, &input.address_types);
        address_pairs.iter().for_each(|(addr, seed)| {
            let sc_addr = ScAddress::try_from(addr).unwrap();
            match sc_addr {
                ScAddress::Account(account_id) => {
                    let signing_key = SigningKey::from_bytes(seed);
                    create_default_account(&env, &account_id, vec![(&signing_key, 100)]);
                    create_default_trustline(&env, &account_id);
                }
                ScAddress::Contract(_) => {}
            }
        });

        let admin = &address_pairs[0].0;

        let token_contract_id = config.register_contract_init(&env, admin);
        token_contract_id_bytes = address_to_bytes(&token_contract_id);
    }

    let mut contract_state = ContractState::init();
    let mut current_state = CurrentState::new(
        &env,
        &config,
        &token_contract_id_bytes,
        input.address_seed,
        &input.address_types,
    );

    let mut results: Vec<(&'static str, bool)> = vec![];

    let mut log_result = |name, r: &Result<_, _>| {
        results.push((name, r.is_ok()));
    };

    for command in input.commands {
        // The Env may be different for each step, so we need to reconstruct
        // everything that depends on it.
        env.mock_all_auths();
        env.budget().reset_unlimited();

        let admin_client = &current_state.admin_client;
        let token_client = &current_state.token_client;
        let accounts = &current_state.accounts;

        contract_state.name = string_to_bytes(token_client.name());
        contract_state.symbol = string_to_bytes(token_client.symbol());
        contract_state.decimals = token_client.decimals();

        // println!("------- command: {:#?}", command);
        match command {
            Command::Mint(input) => {
                let r = admin_client.try_mint(&accounts[input.to_account_index], &input.amount);

                log_result("mint", &r);

                if input.amount < 0 {
                    assert!(r.is_err());
                }

                verify_token_contract_result(&env, &r);

                if let Ok(r) = r {
                    let _r = r.expect("ok");

                    contract_state.add_balance(&accounts[input.to_account_index], input.amount);

                    contract_state.sum_of_mints =
                        contract_state.sum_of_mints + BigInt::from(input.amount);
                }
            }
            Command::Approve(input) => {
                let r = token_client.try_approve(
                    &accounts[input.from_account_index],
                    &accounts[input.spender_account_index],
                    &input.amount,
                    &input.expiration_ledger,
                );

                log_result("approve", &r);

                if input.amount < 0 {
                    assert!(r.is_err());
                }

                verify_token_contract_result(&env, &r);

                if let Ok(r) = r {
                    let _r = r.expect("ok");

                    contract_state.set_allowance(
                        &accounts[input.from_account_index],
                        &accounts[input.spender_account_index],
                        input.amount,
                    );
                }
            }
            Command::TransferFrom(input) => {
                let r = token_client.try_transfer_from(
                    &accounts[input.spender_account_index],
                    &accounts[input.from_account_index],
                    &accounts[input.to_account_index],
                    &input.amount,
                );

                log_result("transfer_from", &r);

                if input.amount < 0 {
                    assert!(r.is_err());
                }

                verify_token_contract_result(&env, &r);

                if let Ok(r) = r {
                    let _r = r.expect("ok");

                    contract_state.sub_balance(&accounts[input.from_account_index], input.amount);
                    contract_state.add_balance(&accounts[input.to_account_index], input.amount);

                    contract_state.sub_allowance(
                        &accounts[input.from_account_index],
                        &accounts[input.spender_account_index],
                        input.amount,
                    );
                }
            }
            Command::Transfer(input) => {
                let r = token_client.try_transfer(
                    &accounts[input.from_account_index],
                    &accounts[input.to_account_index],
                    &input.amount,
                );

                log_result("transfer", &r);

                if input.amount < 0 {
                    assert!(r.is_err());
                }

                verify_token_contract_result(&env, &r);

                if let Ok(r) = r {
                    let _r = r.expect("ok");

                    contract_state.sub_balance(&accounts[input.from_account_index], input.amount);
                    contract_state.add_balance(&accounts[input.to_account_index], input.amount);
                }
            }
            Command::BurnFrom(input) => {
                let r = token_client.try_burn_from(
                    &accounts[input.spender_account_index],
                    &accounts[input.from_account_index],
                    &input.amount,
                );

                log_result("burn_from", &r);

                if input.amount < 0 {
                    assert!(r.is_err());
                }

                verify_token_contract_result(&env, &r);

                if let Ok(r) = r {
                    let _r = r.expect("ok");

                    contract_state.sub_balance(&accounts[input.from_account_index], input.amount);

                    contract_state.sub_allowance(
                        &accounts[input.from_account_index],
                        &accounts[input.spender_account_index],
                        input.amount,
                    );

                    contract_state.sum_of_burns =
                        contract_state.sum_of_burns + &BigInt::from(input.amount);
                }
            }
            Command::Burn(input) => {
                let r = token_client.try_burn(&accounts[input.from_account_index], &input.amount);

                log_result("burn", &r);

                if input.amount < 0 {
                    assert!(r.is_err());
                }

                verify_token_contract_result(&env, &r);

                if let Ok(r) = r {
                    let _r = r.expect("ok");

                    contract_state.sub_balance(&accounts[input.from_account_index], input.amount);

                    contract_state.sum_of_burns =
                        contract_state.sum_of_burns + &BigInt::from(input.amount);
                }
            }
            Command::AdvanceLedgers(cmd_input) => {
                env = advance_time_to(&config, env, &token_contract_id_bytes, cmd_input.ledgers);
                // NB: This env is reconstructed and all previous env-based objects are invalid

                current_state = CurrentState::new(
                    &env,
                    &config,
                    &token_contract_id_bytes,
                    input.address_seed,
                    &input.address_types,
                );

                // update saved allowance number after advance ledgers
                // fixme track expiration ledger instead of asking the contract
                {
                    let pairs = current_state
                        .accounts
                        .iter()
                        .cartesian_product(current_state.accounts.iter());
                    for (addr1, addr2) in pairs {
                        contract_state.set_allowance(
                            addr1,
                            addr2,
                            current_state.token_client.allowance(addr1, addr2),
                        );
                    }
                }
            }
        }

        assert_state(&contract_state, &current_state);
    }

    // eprintln!("results: {results:?}");

    Corpus::Keep
}

/// This tracks what we believe is true about the internal contract state.
///
/// We mirror calculations about balances etc that we expect the contract
/// is making, which means we will be wrong if the token implements economics
/// that differ from the example token.
///
/// This kind of state mirroring I would not generally do in a fuzz test
/// but since the token interface is small and it can be used to test that
/// multiple implementations behave in similar ways, I think it is worth
/// the potential maintenance brittleness.
///
/// Since this state is persistent across transactions,
/// it can not store anything containing an `Env`. Instead
/// it can contain accessors that instantiate various contract
/// types from any `Env`.
pub struct ContractState {
    name: RustVec<u8>,
    symbol: RustVec<u8>,
    decimals: u32,
    balances: BTreeMap<RustVec<u8>, i128>,
    allowances: BTreeMap<(RustVec<u8>, RustVec<u8>), i128>, // (from, spender)
    sum_of_mints: BigInt,
    sum_of_burns: BigInt,
}

impl ContractState {
    fn init() -> Self {
        ContractState {
            name: Vec::<u8>::new(),
            symbol: Vec::<u8>::new(),
            decimals: 0,
            balances: BTreeMap::default(),
            allowances: BTreeMap::default(),
            sum_of_mints: BigInt::default(),
            sum_of_burns: BigInt::default(),
        }
    }

    fn get_balance(&self, addr: &Address) -> i128 {
        let addr_bytes = address_to_bytes(addr);
        self.balances.get(&addr_bytes).copied().unwrap_or(0)
    }

    fn sub_balance(&mut self, addr: &Address, amount: i128) {
        let addr_bytes = address_to_bytes(addr);
        let balance = self.get_balance(addr);
        let new_balance = balance.checked_sub(amount).expect("overflow");
        assert!(new_balance >= 0);
        self.balances.insert(addr_bytes, new_balance);
    }

    fn add_balance(&mut self, addr: &Address, amount: i128) {
        let addr_bytes = address_to_bytes(addr);
        let balance = self.get_balance(addr);
        let new_balance = balance.checked_add(amount).expect("overflow");
        assert!(new_balance >= 0);
        self.balances.insert(addr_bytes, new_balance);
    }

    fn set_allowance(&mut self, from: &Address, spender: &Address, amount: i128) {
        assert!(amount >= 0);
        let from_bytes = address_to_bytes(from);
        let spender_bytes = address_to_bytes(spender);
        self.allowances.insert((from_bytes, spender_bytes), amount);
    }

    fn get_allowance(&self, from: &Address, spender: &Address) -> i128 {
        let from_bytes = address_to_bytes(from);
        let spender_bytes = address_to_bytes(spender);
        self.allowances
            .get(&(from_bytes, spender_bytes))
            .copied()
            .unwrap_or(0)
    }

    fn sub_allowance(&mut self, from: &Address, spender: &Address, amount: i128) {
        let allowance = self.get_allowance(from, spender);
        let new_allowance = allowance.checked_sub(amount).expect("overflow");
        assert!(new_allowance >= 0);
        self.set_allowance(from, spender, new_allowance);
    }
}

/// State that dependso on the `Env` and is reconstructed
/// every transaction.
struct CurrentState<'a> {
    accounts: Vec<Address>,
    admin_client: Box<dyn TokenAdminClient<'a> + 'a>,
    token_client: Client<'a>,
}

impl<'a> CurrentState<'a> {
    fn new(
        env: &Env,
        config: &Config,
        token_contract_id_bytes: &[u8],
        address_seed: u64,
        address_types: &[AddressType; NUMBER_OF_ADDRESSES],
    ) -> Self {
        let token_contract_id =
            Address::from_string_bytes(&Bytes::from_slice(env, &token_contract_id_bytes));
        let admin_client = config.new_admin_client(env, &token_contract_id);
        let token_client = Client::new(env, &token_contract_id);

        let address_pairs = generate_addresses_from_seed(env, address_seed, address_types);

        let accounts: RustVec<Address> =
            address_pairs.iter().map(|(addr, _)| addr.clone()).collect();

        CurrentState {
            accounts,
            admin_client,
            token_client,
        }
    }
}

fn assert_state(contract: &ContractState, current: &CurrentState) {
    let token_client = &current.token_client;

    assert!(contract.name.eq(&string_to_bytes(token_client.name())));
    assert!(contract.symbol.eq(&string_to_bytes(token_client.symbol())));
    assert_eq!(contract.decimals, token_client.decimals());

    for addr in &current.accounts {
        assert_eq!(contract.get_balance(addr), token_client.balance(addr));
        assert!(token_client.balance(addr) >= 0)
    }

    let pairs = current
        .accounts
        .iter()
        .cartesian_product(current.accounts.iter());

    for (addr1, addr2) in pairs {
        assert_eq!(
            contract.get_allowance(addr1, addr2),
            token_client.allowance(addr1, addr2),
        );
    }

    let sum_of_balances_0 = &contract.sum_of_mints - &contract.sum_of_burns;
    let sum_of_balances_1 = current
        .accounts
        .iter()
        .map(|a| BigInt::from(token_client.balance(&a)))
        .sum();

    assert_eq!(sum_of_balances_0, sum_of_balances_1);
}

fn string_to_bytes(s: String) -> RustVec<u8> {
    let mut out = vec![0; s.len() as usize];
    s.copy_into_slice(&mut out);

    out
}

/// Produces a new `Env` after advancing some number of ledgers
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

    // We can either advance the ledger by
    // completely reconstructing the `Env` from a snapshot (prefered),
    // or by just frobbing the ledger of the storage and preserving
    // the same `Env`.
    let use_snapshot = true;

    if !use_snapshot {
        let env = prev_env.clone();
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
    } else {
        let mut snapshot = prev_env.to_snapshot();
        snapshot.ledger.sequence_number = snapshot
            .ledger
            .sequence_number
            .checked_add(ledgers)
            .expect("end of time");
        snapshot.ledger.timestamp = snapshot
            .ledger
            .timestamp
            .checked_add(ledger_time)
            .expect("end of time");

        let env = Env::from_snapshot(snapshot);

        env
    }
}

/// Advance time, but do it in increments, periodically pinging the contract to
/// keep it alive.
fn advance_time_to(
    config: &Config,
    mut env: Env,
    token_contract_id_bytes: &[u8],
    ledgers: u32,
) -> Env {
    let to_ledger = env
        .ledger()
        .sequence()
        .checked_add(ledgers)
        .expect("end of time");

    loop {
        let curr_ledger = env.ledger().get().sequence_number;
        assert!(curr_ledger < to_ledger);

        let next_ledger = curr_ledger
            .checked_add(MAX_LEDGERS_TO_ADVANCE)
            .expect("end of time");
        let next_ledger = next_ledger.min(to_ledger);

        let advance_ledgers = next_ledger - curr_ledger;

        env = advance_env(env, advance_ledgers);

        let token_contract_id =
            Address::from_string_bytes(&Bytes::from_slice(&env, &token_contract_id_bytes));
        config.reregister_contract(&env, &token_contract_id);

        if next_ledger == to_ledger {
            break;
        } else {
            // Keep the contract alive
            let token_contract_id =
                Address::from_string_bytes(&Bytes::from_slice(&env, &token_contract_id_bytes));
            let token_client = Client::new(&env, &token_contract_id);
            let r = token_client.try_allowance(&Address::generate(&env), &Address::generate(&env));
            assert!(r.is_ok());
        }
    }

    env
}

fn address_to_bytes(addr: &Address) -> RustVec<u8> {
    let addr_str = addr.to_string();
    let mut buf = vec![0; addr_str.len() as usize];
    addr_str.copy_into_slice(&mut buf);
    buf
}

fn verify_token_contract_result(env: &Env, r: &TokenContractResult) {
    match r {
        Err(Ok(e)) => {
            if e.is_type(ScErrorType::WasmVm) && e.is_code(ScErrorCode::InvalidAction) {
                let msg = "contract failed with InvalidAction - unexpected panic?";
                eprintln!("{msg}");
                eprintln!("recent events (10):");
                for (i, event) in env.events().all().iter().rev().take(10).enumerate() {
                    eprintln!("{i}: {event:?}");
                }
                panic!("{msg}");
            }
        }
        _ => {}
    }
}

fn generate_addresses_from_seed(
    env: &Env,
    address_seed: u64,
    address_types: &[AddressType; NUMBER_OF_ADDRESSES],
) -> RustVec<(Address, [u8; 32])> {
    let mut addresses = RustVec::<(Address, [u8; 32])>::new();

    for i in 0..NUMBER_OF_ADDRESSES {
        let seed = address_seed
            .checked_add(i as u64)
            .expect("Overflow")
            .to_be_bytes();
        let seed: [u8; 32] = [
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, seed[0],
            seed[1], seed[2], seed[3], seed[4], seed[5], seed[6], seed[7],
        ];

        let address = match address_types[i] {
            AddressType::Account => {
                let signing_key = SigningKey::from_bytes(&seed);
                let account_id = AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(
                    signing_key.verifying_key().to_bytes(),
                )));

                let sc_address = ScAddress::Account(account_id);
                let address = Address::try_from_val(env, &sc_address).unwrap();
                address
            }
            AddressType::Contract => {
                Address::try_from_val(env, &ScAddress::Contract(Hash(seed))).unwrap()
            }
        };

        addresses.push((address, seed));
    }

    addresses
}

fn create_default_account(env: &Env, account_id: &AccountId, signers: Vec<(&SigningKey, u32)>) {
    let key = LedgerKey::Account(LedgerKeyAccount {
        account_id: account_id.clone(),
    });
    let mut acc_signers = vec![];
    for (signer, weight) in signers {
        acc_signers.push(Signer {
            key: SignerKey::Ed25519(Uint256(signer.verifying_key().to_bytes())),
            weight,
        });
    }

    let ext = AccountEntryExt::V0;
    let acc_entry = AccountEntry {
        account_id: account_id.clone(),
        balance: 10_000_000,
        seq_num: SequenceNumber(0),
        num_sub_entries: 0,
        inflation_dest: None,
        flags: 0,
        home_domain: Default::default(),
        thresholds: Thresholds([1, 0, 0, 0]),
        signers: acc_signers.try_into().unwrap(),
        ext,
    };

    env.host()
        .with_mut_storage(|storage| {
            storage.put(
                &Rc::new(key),
                &Rc::new(LedgerEntry {
                    last_modified_ledger_seq: 0,
                    data: LedgerEntryData::Account(acc_entry),
                    ext: LedgerEntryExt::V0,
                }),
                None,
                soroban_env_host::budget::AsBudget::as_budget(env.host()),
            )
        })
        .expect("ok");
}

fn create_default_trustline(env: &Env, account_id: &AccountId) {
    let seed: [u8; 32] = [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 1,
    ];

    let issuer = AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(seed)));
    let asset = TrustLineAsset::CreditAlphanum4(AlphaNum4 {
        asset_code: AssetCode4([b'a', b'a', b'a', 0]),
        issuer: issuer,
    });

    let key = LedgerKey::Trustline(LedgerKeyTrustLine {
        account_id: account_id.clone(),
        asset: asset.clone(),
    });

    let flags =
        TrustLineFlags::AuthorizedFlag as u32 | TrustLineFlags::TrustlineClawbackEnabledFlag as u32;

    let ext = TrustLineEntryExt::V0;

    let trustline_entry = TrustLineEntry {
        account_id: account_id.clone(),
        asset,
        balance: 0,
        limit: i64::MAX,
        flags,
        ext,
    };

    env.host()
        .with_mut_storage(|storage| {
            storage.put(
                &Rc::new(key),
                &Rc::new(LedgerEntry {
                    last_modified_ledger_seq: 0,
                    data: LedgerEntryData::Trustline(trustline_entry),
                    ext: LedgerEntryExt::V0,
                }),
                None,
                soroban_env_host::budget::AsBudget::as_budget(env.host()),
            )
        })
        .expect("ok");
}
