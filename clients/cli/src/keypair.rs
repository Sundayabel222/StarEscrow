/// Native Stellar keypair operations using ed25519-dalek.
///
/// Replaces subprocess calls to `stellar keys` for:
///   - Keypair generation
///   - Deriving a public address (G...) from a secret key (S...)
///   - Signing arbitrary payloads (e.g. transaction hashes)
///   - Verifying signatures
use anyhow::{Context, Result};
use ed25519_dalek::{Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use stellar_strkey::ed25519::{PrivateKey as StrkeySecret, PublicKey as StrkeyPublic};

/// A Stellar keypair wrapping an ed25519 signing key.
pub struct Keypair {
    signing_key: SigningKey,
}

impl Keypair {
    /// Generate a fresh random keypair using the OS CSPRNG.
    pub fn generate() -> Self {
        let signing_key = SigningKey::generate(&mut OsRng);
        Self { signing_key }
    }

    /// Parse a Stellar secret key string (S...) into a Keypair.
    pub fn from_secret_str(secret: &str) -> Result<Self> {
        let strkey = StrkeySecret::from_string(secret)
            .context("invalid Stellar secret key (expected S... strkey)")?;
        let signing_key = SigningKey::from_bytes(&strkey.0);
        Ok(Self { signing_key })
    }

    /// Return the Stellar-encoded secret key (S...).
    pub fn secret_key_str(&self) -> String {
        StrkeySecret(self.signing_key.to_bytes()).to_string().to_string()
    }

    /// Return the Stellar-encoded public address (G...).
    pub fn public_key_str(&self) -> String {
        let vk: VerifyingKey = self.signing_key.verifying_key();
        StrkeyPublic(vk.to_bytes()).to_string().to_string()
    }

    /// Sign a payload (e.g. a 32-byte transaction hash) and return the 64-byte signature.
    pub fn sign(&self, payload: &[u8]) -> [u8; 64] {
        self.signing_key.sign(payload).to_bytes()
    }

    /// Verify a signature produced by this keypair's public key.
    pub fn verify(&self, payload: &[u8], signature_bytes: &[u8; 64]) -> Result<()> {
        let sig = ed25519_dalek::Signature::from_bytes(signature_bytes);
        self.signing_key
            .verifying_key()
            .verify(payload, &sig)
            .context("signature verification failed")
    }
}

/// Derive a Stellar public address (G...) from a secret key string (S...) natively,
/// without spawning a subprocess.
pub fn public_address_from_secret(secret: &str) -> Result<String> {
    Ok(Keypair::from_secret_str(secret)?.public_key_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_produces_valid_strkeys() {
        let kp = Keypair::generate();
        let secret = kp.secret_key_str();
        let public = kp.public_key_str();
        assert!(secret.starts_with('S'), "secret key should start with S, got: {secret}");
        assert!(public.starts_with('G'), "public key should start with G, got: {public}");
    }

    #[test]
    fn test_roundtrip_from_secret_str() {
        let kp1 = Keypair::generate();
        let secret = kp1.secret_key_str();
        let kp2 = Keypair::from_secret_str(&secret).expect("should parse own secret");
        assert_eq!(kp1.public_key_str(), kp2.public_key_str(), "public keys must match after roundtrip");
    }

    #[test]
    fn test_sign_and_verify() {
        let kp = Keypair::generate();
        let payload = b"stellar transaction hash 32 bytes!";
        let sig = kp.sign(payload);
        kp.verify(payload, &sig).expect("own signature must verify");
    }

    #[test]
    fn test_verify_rejects_tampered_payload() {
        let kp = Keypair::generate();
        let payload = b"original payload data here 32byt";
        let sig = kp.sign(payload);
        let tampered = b"tampered payload data here 32byt";
        assert!(kp.verify(tampered, &sig).is_err(), "tampered payload must not verify");
    }

    #[test]
    fn test_verify_rejects_wrong_key() {
        let kp1 = Keypair::generate();
        let kp2 = Keypair::generate();
        let payload = b"some transaction payload 32 bytes";
        let sig = kp1.sign(payload);
        assert!(kp2.verify(payload, &sig).is_err(), "signature from kp1 must not verify with kp2");
    }

    #[test]
    fn test_public_address_from_secret() {
        let kp = Keypair::generate();
        let derived = public_address_from_secret(&kp.secret_key_str())
            .expect("should derive address from own secret");
        assert_eq!(derived, kp.public_key_str());
    }

    #[test]
    fn test_invalid_secret_returns_error() {
        assert!(Keypair::from_secret_str("not-a-valid-key").is_err());
        assert!(Keypair::from_secret_str("GABCDEF").is_err()); // public key, not secret
    }
}
