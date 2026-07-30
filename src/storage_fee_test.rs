#![cfg(test)]
extern crate std;
use soroban_sdk::Env;

use crate::StellarWrapContract;

#[test]
fn storage_accounting_compile_test() {
    let e = Env::default();
    let _ = StellarWrapContract::storage_bytes(e.clone());
    let _ = StellarWrapContract::current_fee(e);
}
