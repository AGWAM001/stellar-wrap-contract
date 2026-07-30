#![cfg(test)]

use ed25519_dalek::{Signer, SigningKey};
use soroban_sdk::{Address, BytesN, Env, Symbol};

use crate::signature::construct_mint_payload;

/// Signs the canonical mint payload that the contract verifies in `mint::mint_wrap`.
///
/// This delegates to `construct_mint_payload` so test helpers stay in sync
/// with the production signing scheme automatically.
pub(crate) fn sign_payload(
    env: &Env,
    signer: &SigningKey,
    contract: &Address,
    user: &Address,
    period: u64,
    archetype: &Symbol,
    data_hash: &BytesN<32>,
    payload_version: u32,
) -> BytesN<64> {
    let payload = construct_mint_payload(env, contract, user, period, archetype, data_hash, payload_version);

    let mut out = [0u8; 512];
    let len = payload.len() as usize;
    payload.copy_into_slice(&mut out[..len]);

    let sig = signer.sign(&out[..len]);
    BytesN::from_array(env, &sig.to_bytes())
}
