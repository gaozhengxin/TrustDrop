#![no_main]
sp1_zkvm::entrypoint!(main);

use drop_lib::chacha8::chacha8_seal;
use drop_lib::walrus_address::compute_blob_id_default;
use drop_lib::{ chacha8 };

pub fn main() {
    use blake3;
    use sp1_zkvm::io::{ commit_slice, read_vec };

    // ================ public inputs ================
    // data commitment
    let c_origin_raw: Vec<u8> = read_vec();
    let c_origin = &c_origin_raw;
    eprintln!("c_origin (from host) hex: {}", hex::encode(c_origin));
    // chacha8 key commitment
    let c_key_raw = read_vec();
    let c_key = &c_key_raw[8..];
    // cipher commitment
    let c_cipher_raw: Vec<u8> = read_vec();
    let c_cipher = &c_cipher_raw[8..];
    // ================================================

    // ================ private inputs ================
    // origin data
    let origin_raw: Vec<u8> = read_vec();
    let origin = &origin_raw[8..];
    // chacha8 key
    let key_raw = read_vec();
    let key: &[u8] = &key_raw[8..];
    // ================================================

    let data_size: u32 = origin.len().try_into().unwrap();

    // ============ check data commitment =============
    let binding: blake3::Hash = blake3::hash(&origin);
    let h_origin = binding.as_bytes();
    if c_origin != h_origin {
        use hex;
        let len = origin.len();
        let head = hex::encode(&origin[..16]);
        let tail = hex::encode(&origin[len - 16..]);
        eprintln!("origin len: {}, head: {}, tail: {}", len, head, tail);
        panic!(
            "{}",
            format!(
                "origin data commitment mismatch, commitment: {}, hash: {}",
                hex::encode(c_origin),
                hex::encode(h_origin)
            )
        );
    }
    // ================================================

    // ============= check key commitment =============
    let binding = blake3::hash(&key);
    let h_key = binding.as_bytes();
    if c_key != h_key {
        use hex;
        panic!(
            "{}",
            format!(
                "key commitment mismatch, commitment: {}, key: {}, hash: {}",
                hex::encode(c_key),
                hex::encode(key),
                hex::encode(h_key)
            )
        );
    }
    // ================================================

    // =========== check cipher commitment ============
    let key_arr_ref: &[u8; 32] = key.try_into().expect("key must be 32 bytes");
    let cipher = chacha8_seal(origin, key_arr_ref, c_origin).expect("data encryption failed");
    let cipher_blob_id = compute_blob_id_default(&cipher).expect(
        "Should compute blob ID for cipher data"
    );

    if c_cipher != cipher_blob_id.as_ref() {
        use hex;
        let len = cipher.len();
        let head = hex::encode(&cipher[..16]);
        let tail = hex::encode(&cipher[len - 16..]);
        eprintln!("cipher len: {}, head: {}, tail: {}", len, head, tail);
        eprintln!(
            "{}",
            format!(
                "cipher commitment mismatch, c_cipher: {}, blob_id: {}",
                hex::encode(c_cipher),
                hex::encode(cipher_blob_id)
            )
        );
        panic!(
            "{}",
            format!(
                "cipher commitment mismatch, c_cipher: {}, blob_id: {}",
                hex::encode(c_cipher),
                hex::encode(cipher_blob_id)
            )
        );
    }
    // ================================================

    // commitment
    let mut combined: Vec<u8> = Vec::with_capacity(128);
    combined.extend_from_slice(&c_origin);
    combined.extend_from_slice(&c_key);
    combined.extend_from_slice(&c_cipher);
    combined.extend_from_slice(&data_size.to_be_bytes());

    commit_slice(&combined);
}
