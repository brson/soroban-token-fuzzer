use crate::addrgen::AddressGenerator;
use crate::util::SmartI128;
use crate::DAY_IN_LEDGERS;
use arbitrary::Unstructured;
use soroban_sdk::testutils::arbitrary::arbitrary;
use std::vec::Vec as RustVec;

pub const NUMBER_OF_ADDRESSES: usize = 3;

/// Input generated by the fuzzer as the argument to `fuzz_target!`.
///
/// It consists of addresses and a series of commands that operate on them.
#[derive(Clone, Debug, arbitrary::Arbitrary)]
pub struct Input {
    pub address_generator: AddressGenerator,
    pub transactions: RustVec<Transaction>,
}

#[derive(Clone, Debug, arbitrary::Arbitrary)]
pub struct Transaction {
    pub commands: RustVec<Command>,
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(1..=DAY_IN_LEDGERS))]
    pub advance_ledgers: u32,
}

#[derive(Clone, Debug, arbitrary::Arbitrary)]
pub enum Command {
    Mint(MintInput),
    Approve(ApproveInput),
    TransferFrom(TransferFromInput),
    Transfer(TransferInput),
    BurnFrom(BurnFromInput),
    Burn(BurnInput),
    // These two exist just to make it more likely the fuzzer
    // will generate a successful transfer_from / burn_from call
    ApproveAndTransferFrom(ApproveAndTransferFromInput),
    ApproveAndBurnFrom(ApproveAndBurnFromInput),
}

#[derive(Clone, Debug, arbitrary::Arbitrary)]
pub struct MintInput {
    pub amount: SmartI128,
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=NUMBER_OF_ADDRESSES - 1))]
    pub to_account_index: usize,
    #[arbitrary(with = |u: &mut Unstructured| {
        // biased bool - only sometimes decline the auth
        Ok(<[bool; NUMBER_OF_ADDRESSES]>::try_from(
            std::iter::from_fn(|| Some(u.ratio(9, 10).unwrap_or(true)))
                .take(NUMBER_OF_ADDRESSES)
                .collect::<Vec<_>>()
        ).unwrap())
    })]
    pub auths: [bool; NUMBER_OF_ADDRESSES],
}

#[derive(Clone, Debug, arbitrary::Arbitrary)]
pub struct ApproveInput {
    pub amount: SmartI128,
    pub expiration_ledger: u32,
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=NUMBER_OF_ADDRESSES - 1))]
    pub from_account_index: usize,
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=NUMBER_OF_ADDRESSES - 1))]
    pub spender_account_index: usize,
    #[arbitrary(with = |u: &mut Unstructured| {
        // biased bool - only sometimes decline the auth
        Ok(<[bool; NUMBER_OF_ADDRESSES]>::try_from(
            std::iter::from_fn(|| Some(u.ratio(9, 10).unwrap_or(true)))
                .take(NUMBER_OF_ADDRESSES)
                .collect::<Vec<_>>()
        ).unwrap())
    })]
    pub auths: [bool; NUMBER_OF_ADDRESSES],
}

#[derive(Clone, Debug, arbitrary::Arbitrary)]
pub struct TransferFromInput {
    pub amount: SmartI128,
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=NUMBER_OF_ADDRESSES - 1))]
    pub spender_account_index: usize,
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=NUMBER_OF_ADDRESSES - 1))]
    pub from_account_index: usize,
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=NUMBER_OF_ADDRESSES - 1))]
    pub to_account_index: usize,
    #[arbitrary(with = |u: &mut Unstructured| {
        // biased bool - only sometimes decline the auth
        Ok(<[bool; NUMBER_OF_ADDRESSES]>::try_from(
            std::iter::from_fn(|| Some(u.ratio(9, 10).unwrap_or(true)))
                .take(NUMBER_OF_ADDRESSES)
                .collect::<Vec<_>>()
        ).unwrap())
    })]
    pub auths: [bool; NUMBER_OF_ADDRESSES],
}

#[derive(Clone, Debug, arbitrary::Arbitrary)]
pub struct TransferInput {
    pub amount: SmartI128,
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=NUMBER_OF_ADDRESSES - 1))]
    pub from_account_index: usize,
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=NUMBER_OF_ADDRESSES - 1))]
    pub to_account_index: usize,
    #[arbitrary(with = |u: &mut Unstructured| {
        // biased bool - only sometimes decline the auth
        Ok(<[bool; NUMBER_OF_ADDRESSES]>::try_from(
            std::iter::from_fn(|| Some(u.ratio(9, 10).unwrap_or(true)))
                .take(NUMBER_OF_ADDRESSES)
                .collect::<Vec<_>>()
        ).unwrap())
    })]
    pub auths: [bool; NUMBER_OF_ADDRESSES],
}

#[derive(Clone, Debug, arbitrary::Arbitrary)]
pub struct BurnFromInput {
    pub amount: SmartI128,
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=NUMBER_OF_ADDRESSES - 1))]
    pub spender_account_index: usize,
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=NUMBER_OF_ADDRESSES - 1))]
    pub from_account_index: usize,
    #[arbitrary(with = |u: &mut Unstructured| {
        // biased bool - only sometimes decline the auth
        Ok(<[bool; NUMBER_OF_ADDRESSES]>::try_from(
            std::iter::from_fn(|| Some(u.ratio(9, 10).unwrap_or(true)))
                .take(NUMBER_OF_ADDRESSES)
                .collect::<Vec<_>>()
        ).unwrap())
    })]
    pub auths: [bool; NUMBER_OF_ADDRESSES],
}

#[derive(Clone, Debug, arbitrary::Arbitrary)]
pub struct BurnInput {
    pub amount: SmartI128,
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=NUMBER_OF_ADDRESSES - 1))]
    pub from_account_index: usize,
    #[arbitrary(with = |u: &mut Unstructured| {
        // biased bool - only sometimes decline the auth
        Ok(<[bool; NUMBER_OF_ADDRESSES]>::try_from(
            std::iter::from_fn(|| Some(u.ratio(9, 10).unwrap_or(true)))
                .take(NUMBER_OF_ADDRESSES)
                .collect::<Vec<_>>()
        ).unwrap())
    })]
    pub auths: [bool; NUMBER_OF_ADDRESSES],
}

#[derive(Clone, Debug, arbitrary::Arbitrary)]
pub struct ApproveAndTransferFromInput {
    pub amount: SmartI128,
    pub expiration_ledger: u32,
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=NUMBER_OF_ADDRESSES - 1))]
    pub from_account_index: usize,
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=NUMBER_OF_ADDRESSES - 1))]
    pub spender_account_index: usize,
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=NUMBER_OF_ADDRESSES - 1))]
    pub to_account_index: usize,
    #[arbitrary(with = |u: &mut Unstructured| {
        // biased bool - only sometimes decline the auth
        Ok(<[bool; NUMBER_OF_ADDRESSES]>::try_from(
            std::iter::from_fn(|| Some(u.ratio(9, 10).unwrap_or(true)))
                .take(NUMBER_OF_ADDRESSES)
                .collect::<Vec<_>>()
        ).unwrap())
    })]
    pub auths: [bool; NUMBER_OF_ADDRESSES],
}

#[derive(Clone, Debug, arbitrary::Arbitrary)]
pub struct ApproveAndBurnFromInput {
    pub amount: SmartI128,
    pub expiration_ledger: u32,
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=NUMBER_OF_ADDRESSES - 1))]
    pub from_account_index: usize,
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=NUMBER_OF_ADDRESSES - 1))]
    pub spender_account_index: usize,
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=NUMBER_OF_ADDRESSES - 1))]
    pub to_account_index: usize,
    #[arbitrary(with = |u: &mut Unstructured| {
        // biased bool - only sometimes decline the auth
        Ok(<[bool; NUMBER_OF_ADDRESSES]>::try_from(
            std::iter::from_fn(|| Some(u.ratio(9, 10).unwrap_or(true)))
                .take(NUMBER_OF_ADDRESSES)
                .collect::<Vec<_>>()
        ).unwrap())
    })]
    pub auths: [bool; NUMBER_OF_ADDRESSES],
}

impl ApproveAndTransferFromInput {
    pub fn to_approve_input(&self) -> ApproveInput {
        ApproveInput {
            amount: self.amount,
            expiration_ledger: self.expiration_ledger,
            from_account_index: self.from_account_index,
            spender_account_index: self.spender_account_index,
            auths: self.auths,
        }
    }

    pub fn to_transfer_from_input(&self) -> TransferFromInput {
        TransferFromInput {
            amount: self.amount,
            spender_account_index: self.spender_account_index,
            from_account_index: self.from_account_index,
            to_account_index: self.to_account_index,
            auths: self.auths,
        }
    }
}

impl ApproveAndBurnFromInput {
    pub fn to_approve_input(&self) -> ApproveInput {
        ApproveInput {
            amount: self.amount,
            expiration_ledger: self.expiration_ledger,
            from_account_index: self.from_account_index,
            spender_account_index: self.spender_account_index,
            auths: self.auths,
        }
    }

    pub fn to_burn_from_input(&self) -> BurnFromInput {
        BurnFromInput {
            amount: self.amount,
            spender_account_index: self.spender_account_index,
            from_account_index: self.from_account_index,
            auths: self.auths,
        }
    }
}
