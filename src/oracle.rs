use soroban_sdk::{contractclient, Address, BytesN, Env};

/// Minimal ABI implemented by a compatible data-hash oracle.
#[contractclient(name = "DataHashOracleClient")]
pub trait DataHashOracle {
    fn verify_data_hash(e: Env, data_hash: BytesN<32>) -> bool;
}

pub(crate) fn verify_data_hash(e: &Env, oracle: &Address, data_hash: &BytesN<32>) -> bool {
    DataHashOracleClient::new(e, oracle).verify_data_hash(data_hash)
}
