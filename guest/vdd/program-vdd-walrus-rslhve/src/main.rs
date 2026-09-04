#![no_main]
sp1_zkvm::entrypoint!(main);

use drop_lib::rslh_ve::{
    verify_rslh_ve_combat_raw, walrus_symbol_size, ParityType, VeShardProof, DEFAULT_SAMPLE_COUNT,
    MIN_VDD_BLOB_BYTES, ROW_WIDTH_PRIMARY,
};
use drop_lib::walrus_open::{
    verify_cipher_column_bound, verify_origin_column_bound, verify_walrus_blob_opening,
    WalrusBlobOpening,
};

pub fn main() {
    // 1. 读取基础参数 (固定长度使用 read, 动态长度使用 read::<Vec<u8>>)
    let c_origin_bytes = sp1_zkvm::io::read::<[u8; 32]>();
    let c_cipher_bytes = sp1_zkvm::io::read::<[u8; 32]>();
    let c_key = sp1_zkvm::io::read::<[u8; 32]>();
    let aux_data = sp1_zkvm::io::read::<Vec<u8>>();
    let key = sp1_zkvm::io::read::<[u8; 32]>();

    let expected_c_key: [u8; 32] = *blake3::hash(&key).as_bytes();
    if c_key != expected_c_key {
        panic!("RSLH-VE Failure: c_key does not match blake3(key)");
    }

    // 2. 读取证据
    let mut proofs = Vec::with_capacity(DEFAULT_SAMPLE_COUNT);
    for _ in 0..DEFAULT_SAMPLE_COUNT {
        let global_index = sp1_zkvm::io::read::<u32>();
        let origin_shard = sp1_zkvm::io::read::<Vec<u8>>();
        let cipher_shard = sp1_zkvm::io::read::<Vec<u8>>();

        proofs.push(VeShardProof {
            global_index,
            col_index: global_index % ROW_WIDTH_PRIMARY,
            parity_offset: 0,
            origin_shard,
            cipher_shard,
            p_type: ParityType::Col,
        });
    }

    // 2.5 原文和密文分别打开到各自的公开 Walrus blob id。
    let origin_opening: WalrusBlobOpening = sp1_zkvm::io::read();
    let cipher_opening: WalrusBlobOpening = sp1_zkvm::io::read();
    if let Err(e) = verify_walrus_blob_opening(&origin_opening) {
        panic!("Origin Walrus-Open Failure: {}", e);
    }
    if let Err(e) = verify_walrus_blob_opening(&cipher_opening) {
        panic!("Cipher Walrus-Open Failure: {}", e);
    }
    if origin_opening.blob_id != c_origin_bytes {
        panic!("Walrus-Binding Failure: origin opening does not match c_origin");
    }
    if cipher_opening.blob_id != c_cipher_bytes {
        panic!("Walrus-Binding Failure: cipher opening does not match c_cipher");
    }
    if origin_opening.unencoded_length != cipher_opening.unencoded_length {
        panic!("VDD Length Failure: origin and cipher lengths differ");
    }
    if origin_opening.unencoded_length < MIN_VDD_BLOB_BYTES {
        panic!("VDD Length Failure: asset is smaller than 1 MiB");
    }

    // 2.75 同态关系两端的聚合分别绑定到两个公开 blob commitment。
    let symbol_size = walrus_symbol_size(origin_opening.unencoded_length);
    if let Err(e) = verify_origin_column_bound(&origin_opening, &proofs, symbol_size) {
        panic!("RSLH-Origin-Binding Failure: {}", e);
    }
    if let Err(e) = verify_cipher_column_bound(&key, &aux_data, &cipher_opening, &proofs, symbol_size) {
        panic!("RSLH-Walrus-Binding Failure: {}", e);
    }

    // 3. 核心验证
    if let Err(e) = verify_rslh_ve_combat_raw(
        &key,
        &c_key,
        &c_origin_bytes,
        &c_cipher_bytes,
        &aux_data,
        1000,
        symbol_size,
        &proofs,
    ) {
        panic!("RSLH-VE Failure: {}", e);
    }

    // 4. Commit
    let mut public_values = Vec::new();
    public_values.extend_from_slice(&c_origin_bytes);
    public_values.extend_from_slice(&c_key);
    public_values.extend_from_slice(&c_cipher_bytes);
    sp1_zkvm::io::commit_slice(&public_values);
}
