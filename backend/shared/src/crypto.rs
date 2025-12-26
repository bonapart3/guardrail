//! Cryptographic utilities for GuardRail
//!
//! Includes hash chain implementation for tamper-evident event logging.

use sha2::{Digest, Sha256};
use serde::{Deserialize, Serialize};

/// Genesis hash used as the first "previous hash" in the chain
pub const GENESIS_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// Compute SHA-256 hash of data and return as hex string
pub fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

/// Compute hash for an event in the chain
///
/// The hash is computed from:
/// - sequence_number
/// - event_type
/// - actor_id
/// - payload (JSON string)
/// - previous_hash
/// - timestamp
pub fn compute_event_hash(
    sequence_number: i64,
    event_type: &str,
    actor_id: &str,
    payload: &str,
    previous_hash: &str,
    timestamp: &str,
) -> String {
    let data = format!(
        "{}:{}:{}:{}:{}:{}",
        sequence_number, event_type, actor_id, payload, previous_hash, timestamp
    );
    sha256_hex(data.as_bytes())
}

/// Verify that an event's hash is valid given its data and previous hash
pub fn verify_event_hash(
    sequence_number: i64,
    event_type: &str,
    actor_id: &str,
    payload: &str,
    previous_hash: &str,
    timestamp: &str,
    expected_hash: &str,
) -> bool {
    let computed = compute_event_hash(
        sequence_number,
        event_type,
        actor_id,
        payload,
        previous_hash,
        timestamp,
    );
    computed == expected_hash
}

/// Merkle tree node for anchoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleNode {
    pub hash: String,
    pub left: Option<Box<MerkleNode>>,
    pub right: Option<Box<MerkleNode>>,
}

/// Build a Merkle tree from a list of event hashes
pub fn build_merkle_tree(hashes: &[String]) -> Option<MerkleNode> {
    if hashes.is_empty() {
        return None;
    }

    // Create leaf nodes
    let mut nodes: Vec<MerkleNode> = hashes
        .iter()
        .map(|h| MerkleNode {
            hash: h.clone(),
            left: None,
            right: None,
        })
        .collect();

    // If odd number, duplicate last
    if nodes.len() % 2 == 1 {
        if let Some(last) = nodes.last().cloned() {
            nodes.push(last);
        }
    }

    // Build tree bottom-up
    while nodes.len() > 1 {
        let mut next_level = Vec::new();

        for chunk in nodes.chunks(2) {
            let left = chunk[0].clone();
            let right = chunk.get(1).cloned().unwrap_or_else(|| left.clone());

            let combined = format!("{}{}", left.hash, right.hash);
            let parent_hash = sha256_hex(combined.as_bytes());

            next_level.push(MerkleNode {
                hash: parent_hash,
                left: Some(Box::new(left)),
                right: Some(Box::new(right)),
            });
        }

        nodes = next_level;
    }

    nodes.into_iter().next()
}

/// Get the Merkle root hash from a list of event hashes
pub fn compute_merkle_root(hashes: &[String]) -> Option<String> {
    build_merkle_tree(hashes).map(|node| node.hash)
}

/// A Merkle proof for verifying inclusion of an event in an anchored batch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleProof {
    pub event_hash: String,
    pub proof_hashes: Vec<ProofElement>,
    pub merkle_root: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofElement {
    pub hash: String,
    pub position: ProofPosition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProofPosition {
    Left,
    Right,
}

/// Generate a Merkle proof for a specific event hash
pub fn generate_merkle_proof(hashes: &[String], target_index: usize) -> Option<MerkleProof> {
    if target_index >= hashes.len() || hashes.is_empty() {
        return None;
    }

    let event_hash = hashes[target_index].clone();
    let mut proof_hashes = Vec::new();
    let mut current_hashes = hashes.to_vec();
    let mut index = target_index;

    // Pad to even length
    if current_hashes.len() % 2 == 1 {
        if let Some(last) = current_hashes.last().cloned() {
            current_hashes.push(last);
        }
    }

    while current_hashes.len() > 1 {
        let sibling_index = if index.is_multiple_of(2) { index + 1 } else { index - 1 };
        let position = if index.is_multiple_of(2) {
            ProofPosition::Right
        } else {
            ProofPosition::Left
        };

        proof_hashes.push(ProofElement {
            hash: current_hashes[sibling_index].clone(),
            position,
        });

        // Compute next level
        let mut next_level = Vec::new();
        for chunk in current_hashes.chunks(2) {
            let combined = format!("{}{}", chunk[0], chunk[1]);
            next_level.push(sha256_hex(combined.as_bytes()));
        }

        current_hashes = next_level;
        if current_hashes.len() % 2 == 1 && current_hashes.len() > 1 {
            if let Some(last) = current_hashes.last().cloned() {
                current_hashes.push(last);
            }
        }
        index /= 2;
    }

    Some(MerkleProof {
        event_hash,
        proof_hashes,
        merkle_root: current_hashes[0].clone(),
    })
}

/// Verify a Merkle proof
pub fn verify_merkle_proof(proof: &MerkleProof) -> bool {
    let mut current_hash = proof.event_hash.clone();

    for element in &proof.proof_hashes {
        let combined = match element.position {
            ProofPosition::Left => format!("{}{}", element.hash, current_hash),
            ProofPosition::Right => format!("{}{}", current_hash, element.hash),
        };
        current_hash = sha256_hex(combined.as_bytes());
    }

    current_hash == proof.merkle_root
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256_hex() {
        let hash = sha256_hex(b"hello");
        assert_eq!(
            hash,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn test_event_hash_chain() {
        let hash1 = compute_event_hash(1, "POLICY_DECISION", "user1", "{}", GENESIS_HASH, "2024-01-01T00:00:00Z");
        let hash2 = compute_event_hash(2, "POLICY_DECISION", "user2", "{}", &hash1, "2024-01-01T00:01:00Z");
        
        assert!(verify_event_hash(1, "POLICY_DECISION", "user1", "{}", GENESIS_HASH, "2024-01-01T00:00:00Z", &hash1));
        assert!(verify_event_hash(2, "POLICY_DECISION", "user2", "{}", &hash1, "2024-01-01T00:01:00Z", &hash2));
    }

    #[test]
    fn test_merkle_tree() {
        let hashes = vec![
            sha256_hex(b"event1"),
            sha256_hex(b"event2"),
            sha256_hex(b"event3"),
            sha256_hex(b"event4"),
        ];

        let root = compute_merkle_root(&hashes);
        assert!(root.is_some());

        // Generate and verify proof for each event
        for (i, hash) in hashes.iter().enumerate() {
            let proof = generate_merkle_proof(&hashes, i).unwrap();
            assert_eq!(proof.event_hash, *hash);
            assert!(verify_merkle_proof(&proof));
        }
    }
}
