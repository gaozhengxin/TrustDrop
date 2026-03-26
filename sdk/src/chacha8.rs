use anyhow::Result;
use chacha20::cipher::{KeyIvInit, StreamCipher, StreamCipherSeek};
use chacha20::ChaCha8; 

pub fn chacha8_encrypt(data: &[u8], key: &[u8; 32], nonce: &[u8; 12], counter: u32) -> Result<Vec<u8>> {
    let mut cipher = ChaCha8::new(key.into(), nonce.into());
    cipher.seek(counter as u64);
    let mut buffer = data.to_vec();
    cipher.apply_keystream(&mut buffer);
    Ok(buffer)
}

pub fn chacha8_decrypt(data: &[u8], key: &[u8; 32], nonce: &[u8; 12], counter: u32) -> Result<Vec<u8>> {
    chacha8_encrypt(data, key, nonce, counter)
}