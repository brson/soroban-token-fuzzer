use crate::DAY_IN_LEDGERS;
use arbitrary::Unstructured;
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

const NUMBER_OF_ADDRESSES: usize = 3;

#[derive(Clone, Debug, arbitrary::Arbitrary)]
pub struct Input {
    pub addresses: [<Address as SorobanArbitrary>::Prototype; NUMBER_OF_ADDRESSES],
    pub commands: RustVec<Command>,
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
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(i128::MIN..=i128::MAX))]
    pub amount: i128,
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=NUMBER_OF_ADDRESSES - 1))]
    pub to_account_index: usize,
}

#[derive(Clone, Debug, arbitrary::Arbitrary)]
pub struct ApproveInput {
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(i128::MIN..=i128::MAX))]
    pub amount: i128,
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=DAY_IN_LEDGERS * 30))]
    pub expiration_ledger: u32,
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=NUMBER_OF_ADDRESSES - 1))]
    pub from_account_index: usize,
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=NUMBER_OF_ADDRESSES - 1))]
    pub spender_account_index: usize,
}

#[derive(Clone, Debug, arbitrary::Arbitrary)]
pub struct TransferFromInput {
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(i128::MIN..=i128::MAX))]
    pub amount: i128,
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=NUMBER_OF_ADDRESSES - 1))]
    pub spender_account_index: usize,
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=NUMBER_OF_ADDRESSES - 1))]
    pub from_account_index: usize,
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=NUMBER_OF_ADDRESSES - 1))]
    pub to_account_index: usize,
}

#[derive(Clone, Debug, arbitrary::Arbitrary)]
pub struct TransferInput {
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(i128::MIN..=i128::MAX))]
    pub amount: i128,
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=NUMBER_OF_ADDRESSES - 1))]
    pub from_account_index: usize,
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=NUMBER_OF_ADDRESSES - 1))]
    pub to_account_index: usize,
}

#[derive(Clone, Debug, arbitrary::Arbitrary)]
pub struct BurnFromInput {
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(i128::MIN..=i128::MAX))]
    pub amount: i128,
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=NUMBER_OF_ADDRESSES - 1))]
    pub spender_account_index: usize,
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=NUMBER_OF_ADDRESSES - 1))]
    pub from_account_index: usize,
}

#[derive(Clone, Debug, arbitrary::Arbitrary)]
pub struct BurnInput {
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(i128::MIN..=i128::MAX))]
    pub amount: i128,
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=NUMBER_OF_ADDRESSES - 1))]
    pub from_account_index: usize,
}

#[derive(Clone, Debug, arbitrary::Arbitrary)]
pub struct AdvanceLedgersInput {
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(1..=DAY_IN_LEDGERS))]
    pub ledgers: u32,
}
