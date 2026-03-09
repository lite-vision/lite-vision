use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;

pub struct KeyPair {
    signing_key: SigningKey,
    verifying_key: VerifyingKey,
}

impl KeyPair {
    pub fn generate() -> Self {
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();
        Self {
            signing_key,
            verifying_key,
        }
    }

    pub fn from_seed(seed: &[u8; 32]) -> Self {
        let signing_key = SigningKey::from_bytes(seed.into());
        let verifying_key = signing_key.verifying_key();
        Self {
            signing_key,
            verifying_key,
        }
    }

    pub fn verify(&self, message: &[u8], signature: &[u8; 64]) -> bool {
        let sig = Signature::from_slice(signature).unwrap();
        self.verifying_key.verify(message, &sig).is_ok()
    }

    pub fn sign(&self, message: &[u8]) -> [u8; 64] {
        let sig = self.signing_key.sign(message);
        sig.to_bytes()
    }

    pub fn public_key(&self) -> [u8; 32] {
        self.verifying_key.to_bytes()
    }
}

pub fn hash(data: &[u8]) -> [u8; 32] {
    use blake3::Hasher;
    let mut hasher = Hasher::new();
    hasher.update(data);
    *hasher.finalize().as_bytes()
}

pub fn verify_signature(pubkey: &[u8; 32], message: &[u8], signature: &[u8; 64]) -> bool {
    let vk = VerifyingKey::from_bytes(pubkey.into()).unwrap();
    let sig = Signature::from_slice(signature).unwrap();
    vk.verify(message, &sig).is_ok()
}
