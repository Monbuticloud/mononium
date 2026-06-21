//! Equivocation detection and slashing logic.
//!
//! Provides [`EquivocationEvidence`] and the 5-check verification routine
//! defined in Phase 2.4 of the Mononium specification.

use parity_scale_codec::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::core::block::BlockHeader;
use crate::crypto::falcon::{Falcon512, Falcon512PublicKey, Falcon512Signature};
use crate::crypto::signature::SignatureScheme;
use crate::error::LibError;

// ---------------------------------------------------------------------------
// EquivocationEvidence
// ---------------------------------------------------------------------------

/// Two signed blocks at the same height and parent, proving equivocation.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, Serialize, Deserialize)]
pub struct EquivocationEvidence {
    pub header_a: BlockHeader,
    pub signature_a: Falcon512Signature,
    pub header_b: BlockHeader,
    pub signature_b: Falcon512Signature,
    pub proposer: [u8; 32],
}

/// Verify equivocation evidence against the proposer's public key.
///
/// The 5 checks (all must pass):
/// 1. `header_a.height == header_b.height`
/// 2. `header_a.parent_hash == header_b.parent_hash`
/// 3. `header_a != header_b` (different block hashes)
/// 4. `falcon_verify(pk, SCALE(header_a), sig_a)`
/// 5. `falcon_verify(pk, SCALE(header_b), sig_b)`
///
/// # Errors
///
/// Returns the specific [`LibError`] variant for the first failing check.
pub fn verify_equivocation(
    evidence: &EquivocationEvidence,
    public_key: &[u8; 897],
) -> std::result::Result<(), LibError> {
    let a = &evidence.header_a;
    let b = &evidence.header_b;

    // 1. Same height
    if a.height != b.height {
        return Err(LibError::EquivocationHeightMismatch(a.height, b.height));
    }

    // 2. Same parent hash
    if a.parent_hash != b.parent_hash {
        return Err(LibError::EquivocationParentMismatch);
    }

    // 3. Different blocks (hash comparison)
    let hash_a = blake3::hash(&a.encode());
    let hash_b = blake3::hash(&b.encode());
    if hash_a == hash_b {
        return Err(LibError::EquivocationIdenticalBlocks);
    }

    // 4. Signature A valid
    let pk = Falcon512PublicKey(*public_key);
    if !Falcon512::verify(&pk, &a.encode(), &evidence.signature_a) {
        return Err(LibError::EquivocationSigAInvalid);
    }

    // 5. Signature B valid
    if !Falcon512::verify(&pk, &b.encode(), &evidence.signature_b) {
        return Err(LibError::EquivocationSigBInvalid);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::account::Address;
    use crate::crypto::falcon::{Falcon512, Falcon512KeyPair};
    use crate::crypto::signature::SignatureScheme;

    fn header(height: u64, parent: u8) -> BlockHeader {
        BlockHeader {
            height,
            parent_hash: [parent; 32],
            global_state_root: [0; 32],
            tx_root: [0; 32],
            timestamp: 1_700_000_000,
            proposer: Address::from([0x01; 32]),
            chain_id: 0,
        }
    }

    fn dummy_keypair() -> (Vec<u8>, Falcon512KeyPair) {
        let seed = [0xABu8; 48];
        let kp = Falcon512::generate(&seed).unwrap();
        let pk = Falcon512::public_key(&kp);
        (pk.as_ref().to_vec(), kp)
    }

    fn sign_header(kp: &Falcon512KeyPair, header: &BlockHeader) -> Falcon512Signature {
        let encoded = header.encode();
        Falcon512::sign(kp, &encoded).unwrap()
    }

    // -----------------------------------------------------------------------
    // construction
    // -----------------------------------------------------------------------

    #[test]
    fn test_equivocation_evidence_scale_roundtrip() {
        let h1 = header(1, 0xAA);
        let h2 = header(1, 0xAA);
        let (_, f5) = dummy_keypair();
        let evidence = EquivocationEvidence {
            header_a: h1,
            signature_a: sign_header(&f5, &header(1, 0xAA)),
            header_b: h2,
            signature_b: sign_header(&f5, &header(1, 0xAA)),
            proposer: [0x01; 32],
        };
        let encoded = evidence.encode();
        let decoded = EquivocationEvidence::decode(&mut &encoded[..]).unwrap();
        assert_eq!(evidence, decoded);
    }

    // -----------------------------------------------------------------------
    // verify_equivocation — 5 checks
    // -----------------------------------------------------------------------

    #[test]
    fn test_verify_valid_evidence_accepted() {
        let (pk_bytes, kp) = dummy_keypair();
        let pk: [u8; 897] = pk_bytes.try_into().unwrap();
        let h1 = header(5, 0xBB);
        let mut h2 = header(5, 0xBB);
        h2.tx_root = [0xCC; 32]; // different body → different block hash

        let evidence = EquivocationEvidence {
            header_a: h1.clone(),
            signature_a: sign_header(&kp, &h1),
            header_b: h2.clone(),
            signature_b: sign_header(&kp, &h2),
            proposer: [0x01; 32],
        };
        assert!(verify_equivocation(&evidence, &pk).is_ok());
    }

    #[test]
    fn test_verify_height_mismatch() {
        let (pk_bytes, _) = dummy_keypair();
        let pk: [u8; 897] = pk_bytes.try_into().unwrap();
        // Can't use valid sigs for headers at different heights, so
        // we use bogus sigs — the height check happens first.
        let h1 = header(5, 0xBB);
        let h2 = header(6, 0xBB); // different height

        let evidence = EquivocationEvidence {
            header_a: h1,
            signature_a: Falcon512Signature::from_bytes(&[0u8; 809]).unwrap(),
            header_b: h2,
            signature_b: Falcon512Signature::from_bytes(&[0u8; 809]).unwrap(),
            proposer: [0x01; 32],
        };
        assert_eq!(
            verify_equivocation(&evidence, &pk),
            Err(LibError::EquivocationHeightMismatch(5, 6))
        );
    }

    #[test]
    fn test_verify_parent_hash_mismatch() {
        let (pk_bytes, _) = dummy_keypair();
        let pk: [u8; 897] = pk_bytes.try_into().unwrap();
        // Bogus sigs — parent hash check runs before sig verification.
        let h1 = header(5, 0xBB);
        let h2 = header(5, 0xCC); // different parent hash

        let evidence = EquivocationEvidence {
            header_a: h1,
            signature_a: Falcon512Signature::from_bytes(&[0u8; 809]).unwrap(),
            header_b: h2,
            signature_b: Falcon512Signature::from_bytes(&[0u8; 809]).unwrap(),
            proposer: [0x01; 32],
        };
        assert_eq!(
            verify_equivocation(&evidence, &pk),
            Err(LibError::EquivocationParentMismatch)
        );
    }

    #[test]
    fn test_verify_identical_blocks() {
        let (pk_bytes, kp) = dummy_keypair();
        let pk: [u8; 897] = pk_bytes.try_into().unwrap();
        let h1 = header(5, 0xBB);
        let h2 = header(5, 0xBB); // identical to h1

        let evidence = EquivocationEvidence {
            header_a: h1,
            signature_a: sign_header(&kp, &header(5, 0xBB)),
            header_b: h2,
            signature_b: sign_header(&kp, &header(5, 0xBB)),
            proposer: [0x01; 32],
        };
        assert_eq!(
            verify_equivocation(&evidence, &pk),
            Err(LibError::EquivocationIdenticalBlocks)
        );
    }

    #[test]
    fn test_verify_invalid_sig_a() {
        let (pk_bytes, f5) = dummy_keypair();
        let pk: [u8; 897] = pk_bytes.try_into().unwrap();
        let mut h2 = header(5, 0xBB);
        h2.tx_root = [0xCC; 32];

        let evidence = EquivocationEvidence {
            header_a: header(5, 0xBB),
            signature_a: Falcon512Signature::from_bytes(&[0u8; 809]).unwrap(),
            header_b: h2,
            signature_b: sign_header(&f5, &header(5, 0xBB)),
            proposer: [0x01; 32],
        };
        assert_eq!(
            verify_equivocation(&evidence, &pk),
            Err(LibError::EquivocationSigAInvalid)
        );
    }

    #[test]
    fn test_verify_invalid_sig_b() {
        let (pk_bytes, f5) = dummy_keypair();
        let pk: [u8; 897] = pk_bytes.try_into().unwrap();
        let mut h2 = header(5, 0xBB);
        h2.tx_root = [0xCC; 32];

        let evidence = EquivocationEvidence {
            header_a: header(5, 0xBB),
            signature_a: sign_header(&f5, &header(5, 0xBB)),
            header_b: h2,
            signature_b: Falcon512Signature::from_bytes(&[0u8; 809]).unwrap(),
            proposer: [0x01; 32],
        };
        assert_eq!(
            verify_equivocation(&evidence, &pk),
            Err(LibError::EquivocationSigBInvalid)
        );
    }
}
