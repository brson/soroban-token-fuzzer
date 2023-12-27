use soroban_sdk::{String, Address};
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

