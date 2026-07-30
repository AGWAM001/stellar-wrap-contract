use soroban_sdk::{xdr::ToXdr, Address, Bytes, BytesN, Env, Symbol};

use crate::ContractError;

/// Domain separator used for mint signatures.
///
/// Off-chain signers must construct the same byte sequence before signing.
/// Including a domain separator makes the payload self-describing and prevents
/// ambiguity if the same key is reused for other Soroban contracts or future
/// signing schemes.
pub const MINT_DOMAIN_SEPARATOR: &[u8; 15] = b"stellar-wrap-v1";

/// Construct the canonical mint payload that is signed by the admin.
///
/// The payload is the concatenation of a domain separator, the contract ID,
/// the user address, the period, the archetype, and the data hash. Each field
/// is encoded with XDR so the byte layout is deterministic and unambiguous.
pub fn construct_mint_payload(
    e: &Env,
    contract_id: &Address,
    user: &Address,
    period: u64,
    archetype: &Symbol,
    data_hash: &BytesN<32>,
    payload_version: u32,
) -> Bytes {
    let mut payload = Bytes::new(e);
    payload.append(&Bytes::from_array(e, MINT_DOMAIN_SEPARATOR));
    payload.append(&payload_version.to_xdr(e));
    payload.append(&contract_id.to_xdr(e));
    payload.append(&user.clone().to_xdr(e));
    payload.append(&period.to_xdr(e));
    payload.append(&archetype.clone().to_xdr(e));
    payload.append(&data_hash.clone().to_xdr(e));
    payload
}

/// Verify an admin signature for a wrap mint request.
///
/// The verification is performed over the canonical mint payload so the
/// signature is bound to the current contract instance, the target user,
/// the period, the archetype, and the data hash.
#[allow(clippy::too_many_arguments)]
pub fn verify_mint_signature(
    e: &Env,
    admin_pubkey: &BytesN<32>,
    contract_id: &Address,
    user: &Address,
    period: u64,
    archetype: &Symbol,
    data_hash: &BytesN<32>,
    payload_version: u32,
    signature: &BytesN<64>,
) -> Result<(), ContractError> {
    let payload = construct_mint_payload(e, contract_id, user, period, archetype, data_hash, payload_version);
    e.crypto().ed25519_verify(admin_pubkey, &payload, signature);
    Ok(())
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use soroban_sdk::{symbol_short, testutils::Address as _, Address, Bytes, BytesN, Env, Symbol};
    use std::panic::{catch_unwind, AssertUnwindSafe};

    use crate::StellarWrapContract;

    fn sign_payload(
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

        let signature = signer.sign(&out[..len]);
        BytesN::from_array(env, &signature.to_bytes())
    }

    #[test]
    fn test_construct_mint_payload_has_expected_byte_layout() {
        let env = Env::default();
        let contract_id = env.register_contract(None, StellarWrapContract);
        let user = Address::generate(&env);
        let archetype = symbol_short!("arch");
        let data_hash = BytesN::from_array(&env, &[42u8; 32]);
        let period = 202512u64;

        let payload =
            construct_mint_payload(&env, &contract_id, &user, period, &archetype, &data_hash, 1);

        let mut expected = Bytes::new(&env);
        expected.append(&Bytes::from_array(&env, MINT_DOMAIN_SEPARATOR));
        expected.append(&1u32.to_xdr(&env));
        expected.append(&contract_id.to_xdr(&env));
        expected.append(&user.clone().to_xdr(&env));
        expected.append(&period.to_xdr(&env));
        expected.append(&archetype.clone().to_xdr(&env));
        expected.append(&data_hash.clone().to_xdr(&env));

        assert_eq!(payload, expected);
    }

    #[test]
    fn test_verify_mint_signature_accepts_valid_signature() {
        let env = Env::default();
        let contract_id = env.register_contract(None, StellarWrapContract);
        let user = Address::generate(&env);
        let archetype = symbol_short!("arch");
        let data_hash = BytesN::from_array(&env, &[7u8; 32]);
        let period = 202601u64;

        let signing_key = SigningKey::from_bytes(&[11u8; 32]);
        let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
        let signature = sign_payload(
            &env,
            &signing_key,
            &contract_id,
            &user,
            period,
            &archetype,
            &data_hash,
            1,
        );

        assert!(verify_mint_signature(
            &env,
            &admin_pubkey,
            &contract_id,
            &user,
            period,
            &archetype,
            &data_hash,
            1,
            &signature,
        )
        .is_ok());
    }

    #[test]
    fn test_verify_mint_signature_rejects_invalid_signature() {
        let env = Env::default();
        let contract_id = env.register_contract(None, StellarWrapContract);
        let user = Address::generate(&env);
        let archetype = symbol_short!("arch");
        let data_hash = BytesN::from_array(&env, &[8u8; 32]);
        let period = 202602u64;

        let signing_key = SigningKey::from_bytes(&[12u8; 32]);
        let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
        let invalid_signature = BytesN::from_array(&env, &[0u8; 64]);

        let result = catch_unwind(AssertUnwindSafe(|| {
            verify_mint_signature(
                &env,
                &admin_pubkey,
                &contract_id,
                &user,
                period,
                &archetype,
                &data_hash,
                1,
                &invalid_signature,
            )
            .unwrap();
        }));

        assert!(result.is_err());
    }

    #[test]
    fn test_verify_mint_signature_rejects_wrong_key() {
        let env = Env::default();
        let contract_id = env.register_contract(None, StellarWrapContract);
        let user = Address::generate(&env);
        let archetype = symbol_short!("arch");
        let data_hash = BytesN::from_array(&env, &[9u8; 32]);
        let period = 202603u64;

        let signing_key = SigningKey::from_bytes(&[13u8; 32]);
        let wrong_signing_key = SigningKey::from_bytes(&[14u8; 32]);
        let admin_pubkey = BytesN::from_array(&env, &signing_key.verifying_key().to_bytes());
        let wrong_signature = sign_payload(
            &env,
            &wrong_signing_key,
            &contract_id,
            &user,
            period,
            &archetype,
            &data_hash,
            1,
        );

        let result = catch_unwind(AssertUnwindSafe(|| {
            verify_mint_signature(
                &env,
                &admin_pubkey,
                &contract_id,
                &user,
                period,
                &archetype,
                &data_hash,
                1,
                &wrong_signature,
            )
            .unwrap();
        }));

        assert!(result.is_err());
    }
}
