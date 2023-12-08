#![allow(unused)]

pub mod input;
pub mod config;
pub mod fuzz;

pub use input::Input;
pub use config::{Config, ContractTokenOps, TokenAdminClient};
pub use fuzz::fuzz_token;

// copied from somewhere in the sdk
const DAY_IN_LEDGERS: u32 = 17280;
const INSTANCE_BUMP_AMOUNT: u32 = 7 * DAY_IN_LEDGERS;
const INSTANCE_LIFETIME_THRESHOLD: u32 = INSTANCE_BUMP_AMOUNT - DAY_IN_LEDGERS;

