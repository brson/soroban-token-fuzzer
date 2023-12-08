use soroban_sdk::{TryFromVal, Val, Error, InvokeError};
use soroban_sdk::{Env, Address};
use soroban_sdk::token::StellarAssetClient;
use soroban_sdk::xdr::SorobanAuthorizationEntry;
use soroban_sdk::testutils::ContractFunctionSet;

pub struct Config {
    kind: TokenKind,
}

pub enum TokenKind {
    Native,
    Contract(ContractTokenConfig),
}

pub struct ContractTokenConfig {
    ops: Box<dyn ContractTokenOps>,
}

pub trait ContractTokenOps {
    fn register_contract_init(
        &self,
        env: &Env,
        admin: &Address,
    ) -> Address;

    fn reregister_contract(
        &self,
        env: &Env,
        token_contract_id: &Address,
    );

    fn new_admin_client<'a>(
        &self,
        env: &Env,
        token_contract_id: &Address,
    ) -> Box<dyn TokenAdminClient<'a> + 'a>;
}

pub trait TokenAdminClient<'a> {
    fn try_mint(
        &self,
        to: &Address,
        amount: &i128,
    ) -> Result<Result<(), <() as TryFromVal<Env, Val>>::Error>, Result<Error, InvokeError>>;

    // This is just defined to make sure the lifetimes work;
    // we don't actually need to implement it yet. Unused.
    fn set_auths(&self, auths: &'a [SorobanAuthorizationEntry]) -> Box<dyn TokenAdminClient> {
        todo!()
    }
}

struct NativeTokenAdminClient<'a> {
    admin_client: StellarAssetClient<'a>,
}

impl Config {
    pub fn native() -> Config {
        Config {
            kind: TokenKind::Native,
        }
    }

    pub fn contract(
        ops: impl ContractTokenOps + 'static,
    ) -> Config {
        Config {
            kind: TokenKind::Contract(
                ContractTokenConfig {
                    ops: Box::new(ops),
                }
            )
        }
    }

    pub fn register_contract_init(
        &self,
        env: &Env,
        admin: &Address,
    ) -> Address {
        match &self.kind {
            TokenKind::Native => {
                env.register_stellar_asset_contract(admin.clone())
            }
            TokenKind::Contract(cfg) => {
                cfg.register_contract_init(env, admin)
            }
        }
    }

    pub fn reregister_contract(
        &self,
        env: &Env,
        token_contract_id: &Address,
    ) {
        match &self.kind {
            TokenKind::Native => {
                /* nop */
            }
            TokenKind::Contract(cfg) => {
                cfg.reregister_contract(env, token_contract_id)
            }
        }
    }

    pub fn new_admin_client<'a>(
        &self,
        env: &Env,
        token_contract_id: &Address,
    ) -> Box<dyn TokenAdminClient<'a> + 'a> {
        match &self.kind {
            TokenKind::Native => {
                Box::new(NativeTokenAdminClient {
                    admin_client: {
                        StellarAssetClient::new(env, &token_contract_id)
                    }
                })
            }
            TokenKind::Contract(cfg) => {
                cfg.new_admin_client(env, token_contract_id)
            }
        }
    }
}

impl<'a> TokenAdminClient<'a> for NativeTokenAdminClient<'a> {
    fn try_mint(
        &self,
        to: &Address,
        amount: &i128,
    ) -> Result<Result<(), <() as TryFromVal<Env, Val>>::Error>, Result<Error, InvokeError>> {
        self.admin_client.try_mint(to, amount)
    }

    fn set_auths(&self, auths: &'a [SorobanAuthorizationEntry]) -> Box<dyn TokenAdminClient> {
        todo!()
    }
}

impl ContractTokenConfig {
    pub fn register_contract_init(
        &self,
        env: &Env,
        admin: &Address,
    ) -> Address {
        self.ops.register_contract_init(env, admin)
    }

    pub fn reregister_contract(
        &self,
        env: &Env,
        token_contract_id: &Address,
    ) {
        self.ops.reregister_contract(env, token_contract_id)
    }

    pub fn new_admin_client<'a>(
        &self,
        env: &Env,
        token_contract_id: &Address,
    ) -> Box<dyn TokenAdminClient<'a> + 'a> {
        self.ops.new_admin_client(env, token_contract_id)
    }
}

