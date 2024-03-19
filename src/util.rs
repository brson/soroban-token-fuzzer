use soroban_sdk::testutils::arbitrary::arbitrary::{self, Arbitrary, Unstructured};
use soroban_sdk::{Address, String};
use std::vec::Vec as RustVec;

pub fn string_to_bytes(s: String) -> RustVec<u8> {
    let mut out = vec![0; s.len() as usize];
    s.copy_into_slice(&mut out);

    out
}

pub fn address_to_bytes(addr: &Address) -> RustVec<u8> {
    let addr_str = addr.to_string();
    let mut buf = vec![0; addr_str.len() as usize];
    addr_str.copy_into_slice(&mut buf);
    buf
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct SmartI128(pub i128);

impl<'a> Arbitrary<'a> for SmartI128 {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        const SMART_CHANCE: (u8, u8) = (1, 100);
        const SMART_VALS: &[i128] = &[0, -1, 1, i128::MIN, i128::MAX];

        if u.ratio(SMART_CHANCE.0, SMART_CHANCE.1)? {
            Ok(SmartI128(*u.choose(SMART_VALS)?))
        } else {
            Ok(SmartI128(u.arbitrary()?))
        }
    }

    fn size_hint(_depth: usize) -> (usize, Option<usize>) {
        let size_u8 = core::mem::size_of::<u8>(); // for the ratio
        let size_usize = core::mem::size_of::<usize>(); // for the choose
        let size_i128 = core::mem::size_of::<i128>(); // for the arbitrary i128
        let needed = size_u8 + size_usize.min(size_i128);
        (needed, Some(needed))
    }
}
