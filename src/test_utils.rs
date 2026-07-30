#![cfg(test)]

use ed25519_dalek::{Signer, SigningKey};
use soroban_sdk::{xdr::ToXdr, Address, Bytes, BytesN, Env, Symbol};

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
    let payload = crate::signature::construct_mint_payload(
        env,
        contract,
        user,
        period,
        archetype,
        data_hash,
        1, // Defaulting payload_version to 1 for this test utility
    );

    let mut out = [0u8; 512];
    let len = payload.len() as usize;
    payload.copy_into_slice(&mut out[..len]);

    let signature = signer.sign(&out[..len]);
    BytesN::from_array(env, &signature.to_bytes())
}
