#![no_main]

use libfuzzer_sys::{fuzz_target, Corpus};
use soroban_token_fuzzer::*;
use soroban_sdk::{
    Env, Address, String,
    TryFromVal, Val, Error, InvokeError
};

fuzz_target!(|input: Input| -> Corpus {
    let config = Config::contract(TokenOps);
    fuzz_token(config, input)
});

struct TokenOps;

struct AdminClient<'a> {
    client: example_token::TokenClient<'a>,
}

impl ContractTokenOps for TokenOps {
    fn register_contract_init(
        &self,
        env: &Env,
        admin: &Address,
    ) -> Address {
        let token_contract_id = env.register_contract(None, example_token::contract::Token);

        let admin_client = example_token::TokenClient::new(&env, &token_contract_id);
        let r = admin_client.try_initialize(
            &admin,
            &10,
            &String::from_str(&env, "token"),
            &String::from_str(&env, "TKN"),
        );

        assert!(r.is_ok());

        token_contract_id
    }

    fn reregister_contract(
        &self,
        env: &Env,
        token_contract_id: &Address,
    ) {
        env.register_contract(Some(token_contract_id), example_token::contract::Token);
    }

    fn new_admin_client<'a>(
        &self,
        env: &Env,
        token_contract_id: &Address,
    ) -> Box<dyn TokenAdminClient<'a> + 'a> {
        Box::new(AdminClient {
            client: example_token::TokenClient::new(&env, &token_contract_id),
        })
    }
}

impl<'a> TokenAdminClient<'a> for AdminClient<'a> {
    fn try_mint(
        &self,
        to: &Address,
        amount: &i128,
    ) -> Result<Result<(), <() as TryFromVal<Env, Val>>::Error>, Result<Error, InvokeError>> {
        self.client.try_mint(to, amount)
    }
}
