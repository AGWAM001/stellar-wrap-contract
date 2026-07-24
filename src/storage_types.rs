use soroban_sdk::{contracttype, Address, BytesN, String, Symbol};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractInfo {
    pub name: String,
    pub version: String,
    pub repo: String,
    pub description: String,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WrapRecord {
    pub timestamp: u64,
    pub data_hash: BytesN<32>,
    pub archetype: Symbol,
    pub period: u64,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    AdminPubKey,
    Wrap(Address, u64),
    WrapCount(Address),
    LatestPeriod(Address),
    MintGuard(Address),
    AllowedArchetypes,
    MerkleRoot(u64),
    MerkleClaimed(Address, u64),
    UserOptOut(Address),
    /// Ordered list of all periods minted for a user (Vec<u64>)
    WrapPeriods(Address),
}
