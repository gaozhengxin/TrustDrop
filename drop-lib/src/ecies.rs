use anyhow::{Result, anyhow};
use k256::{PublicKey, SecretKey};
use k256::ecdh::{diffie_hellman, EphemeralSecret};
use k256::elliptic_curve::sec1::ToEncodedPoint;
use k256::elliptic_curve::rand_core::{OsRng}; 
use sha2::{Digest, Sha256};

/// 物理加密：买家使用卖家公钥加密 vssKey
pub fn encrypt(recipient_pubkey_bytes: &[u8], secret: &[u8; 32]) -> Result<([u8; 32], Vec<u8>)> {
    let recipient_pubk = PublicKey::from_sec1_bytes(recipient_pubkey_bytes)
        .map_err(|e| anyhow!("Invalid recipient public key: {}", e))?;

    let mut rng = OsRng;
    let ephemeral_sk = EphemeralSecret::random(&mut rng);
    let ephemeral_pk = ephemeral_sk.public_key();

    let shared_secret = ephemeral_sk.diffie_hellman(&recipient_pubk);
    let mask = Sha256::digest(shared_secret.raw_secret_bytes());
    
    let mut ciphertext = [0u8; 32];
    for i in 0..32 {
        ciphertext[i] = secret[i] ^ mask[i];
    }

    Ok((ciphertext, ephemeral_pk.to_encoded_point(true).as_bytes().to_vec()))
}

/// 物理解密：卖家使用私钥和临时公钥解密
pub fn decrypt(my_sk_bytes: &[u8; 32], ciphertext: &[u8; 32], ephemeral_pubkey_bytes: &[u8]) -> Result<[u8; 32]> {
    let my_sk = SecretKey::from_slice(my_sk_bytes)
        .map_err(|e| anyhow!("Invalid secret key: {}", e))?;
    
    let ephemeral_pk = PublicKey::from_sec1_bytes(ephemeral_pubkey_bytes)
        .map_err(|e| anyhow!("Invalid ephemeral public key: {}", e))?;

    let shared_secret = diffie_hellman(my_sk.to_nonzero_scalar(), ephemeral_pk.as_affine());
    let mask = Sha256::digest(shared_secret.raw_secret_bytes());

    let mut plaintext = [0u8; 32];
    for i in 0..32 {
        plaintext[i] = ciphertext[i] ^ mask[i];
    }

    Ok(plaintext)
}