//! Cryptographic utilities for VeloChain.
//!
//! Provides ECDSA signing and verification using the secp256k1 curve,
//! compatible with Ethereum's signature scheme.

use alloy_primitives::{Address, B256};
use k256::ecdsa::{
    signature::hazmat::PrehashSigner, RecoveryId, Signature, SigningKey, VerifyingKey,
};
use sha3::{Digest, Keccak256};

use crate::PrimitivesError;

/// A keypair for signing transactions and blocks.
#[derive(Clone)]
pub struct Keypair {
    signing_key: SigningKey,
    address: Address,
}

impl std::fmt::Debug for Keypair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Keypair")
            .field("address", &self.address)
            .finish()
    }
}

impl Keypair {
    /// Create a keypair from a raw 32-byte private key.
    pub fn from_secret_key(secret: &[u8; 32]) -> Result<Self, PrimitivesError> {
        let signing_key = SigningKey::from_bytes(secret.into())
            .map_err(|e| PrimitivesError::SignatureError(format!("Invalid secret key: {e}")))?;
        let address = public_key_to_address(signing_key.verifying_key());
        Ok(Self {
            signing_key,
            address,
        })
    }

    /// Create a keypair from a hex-encoded private key (with or without 0x prefix).
    pub fn from_hex(hex_key: &str) -> Result<Self, PrimitivesError> {
        let hex_key = hex_key.strip_prefix("0x").unwrap_or(hex_key);
        let bytes = hex::decode(hex_key)
            .map_err(|e| PrimitivesError::SignatureError(format!("Invalid hex: {e}")))?;
        if bytes.len() != 32 {
            return Err(PrimitivesError::SignatureError(format!(
                "Secret key must be 32 bytes, got {}",
                bytes.len()
            )));
        }
        let mut secret = [0u8; 32];
        secret.copy_from_slice(&bytes);
        Self::from_secret_key(&secret)
    }

    /// Generate a random keypair.
    pub fn random() -> Self {
        let signing_key = SigningKey::random(&mut rand::thread_rng());
        let address = public_key_to_address(signing_key.verifying_key());
        Self {
            signing_key,
            address,
        }
    }

    /// Get the Ethereum address derived from this keypair.
    pub fn address(&self) -> Address {
        self.address
    }

    /// Sign a 32-byte message hash, returning (signature, recovery_id).
    pub fn sign_hash(&self, hash: &B256) -> Result<(Signature, RecoveryId), PrimitivesError> {
        let (sig, recid) = self
            .signing_key
            .sign_prehash(hash.as_ref())
            .map_err(|e| PrimitivesError::SignatureError(format!("Signing failed: {e}")))?;
        Ok((sig, recid))
    }

    /// Get the raw secret key bytes.
    pub fn secret_key_bytes(&self) -> [u8; 32] {
        self.signing_key.to_bytes().into()
    }

    /// Get the hex-encoded private key (without 0x prefix).
    pub fn secret_key_hex(&self) -> String {
        hex::encode(self.secret_key_bytes())
    }

    /// Save the keypair to a keystore file (simple JSON format).
    /// The private key is XOR-encrypted with the password hash for basic protection.
    pub fn save_keystore(
        &self,
        path: &std::path::Path,
        password: &str,
    ) -> Result<(), PrimitivesError> {
        let secret = self.secret_key_bytes();
        let pw_hash = Keccak256::digest(password.as_bytes());
        let mut encrypted = [0u8; 32];
        for i in 0..32 {
            encrypted[i] = secret[i] ^ pw_hash[i];
        }
        let keystore = serde_json::json!({
            "address": format!("{:?}", self.address),
            "crypto": {
                "cipher": "xor-keccak256",
                "ciphertext": hex::encode(encrypted),
            },
            "version": 1,
        });
        let data = serde_json::to_string_pretty(&keystore)
            .map_err(|e| PrimitivesError::SignatureError(format!("Serialize keystore: {e}")))?;
        std::fs::write(path, data)
            .map_err(|e| PrimitivesError::SignatureError(format!("Write keystore: {e}")))?;
        Ok(())
    }

    /// Load a keypair from a keystore file.
    pub fn load_keystore(path: &std::path::Path, password: &str) -> Result<Self, PrimitivesError> {
        let data = std::fs::read_to_string(path)
            .map_err(|e| PrimitivesError::SignatureError(format!("Read keystore: {e}")))?;
        let keystore: serde_json::Value = serde_json::from_str(&data)
            .map_err(|e| PrimitivesError::SignatureError(format!("Parse keystore: {e}")))?;
        let ciphertext = keystore["crypto"]["ciphertext"]
            .as_str()
            .ok_or_else(|| PrimitivesError::SignatureError("Missing ciphertext".to_string()))?;
        let encrypted = hex::decode(ciphertext)
            .map_err(|e| PrimitivesError::SignatureError(format!("Decode ciphertext: {e}")))?;
        if encrypted.len() != 32 {
            return Err(PrimitivesError::SignatureError(
                "Invalid ciphertext length".to_string(),
            ));
        }
        let pw_hash = Keccak256::digest(password.as_bytes());
        let mut secret = [0u8; 32];
        for i in 0..32 {
            secret[i] = encrypted[i] ^ pw_hash[i];
        }
        Self::from_secret_key(&secret)
    }
}

/// Recover the signer's Ethereum address from a message hash and signature.
pub fn recover_signer(
    hash: &B256,
    v: u64,
    r: &[u8; 32],
    s: &[u8; 32],
) -> Result<Address, PrimitivesError> {
    let mut sig_bytes = [0u8; 64];
    sig_bytes[..32].copy_from_slice(r);
    sig_bytes[32..].copy_from_slice(s);

    let signature = Signature::from_bytes((&sig_bytes).into())
        .map_err(|e| PrimitivesError::SignatureError(format!("Invalid signature bytes: {e}")))?;

    // v is the recovery id: 0 or 1 (or 27/28 for legacy Ethereum)
    let recovery_id = if v >= 27 { v - 27 } else { v };
    let recid = RecoveryId::try_from(recovery_id as u8)
        .map_err(|e| PrimitivesError::SignatureError(format!("Invalid recovery id: {e}")))?;

    let verifying_key = VerifyingKey::recover_from_prehash(hash.as_ref(), &signature, recid)
        .map_err(|e| PrimitivesError::SignatureError(format!("Recovery failed: {e}")))?;

    Ok(public_key_to_address(&verifying_key))
}

/// Derive an Ethereum address from a secp256k1 public key.
/// Address = keccak256(uncompressed_pubkey_bytes[1..])[12..32]
fn public_key_to_address(key: &VerifyingKey) -> Address {
    let uncompressed = key.to_encoded_point(false);
    let pubkey_bytes = &uncompressed.as_bytes()[1..]; // skip the 0x04 prefix
    let hash = Keccak256::digest(pubkey_bytes);
    Address::from_slice(&hash[12..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keypair_random() {
        let kp = Keypair::random();
        assert_ne!(kp.address(), Address::ZERO);
    }

    #[test]
    fn test_sign_and_recover() {
        let kp = Keypair::random();
        let msg_hash = B256::from_slice(&Keccak256::digest(b"test message"));

        let (sig, recid) = kp.sign_hash(&msg_hash).unwrap();
        let sig_bytes = sig.to_bytes();
        let mut r = [0u8; 32];
        let mut s = [0u8; 32];
        r.copy_from_slice(&sig_bytes[..32]);
        s.copy_from_slice(&sig_bytes[32..]);
        let v = recid.to_byte() as u64;

        let recovered = recover_signer(&msg_hash, v, &r, &s).unwrap();
        assert_eq!(recovered, kp.address());
    }

    #[test]
    fn test_keypair_from_hex() {
        // Well-known test key
        let kp =
            Keypair::from_hex("ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80")
                .unwrap();
        // This is the first Hardhat test account
        assert_eq!(
            format!("{:?}", kp.address()).to_lowercase(),
            "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266"
        );
    }

    #[test]
    fn test_keypair_from_hex_with_prefix() {
        let kp =
            Keypair::from_hex("0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80")
                .unwrap();
        assert_eq!(
            format!("{:?}", kp.address()).to_lowercase(),
            "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266"
        );
    }
}
