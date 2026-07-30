#![cfg(test)]

use ed25519_dalek::{Signer, SigningKey};
use soroban_sdk::{Address, BytesN, Env, Symbol};

use crate::mint::CURRENT_PAYLOAD_VERSION;
use crate::signature::construct_mint_payload;

/// Signs the same payload layout the contract rebuilds in `mint::mint_wrap`.
pub(crate) fn sign_payload(
    env: &Env,
    signer: &SigningKey,
    contract: &Address,
    user: &Address,
    period: u64,
    archetype: &Symbol,
    data_hash: &BytesN<32>,
) -> BytesN<64> {
    sign_payload_versioned(
        env,
        signer,
        contract,
        user,
        period,
        archetype,
        data_hash,
        CURRENT_PAYLOAD_VERSION,
    )
}

/// Signs a mint payload for an explicit `payload_version`.
pub(crate) fn sign_payload_versioned(
    env: &Env,
    signer: &SigningKey,
    contract: &Address,
    user: &Address,
    period: u64,
    archetype: &Symbol,
    data_hash: &BytesN<32>,
    payload_version: u32,
) -> BytesN<64> {
    let payload = construct_mint_payload(
        env,
        contract,
        user,
        period,
        archetype,
        data_hash,
        payload_version,
    );

    let mut out = [0u8; 512];
    let len = payload.len() as usize;
    payload.copy_into_slice(&mut out[..len]);

    let signature = signer.sign(&out[..len]);
    BytesN::from_array(env, &signature.to_bytes())
}
