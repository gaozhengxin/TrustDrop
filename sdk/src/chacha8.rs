use anyhow::Result;
use chacha20::cipher::{KeyIvInit, StreamCipher, StreamCipherSeek};
use chacha20::ChaCha8;

pub fn chacha8_encrypt(
    data: &[u8],
    key: &[u8; 32],
    nonce: &[u8; 12],
    counter: u32,
) -> Result<Vec<u8>> {
    let mut cipher = ChaCha8::new(key.into(), nonce.into());
    cipher.seek(counter as u64);
    let mut buffer = data.to_vec();
    cipher.apply_keystream(&mut buffer);
    Ok(buffer)
}

pub fn chacha8_decrypt(
    data: &[u8],
    key: &[u8; 32],
    nonce: &[u8; 12],
    counter: u32,
) -> Result<Vec<u8>> {
    chacha8_encrypt(data, key, nonce, counter)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let key = [7u8; 32];
        let nonce = [3u8; 12];
        let plaintext = b"trustdrop sdk chacha8 roundtrip";

        let ciphertext = chacha8_encrypt(plaintext, &key, &nonce, 0).unwrap();
        assert_ne!(ciphertext, plaintext);

        let recovered = chacha8_decrypt(&ciphertext, &key, &nonce, 0).unwrap();
        assert_eq!(recovered, plaintext);
    }

    #[test]
    fn counter_changes_keystream() {
        let key = [7u8; 32];
        let nonce = [3u8; 12];
        let plaintext = b"same plaintext";

        let a = chacha8_encrypt(plaintext, &key, &nonce, 0).unwrap();
        let b = chacha8_encrypt(plaintext, &key, &nonce, 1).unwrap();

        assert_ne!(a, b);
    }
}
