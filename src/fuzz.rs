use crate::addrgen::{AddressGenerator, TestSigner};
use crate::config::*;
use crate::input::*;
use crate::util::*;
use crate::DAY_IN_LEDGERS;
use itertools::Itertools;
use libfuzzer_sys::Corpus;
use num_bigint::BigInt;
use soroban_sdk::testutils::{Address as _, Events, Ledger, LedgerInfo};
use soroban_sdk::xdr::{ScErrorCode, ScErrorType};
use soroban_sdk::{
    token::Client, Address, Bytes, Env, Error, IntoVal, InvokeError, TryFromVal, Val, BytesN,
    contract, contractimpl, contracttype,
};
use soroban_sdk::xdr::{
    VecM,
    ScAddress,
    ScVal,
    SorobanAuthorizationEntry,
    HashIdPreimageSorobanAuthorization,
    HashIdPreimage,
    SorobanCredentials,
    ScSymbol,
    InvokeContractArgs,
    SorobanAuthorizedFunction,
    SorobanAuthorizedInvocation,
    SorobanAddressCredentials,
};
use std::collections::BTreeMap;
use std::vec::Vec as RustVec;
use ed25519_dalek::{SigningKey, Signer};
use soroban_sdk::xdr::{WriteXdr, Limited, Limits};
use sha2::{Sha256, Digest};

// Don't know where this number comes from.
const MAX_LEDGERS_TO_ADVANCE: u32 = 4095;

type TokenContractResult =
    Result<Result<(), <() as TryFromVal<Env, Val>>::Error>, Result<Error, InvokeError>>;

pub fn fuzz_token(config: Config, input: Input) -> Corpus {

    if input.transactions.iter().all(|tx| tx.commands.is_empty()) {
        return Corpus::Reject;
    }

    //eprintln!("input: {input:#?}");

    // The initial Env. This will be destroyed and recreated when we advance time,
    // to simulate distinct transactions.
    let mut env = Env::default();

    let token_contract_id_bytes: RustVec<u8>;

    // Do initial setup, including registering the contract.
    {
        input.address_generator.setup_account_storage(&env);

        let signers = input.address_generator.generate_signers(&env);
        let admin = &signers[0].address;

        let token_contract_id = config.register_contract_init(&env, admin);
        token_contract_id_bytes = address_to_bytes(&token_contract_id);
    }

    let mut contract_state = ContractState::init();
    let mut current_state = CurrentState::new(
        &env,
        &config,
        &token_contract_id_bytes,
        &input.address_generator,
    );
    let mut signature_nonce = 0;

    // Save some values that should never change
    // fixme put this in the ContractState ctor
    {
        let token_client = &current_state.token_client;

        contract_state.name = string_to_bytes(token_client.name());
        contract_state.symbol = string_to_bytes(token_client.symbol());
        contract_state.decimals = token_client.decimals();
    }

    for transaction in &input.transactions {
        // The Env will be different for each tx, so we need to reconstruct
        // everything that depends on it.
        env.budget().reset_unlimited();

        for command in &transaction.commands {
            // println!("------- command: {:#?}", command);
            exec_command(
                &command,
                &env,
                &token_contract_id_bytes,
                &mut contract_state,
                &current_state,
                &mut signature_nonce,
            );
        }

        // Advance time and begin new transaction
        {
            env = advance_time(
                &config,
                env,
                &token_contract_id_bytes,
                transaction.advance_ledgers,
            );
            // NB: This env is reconstructed and all previous env-based objects are invalid

            current_state = CurrentState::new(
                &env,
                &config,
                &token_contract_id_bytes,
                &input.address_generator,
            );

            // update saved allowance number after advance ledgers
            // fixme track expiration ledger instead of asking the contract
            {
                let pairs = current_state
                    .accounts
                    .iter()
                    .cartesian_product(current_state.accounts.iter());
                for (signer1, signer2) in pairs {
                    let expected_allowance = contract_state.get_allowance(&signer1.address, &signer2.address);
                    let actual_allowance = current_state.token_client.allowance(&signer1.address, &signer2.address);
                    if actual_allowance == 0 && expected_allowance != 0 {
                        // Assume the allowance expired.
                        contract_state.set_allowance(&signer1.address, &signer2.address, actual_allowance);
                    }
                }
            }

            assert_state(&contract_state, &current_state);
        }
    }

    Corpus::Keep
}

fn exec_command(
    command: &Command,
    env: &Env,
    token_contract_id_bytes: &[u8],
    contract_state: &mut ContractState,
    current_state: &CurrentState,
    signature_nonce: &mut i64,
) {
    let admin_client = &current_state.admin_client;
    let token_client = &current_state.token_client;
    let accounts = &current_state.accounts;

    match command {
        Command::Mint(input) => {
            mock_auths_for_command(
                env,
                "mint",
                &input.auths,
                current_state,
                token_contract_id_bytes,
                signature_nonce,
                (
                    &accounts[input.to_account_index].address,
                    input.amount,
                )
                    .into_val(env),
            );

            let r = admin_client.try_mint(
                &accounts[input.to_account_index].address,
                &input.amount,
            );

            verify_token_contract_result(&env, &r);

            if input.amount < 0 {
                assert!(r.is_err());
            }

            if let Ok(r) = r {
                let _r = r.expect("ok");

                contract_state.add_balance(&accounts[input.to_account_index].address, input.amount);

                contract_state.sum_of_mints =
                    contract_state.sum_of_mints.clone() + BigInt::from(input.amount);
            }
        }
        Command::Approve(input) => {
            mock_auths_for_command(
                env,
                "approve",
                &input.auths,
                current_state,
                token_contract_id_bytes,
                signature_nonce,
                (
                    &accounts[input.from_account_index].address,
                    &accounts[input.spender_account_index].address,
                    input.amount,
                    input.expiration_ledger,
                )
                    .into_val(env),
            );

            let r = token_client.try_approve(
                &accounts[input.from_account_index].address,
                &accounts[input.spender_account_index].address,
                &input.amount,
                &input.expiration_ledger,
            );

            verify_token_contract_result(&env, &r);

            if input.amount < 0 {
                assert!(r.is_err());
            }

            if let Ok(r) = r {
                let _r = r.expect("ok");

                contract_state.set_allowance(
                    &accounts[input.from_account_index].address,
                    &accounts[input.spender_account_index].address,
                    input.amount,
                );
            }
        }
        Command::TransferFrom(input) => {
            mock_auths_for_command(
                env,
                "transfer_from",
                &input.auths,
                current_state,
                token_contract_id_bytes,
                signature_nonce,
                (
                    &accounts[input.spender_account_index].address,
                    &accounts[input.from_account_index].address,
                    &accounts[input.to_account_index].address,
                    input.amount,
                )
                    .into_val(env),
            );

            let r = token_client.try_transfer_from(
                &accounts[input.spender_account_index].address,
                &accounts[input.from_account_index].address,
                &accounts[input.to_account_index].address,
                &input.amount,
            );

            verify_token_contract_result(&env, &r);

            if input.amount < 0 {
                assert!(r.is_err());
            }

            if let Ok(r) = r {
                let _r = r.expect("ok");

                contract_state.sub_balance(&accounts[input.from_account_index].address, input.amount);
                contract_state.add_balance(&accounts[input.to_account_index].address, input.amount);

                contract_state.sub_allowance(
                    &accounts[input.from_account_index].address,
                    &accounts[input.spender_account_index].address,
                    input.amount,
                );
            }
        }
        Command::Transfer(input) => {
            mock_auths_for_command(
                env,
                "transfer",
                &input.auths,
                current_state,
                token_contract_id_bytes,
                signature_nonce,
                (
                    &accounts[input.from_account_index].address,
                    &accounts[input.to_account_index].address,
                    input.amount,
                )
                    .into_val(env),
            );

            let r = token_client.try_transfer(
                &accounts[input.from_account_index].address,
                &accounts[input.to_account_index].address,
                &input.amount,
            );

            verify_token_contract_result(&env, &r);

            if input.amount < 0 {
                assert!(r.is_err());
            }

            if let Ok(r) = r {
                let _r = r.expect("ok");

                contract_state.sub_balance(&accounts[input.from_account_index].address, input.amount);
                contract_state.add_balance(&accounts[input.to_account_index].address, input.amount);
            }
        }
        Command::BurnFrom(input) => {
            mock_auths_for_command(
                env,
                "burn_from",
                &input.auths,
                current_state,
                token_contract_id_bytes,
                signature_nonce,
                (
                    &accounts[input.spender_account_index].address,
                    &accounts[input.from_account_index].address,
                    input.amount,
                )
                    .into_val(env),
            );

            let r = token_client.try_burn_from(
                &accounts[input.spender_account_index].address,
                &accounts[input.from_account_index].address,
                &input.amount,
            );

            verify_token_contract_result(&env, &r);

            if input.amount < 0 {
                assert!(r.is_err());
            }

            if let Ok(r) = r {
                let _r = r.expect("ok");

                contract_state.sub_balance(&accounts[input.from_account_index].address, input.amount);

                contract_state.sub_allowance(
                    &accounts[input.from_account_index].address,
                    &accounts[input.spender_account_index].address,
                    input.amount,
                );

                contract_state.sum_of_burns =
                    contract_state.sum_of_burns.clone() + &BigInt::from(input.amount);
            }
        }
        Command::Burn(input) => {
            mock_auths_for_command(
                env,
                "burn",
                &input.auths,
                current_state,
                token_contract_id_bytes,
                signature_nonce,
                (&accounts[input.from_account_index].address, input.amount).into_val(env),
            );

            let r = token_client.try_burn(&accounts[input.from_account_index].address, &input.amount);

            verify_token_contract_result(&env, &r);

            if input.amount < 0 {
                assert!(r.is_err());
            }

            if let Ok(r) = r {
                let _r = r.expect("ok");

                contract_state.sub_balance(&accounts[input.from_account_index].address, input.amount);

                contract_state.sum_of_burns =
                    contract_state.sum_of_burns.clone() + &BigInt::from(input.amount);
            }
        }
    }
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
    accounts: Vec<TestSigner>,
    admin_client: Box<dyn TokenAdminClient<'a> + 'a>,
    token_client: Client<'a>,
}

impl<'a> CurrentState<'a> {
    fn new(
        env: &Env,
        config: &Config,
        token_contract_id_bytes: &[u8],
        address_generator: &AddressGenerator,
    ) -> Self {
        let token_contract_id =
            Address::from_string_bytes(&Bytes::from_slice(env, &token_contract_id_bytes));
        let admin_client = config.new_admin_client(env, &token_contract_id);
        let token_client = Client::new(env, &token_contract_id);

        let accounts = address_generator.generate_signers(env);

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

    for signer in &current.accounts {
        assert_eq!(contract.get_balance(&signer.address), token_client.balance(&signer.address));
        assert!(token_client.balance(&signer.address) >= 0)
    }

    let pairs = current
        .accounts
        .iter()
        .cartesian_product(current.accounts.iter());

    for (signer1, signer2) in pairs {
        assert_eq!(
            contract.get_allowance(&signer1.address, &signer2.address),
            token_client.allowance(&signer1.address, &signer2.address),
        );
    }

    let sum_of_balances_0 = &contract.sum_of_mints - &contract.sum_of_burns;
    let sum_of_balances_1 = current
        .accounts
        .iter()
        .map(|a| BigInt::from(token_client.balance(&a.address)))
        .sum();

    assert_eq!(sum_of_balances_0, sum_of_balances_1);
}

/// Advance time, but do it in increments, periodically pinging the contract to
/// keep it alive.
fn advance_time(
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

fn verify_token_contract_result(env: &Env, r: &TokenContractResult) {
    match r {
        Err(Ok(e)) => {
            if e.is_type(ScErrorType::WasmVm) && e.is_code(ScErrorCode::InvalidAction) {
                let msg = "contract failed with InvalidAction - unexpected panic?";
                eprintln!("{msg}");
                print_diagnostics(env);
                panic!("{msg}");
            }
        }
        _ => {}
    }
}

fn print_diagnostics(env: &Env) {
    eprintln!("recent events (10):");
    for (i, event) in env.events().all().iter().rev().take(10).enumerate() {
        eprintln!("{i}: {event:?}");
    }
}

#[contract]
pub struct MockAuthContract;

#[contractimpl]
impl MockAuthContract {
    #[allow(non_snake_case)]
    pub fn __check_auth(_signature_payload: Val, _signatures: Val, _auth_context: Val) {}
}

fn mock_auths_for_command(
    env: &Env,
    fn_name: &str,
    auths: &[bool],
    current_state: &CurrentState,
    token_contract_id_bytes: &[u8],
    signature_nonce: &mut i64,
    args: soroban_sdk::Vec<Val>,
) {
    let curr_ledger = env.ledger().sequence();
    let max_entry_ttl = env.ledger().get().max_entry_ttl;
    let expiration_ledger = curr_ledger + max_entry_ttl - 1;

    let token_contract_id =
        Address::from_string_bytes(&Bytes::from_slice(env, token_contract_id_bytes));
    let token_contract_sc_address = ScAddress::try_from(token_contract_id).unwrap();

    let mut auth_entries = RustVec::new();
    
    for i in 0..NUMBER_OF_ADDRESSES {
        if auths[i] {
            let signer = &current_state.accounts[i];
            let sc_address = ScAddress::try_from(signer.address.clone()).unwrap();

            let is_contract_address = signer.key.is_none();

            // contract addresses need to have registered contracts to be authorizers,
            // at least according to the sdk's mock_auths method
            if is_contract_address {
                env.register_contract(&signer.address, MockAuthContract);
            }

            let mut credentials = SorobanAddressCredentials {
                address: sc_address,
                nonce: *signature_nonce,
                signature_expiration_ledger: expiration_ledger,
                signature: ScVal::Void, // updated below for non-contract addresses
            };

            let root_invocation = SorobanAuthorizedInvocation {
                function: SorobanAuthorizedFunction::ContractFn(InvokeContractArgs {
                    contract_address: token_contract_sc_address.clone(),
                    function_name: ScSymbol(fn_name.try_into().unwrap()),
                    args: VecM::try_from(args.clone()).unwrap(),
                }),
                sub_invocations: Default::default(),
            };

            if let Some(key) = &signer.key {
                let signature_payload_preimage =
                    HashIdPreimage::SorobanAuthorization(HashIdPreimageSorobanAuthorization {
                        network_id: env
                            .host()
                            .with_ledger_info(|li: &LedgerInfo| Ok(li.network_id))
                            .unwrap()
                            .try_into()
                            .unwrap(),
                        invocation: root_invocation.clone(),
                        nonce: *signature_nonce,
                        signature_expiration_ledger: expiration_ledger,
                    });

                let mut buf = vec![];
                let mut unlimited_buf = Limited::new(&mut buf, Limits::none());
                signature_payload_preimage.write_xdr(&mut unlimited_buf).unwrap();
                let signature_payload: [u8; 32] = Sha256::digest(&buf).try_into().unwrap();

                let signature = sign_payload_for_account(env, key, &signature_payload);
                let signatures = soroban_sdk::vec![env, signature];
                credentials.signature = signatures.try_into().unwrap();
            }
            
            *signature_nonce += 1;

            let auth_entry = SorobanAuthorizationEntry {
                credentials: SorobanCredentials::Address(credentials),
                root_invocation,
            };
            auth_entries.push(auth_entry);
        }
    }

    env.set_auths(&auth_entries);
}

#[contracttype]
pub(crate) struct AccountEd25519Signature {
    pub(crate) public_key: BytesN<32>,
    pub(crate) signature: BytesN<64>,
}

fn sign_payload_for_account(
    env: &Env,
    signer: &SigningKey,
    payload: &[u8],
) -> AccountEd25519Signature {
    AccountEd25519Signature {
        public_key: BytesN::<32>::try_from_val(
            env,
            &signer.verifying_key().to_bytes(),
        )
        .unwrap(),
        signature: BytesN::<64>::try_from_val(
            env,
            &signer.sign(payload).to_bytes(),
        )
        .unwrap(),
    }
}
