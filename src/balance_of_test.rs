#![cfg(test)]

use super::{StellarWrapContract, StellarWrapContractClient};
use soroban_sdk::{Env, Address, testutils::Address as _};

#[test]
fn test_balance_of_starts_at_zero() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StellarWrapContract);
    let client = StellarWrapContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    assert_eq!(client.balance_of(&user), 0);
}
