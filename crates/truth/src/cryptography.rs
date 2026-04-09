use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};

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
    let vk = match VerifyingKey::from_bytes(pubkey.into()) {
        Ok(vk) => vk,
        Err(_) => return false,
    };
    let sig = match Signature::from_slice(signature) {
        Ok(sig) => sig,
        Err(_) => return false,
    };
    vk.verify(message, &sig).is_ok()
}

pub fn hash_serializable<T: Serialize>(value: &T) -> [u8; 32] {
    let encoded = bincode::serialize(value).unwrap();
    hash(&encoded)
}

pub fn verify_serializable_signature<T: Serialize>(
    pubkey: &[u8; 32],
    value: &T,
    signature: &[u8; 64],
) -> bool {
    let encoded = bincode::serialize(value).unwrap();
    verify_signature(pubkey, &encoded, signature)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleProof {
    pub leaf_hash: [u8; 32],
    pub path: Vec<MerkleNode>,
    pub root_hash: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleNode {
    pub hash: [u8; 32],
    pub is_left: bool,
}

pub struct MerkleTree {
    leaves: Vec<[u8; 32]>,
    nodes: Vec<[u8; 32]>,
    height: usize,
}

impl MerkleTree {
    pub fn new(leaves: Vec<[u8; 32]>) -> Self {
        if leaves.is_empty() {
            return Self {
                leaves: vec![],
                nodes: vec![],
                height: 0,
            };
        }

        let height = (leaves.len() as f64).log2().ceil() as usize + 1;
        let mut levels: Vec<Vec<[u8; 32]>> = Vec::new();
        levels.push(leaves.clone());

        let mut current_level = leaves.clone();
        while current_level.len() > 1 {
            let mut next_level = Vec::new();
            for chunk in current_level.chunks(2) {
                let left = chunk[0];
                let right = if chunk.len() > 1 { chunk[1] } else { chunk[0] };
                next_level.push(combine_hashes(left, right));
            }
            current_level = next_level;
            levels.push(current_level.clone());
        }

        let root = levels.last().unwrap().first().copied();
        let nodes: Vec<[u8; 32]> = levels.into_iter().flatten().collect();

        Self {
            leaves,
            nodes,
            height,
        }
    }

    pub fn root(&self) -> Option<[u8; 32]> {
        self.nodes.last().copied()
    }

    pub fn proof(&self, index: usize) -> Option<MerkleProof> {
        if index >= self.leaves.len() {
            return None;
        }

        let leaf_hash = self.leaves[index];
        let root = self.root()?;

        let mut path = Vec::new();
        let mut level_leaves = self.leaves.clone();
        let mut current_idx = index;

        while level_leaves.len() > 1 {
            let is_left = current_idx % 2 == 1;
            let sibling_idx = if is_left {
                current_idx + 1
            } else {
                if current_idx > 0 {
                    current_idx - 1
                } else {
                    current_idx
                }
            };

            let sibling = if sibling_idx < level_leaves.len() && sibling_idx != current_idx {
                level_leaves[sibling_idx]
            } else {
                level_leaves[current_idx]
            };

            path.push(MerkleNode {
                hash: sibling,
                is_left,
            });

            current_idx = current_idx / 2;
            let next_level_len = (level_leaves.len() + 1) / 2;
            level_leaves = level_leaves
                .chunks(2)
                .map(|c| combine_hashes(c[0], if c.len() > 1 { c[1] } else { c[0] }))
                .collect();
        }

        Some(MerkleProof {
            leaf_hash,
            path,
            root_hash: root,
        })
    }

    pub fn verify_proof(proof: &MerkleProof) -> bool {
        let mut current = proof.leaf_hash;
        for node in &proof.path {
            current = if node.is_left {
                combine_hashes(node.hash, current)
            } else {
                combine_hashes(current, node.hash)
            };
        }
        current == proof.root_hash
    }
}

fn combine_hashes(left: [u8; 32], right: [u8; 32]) -> [u8; 32] {
    use blake3::Hasher;
    let mut hasher = Hasher::new();
    hasher.update(&left);
    hasher.update(&right);
    *hasher.finalize().as_bytes()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyRotationRecord {
    pub old_key: [u8; 32],
    pub new_key: [u8; 32],
    pub height: u64,
    pub rotation_type: KeyRotationType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyRotationType {
    Regular,
    Emergency,
    Slashing,
}

pub struct KeyManager {
    current_key: [u8; 32],
    rotation_history: Vec<KeyRotationRecord>,
}

impl KeyManager {
    pub fn new(initial_key: [u8; 32]) -> Self {
        Self {
            current_key: initial_key,
            rotation_history: Vec::new(),
        }
    }

    pub fn rotate(&mut self, new_key: [u8; 32], height: u64, rotation_type: KeyRotationType) {
        let record = KeyRotationRecord {
            old_key: self.current_key,
            new_key,
            height,
            rotation_type,
        };
        self.rotation_history.push(record);
        self.current_key = new_key;
    }

    pub fn current_key(&self) -> [u8; 32] {
        self.current_key
    }

    pub fn get_key_at_height(&self, height: u64) -> Option<[u8; 32]> {
        let mut key = self.current_key;
        for record in self.rotation_history.iter().rev() {
            if record.height <= height {
                return Some(key);
            }
            key = record.old_key;
        }
        Some(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keypair_generate_and_sign() {
        let keypair = KeyPair::generate();
        let message = b"test message";
        let signature = keypair.sign(message);
        assert!(keypair.verify(message, &signature));
    }

    #[test]
    fn test_keypair_from_seed() {
        let seed = [42u8; 32];
        let keypair1 = KeyPair::from_seed(&seed);
        let keypair2 = KeyPair::from_seed(&seed);

        let message = b"test";
        assert_eq!(keypair1.sign(message), keypair2.sign(message));
    }

    #[test]
    fn test_verify_signature() {
        let keypair = KeyPair::generate();
        let message = b"test message";
        let signature = keypair.sign(message);

        let pubkey: [u8; 32] = keypair.public_key();
        assert!(verify_signature(&pubkey, message, &signature));
    }

    #[test]
    fn test_hash_serializable() {
        let data = (42u64, "test".to_string(), vec![1, 2, 3]);
        let hash1 = hash_serializable(&data);
        let hash2 = hash_serializable(&data);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_merkle_tree() {
        let leaves: Vec<[u8; 32]> = (0..8).map(|i| hash(&[i])).collect();
        let tree = MerkleTree::new(leaves.clone());

        let root = tree.root();
        assert!(root.is_some());

        let expected_root = compute_merkle_root(&leaves);
        assert_eq!(root.unwrap(), expected_root);
    }

    fn compute_merkle_root(leaves: &[[u8; 32]]) -> [u8; 32] {
        if leaves.is_empty() {
            return [0u8; 32];
        }
        if leaves.len() == 1 {
            return leaves[0];
        }

        let mut level = leaves.to_vec();
        while level.len() > 1 {
            let mut next = Vec::new();
            for chunk in level.chunks(2) {
                let h = combine_hashes(chunk[0], if chunk.len() > 1 { chunk[1] } else { chunk[0] });
                next.push(h);
            }
            level = next;
        }
        level[0]
    }

    #[test]
    fn test_key_rotation() {
        let key1 = [1u8; 32];
        let key2 = [2u8; 32];
        let key3 = [3u8; 32];

        let mut manager = KeyManager::new(key1);
        assert_eq!(manager.current_key(), key1);

        manager.rotate(key2, 100, KeyRotationType::Regular);
        assert_eq!(manager.current_key(), key2);

        manager.rotate(key3, 200, KeyRotationType::Emergency);
        assert_eq!(manager.current_key(), key3);

        assert_eq!(manager.get_key_at_height(50), Some(key1));
        assert_eq!(manager.get_key_at_height(150), Some(key2));
        assert_eq!(manager.get_key_at_height(250), Some(key3));
    }
}
