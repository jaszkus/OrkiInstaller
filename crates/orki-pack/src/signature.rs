use ed25519_dalek::{Signature, Signer, Verifier, VerifyingKey};

use crate::format::{SIGNATURE_BLOCK_SIZE, SIGNATURE_MAGIC};
use crate::{DetectedFormat, FLAG_SIGNED, PackError};

pub const SIGNATURE_ALGO_ED25519: u8 = 1;
pub const SIGNATURE_SIZE: usize = 64;
pub const PUBKEY_SIZE: usize = 32;

pub type SigningKey = ed25519_dalek::SigningKey;

pub struct TrustedKey {
    pub slot: u8,
    pub pubkey: [u8; 32],
}

pub fn write_signature_block(
    sig: &[u8; SIGNATURE_SIZE],
    key_slot: u8,
) -> [u8; SIGNATURE_BLOCK_SIZE] {
    let mut block = [0u8; SIGNATURE_BLOCK_SIZE];
    block[0..8].copy_from_slice(&SIGNATURE_MAGIC);
    block[8] = SIGNATURE_ALGO_ED25519;
    block[9] = key_slot;
    block[12..76].copy_from_slice(sig);
    block
}

pub fn signed_message(
    data: &[u8],
    overlay: usize,
    header_len: usize,
    manifest_offset: u64,
    manifest_len: u64,
) -> Result<Vec<u8>, PackError> {
    let mstart = overlay
        .checked_add(manifest_offset as usize)
        .ok_or(PackError::Truncated)?;
    let mend = mstart
        .checked_add(manifest_len as usize)
        .ok_or(PackError::Truncated)?;
    let hend = overlay
        .checked_add(header_len)
        .ok_or(PackError::Truncated)?;
    if mend > data.len() || hend > data.len() {
        return Err(PackError::Truncated);
    }
    let mut msg = Vec::with_capacity(hend - overlay + manifest_len as usize);
    msg.extend_from_slice(&data[overlay..hend]);
    msg.extend_from_slice(&data[mstart..mend]);
    Ok(msg)
}

pub fn verify_payload_signature(data: &[u8], trusted: &[TrustedKey]) -> Result<(), PackError> {
    let detected = crate::detect(data)?;
    let (overlay, manifest_offset, manifest_len, sig_block_off, signed) = match &detected {
        DetectedFormat::V1 { overlay, header } => (
            *overlay,
            header.manifest_offset,
            header.manifest_len,
            overlay
                .checked_add(header.payload_len as usize)
                .ok_or(PackError::Truncated)?
                .saturating_sub(SIGNATURE_BLOCK_SIZE),
            header.flags & FLAG_SIGNED != 0,
        ),
        DetectedFormat::V0 { .. } => return Ok(()),
    };
    if !signed {
        return Err(PackError::UnsignedPayload);
    }
    if sig_block_off < overlay {
        return Err(PackError::Truncated);
    }
    if sig_block_off + SIGNATURE_BLOCK_SIZE > data.len() {
        return Err(PackError::Truncated);
    }
    let block = &data[sig_block_off..sig_block_off + SIGNATURE_BLOCK_SIZE];
    if block[0..8] != SIGNATURE_MAGIC {
        return Err(PackError::BadMagic);
    }
    if block[8] != SIGNATURE_ALGO_ED25519 {
        return Err(PackError::Manifest(format!(
            "unknown signature algorithm {}",
            block[8]
        )));
    }
    let key_slot = block[9];
    let key = trusted
        .iter()
        .find(|k| k.slot == key_slot)
        .ok_or(PackError::UnknownKeySlot(key_slot))?;
    let vk =
        VerifyingKey::from_bytes(&key.pubkey).map_err(|e| PackError::Manifest(e.to_string()))?;
    let sig =
        Signature::from_slice(&block[12..76]).map_err(|e| PackError::Manifest(e.to_string()))?;
    let msg = signed_message(data, overlay, 56, manifest_offset, manifest_len)?;
    vk.verify(&msg, &sig)
        .map_err(|_| PackError::SignatureMismatch)
}

pub fn sign_message(key: &SigningKey, header: &[u8], manifest: &[u8]) -> Signature {
    let mut msg = Vec::with_capacity(header.len() + manifest.len());
    msg.extend_from_slice(header);
    msg.extend_from_slice(manifest);
    key.sign(&msg)
}

pub fn pubkey_from_seed(seed: &[u8; 32]) -> [u8; 32] {
    SigningKey::from_bytes(seed).verifying_key().to_bytes()
}

pub fn append_signature(data: &mut Vec<u8>, key: &SigningKey, slot: u8) -> Result<(), PackError> {
    let (overlay, header) = crate::locate(data)?;
    if header.flags & FLAG_SIGNED != 0 {
        return Err(PackError::Manifest("payload is already signed".into()));
    }
    let payload_end = overlay
        .checked_add(header.payload_len as usize)
        .ok_or(PackError::Truncated)?;
    if payload_end != data.len() {
        return Err(PackError::Truncated);
    }
    data[overlay + 16..overlay + 24]
        .copy_from_slice(&(header.payload_len + SIGNATURE_BLOCK_SIZE as u64).to_le_bytes());
    data[overlay + 44..overlay + 48].copy_from_slice(&FLAG_SIGNED.to_le_bytes());
    let header_crc = crc32fast::hash(&data[overlay..overlay + 56]);
    data[overlay + 56..overlay + 60].copy_from_slice(&header_crc.to_le_bytes());
    let msg = signed_message(
        data,
        overlay,
        56,
        header.manifest_offset,
        header.manifest_len,
    )?;
    let sig = key.sign(&msg);
    data.extend_from_slice(&write_signature_block(&sig.to_bytes(), slot));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        TrustedKey, append_signature, pubkey_from_seed, sign_message, verify_payload_signature,
    };
    use crate::{AppMeta, FLAG_SIGNED, PackBuilder, PackError, SIGNATURE_BLOCK_SIZE};
    use ed25519_dalek::SigningKey;

    fn key_from_hex_seed(byte: u8) -> SigningKey {
        let seed = [byte; 32];
        SigningKey::from_bytes(&seed)
    }

    #[test]
    fn sign_and_verify_roundtrip() {
        let key = key_from_hex_seed(1);
        let mut b = PackBuilder::new(0);
        b.add_file("app.exe", b"hello installer").unwrap();
        let data = b.finish_signed(&AppMeta::new("a", "a", "0.1.0"), &key, 0);
        let trusted = [TrustedKey {
            slot: 0,
            pubkey: pubkey_from_seed(&[1; 32]),
        }];
        assert!(verify_payload_signature(&data, &trusted).is_ok());
    }

    #[test]
    fn unsigned_payload_rejected() {
        let mut b = PackBuilder::new(0);
        b.add_file("a.txt", b"x").unwrap();
        let data = b.finish(&AppMeta::new("a", "a", "0.1.0"));
        assert!(matches!(
            verify_payload_signature(&data, &[]),
            Err(PackError::UnsignedPayload)
        ));
    }

    #[test]
    fn tampered_manifest_rejected() {
        let key = key_from_hex_seed(2);
        let mut b = PackBuilder::new(0);
        b.add_file("a.txt", b"x").unwrap();
        let data = b.finish_signed(&AppMeta::new("a", "a", "0.1.0"), &key, 0);
        let (_, header) = crate::locate(&data).unwrap();
        let mut corrupt = data.clone();
        let mi = header.manifest_offset as usize + 30;
        corrupt[mi] ^= 0xff;
        let trusted = [TrustedKey {
            slot: 0,
            pubkey: pubkey_from_seed(&[2; 32]),
        }];
        assert!(matches!(
            verify_payload_signature(&corrupt, &trusted),
            Err(PackError::SignatureMismatch)
        ));
    }

    #[test]
    fn body_tamper_caught_by_chunk_hash_not_signature() {
        let key = key_from_hex_seed(3);
        let mut b = PackBuilder::new(0);
        b.add_file("a.txt", b"original").unwrap();
        let data = b.finish_signed(&AppMeta::new("a", "a", "0.1.0"), &key, 0);
        let (_, header) = crate::locate(&data).unwrap();
        let m = crate::read_manifest(&data).unwrap();
        let trusted = [TrustedKey {
            slot: 0,
            pubkey: pubkey_from_seed(&[3; 32]),
        }];
        let mut corrupt = data.clone();
        let body_pos = header.manifest_offset as usize - 1;
        corrupt[body_pos] ^= 0xff;
        assert!(verify_payload_signature(&corrupt, &trusted).is_ok());
        assert!(crate::extract_file(&corrupt, &m, &m.files[0]).is_err());
    }

    #[test]
    fn unknown_key_slot_rejected() {
        let key = key_from_hex_seed(4);
        let mut b = PackBuilder::new(0);
        b.add_file("a.txt", b"x").unwrap();
        let data = b.finish_signed(&AppMeta::new("a", "a", "0.1.0"), &key, 0);
        let trusted = [TrustedKey {
            slot: 7,
            pubkey: pubkey_from_seed(&[4; 32]),
        }];
        assert!(matches!(
            verify_payload_signature(&data, &trusted),
            Err(PackError::UnknownKeySlot(0))
        ));
    }

    #[test]
    fn wrong_key_rejected() {
        let key = key_from_hex_seed(5);
        let mut b = PackBuilder::new(0);
        b.add_file("a.txt", b"x").unwrap();
        let data = b.finish_signed(&AppMeta::new("a", "a", "0.1.0"), &key, 0);
        let trusted = [TrustedKey {
            slot: 0,
            pubkey: pubkey_from_seed(&[99; 32]),
        }];
        assert!(matches!(
            verify_payload_signature(&data, &trusted),
            Err(PackError::SignatureMismatch)
        ));
    }

    #[test]
    fn append_signature_flow() {
        let key = key_from_hex_seed(6);
        let mut b = PackBuilder::new(0);
        b.add_file("a.txt", b"x").unwrap();
        let mut data = b.finish(&AppMeta::new("a", "a", "0.1.0"));
        append_signature(&mut data, &key, 0).unwrap();
        let trusted = [TrustedKey {
            slot: 0,
            pubkey: pubkey_from_seed(&[6; 32]),
        }];
        assert!(verify_payload_signature(&data, &trusted).is_ok());
        assert!(matches!(
            append_signature(&mut data, &key, 0),
            Err(PackError::Manifest(_))
        ));
    }

    #[test]
    fn corrupt_signature_block_rejected() {
        let key = key_from_hex_seed(7);
        let mut b = PackBuilder::new(0);
        b.add_file("a.txt", b"x").unwrap();
        let mut data = b.finish_signed(&AppMeta::new("a", "a", "0.1.0"), &key, 0);
        let sig_pos = data.len() - SIGNATURE_BLOCK_SIZE + 20;
        data[sig_pos] ^= 0xff;
        let trusted = [TrustedKey {
            slot: 0,
            pubkey: pubkey_from_seed(&[7; 32]),
        }];
        assert!(matches!(
            verify_payload_signature(&data, &trusted),
            Err(PackError::SignatureMismatch)
        ));
    }

    #[test]
    fn signed_flag_visible_in_header() {
        let key = key_from_hex_seed(8);
        let mut b = PackBuilder::new(0);
        b.add_file("a.txt", b"x").unwrap();
        let data = b.finish_signed(&AppMeta::new("a", "a", "0.1.0"), &key, 0);
        let (_, header) = crate::locate(&data).unwrap();
        assert!(header.flags & FLAG_SIGNED != 0);
    }

    #[test]
    fn sign_message_is_deterministic() {
        let key = key_from_hex_seed(9);
        let header = [7u8; 64];
        let manifest = [1u8, 2, 3];
        let a = sign_message(&key, &header, &manifest).to_bytes();
        let b = sign_message(&key, &header, &manifest).to_bytes();
        assert_eq!(a, b);
    }
}
