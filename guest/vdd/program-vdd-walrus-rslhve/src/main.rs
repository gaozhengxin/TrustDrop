#![no_main]
sp1_zkvm::entrypoint!(main);

use drop_lib::rslh_ve::{
    verify_rslh_ve_combat_raw, walrus_symbol_size, ParityType, VeShardProof, DEFAULT_SAMPLE_COUNT,
    ROW_WIDTH_PRIMARY,
};
use drop_lib::walrus_open::{
    verify_cipher_column_bound, verify_walrus_blob_opening, WalrusBlobOpening,
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

    // 2.5 读取并验证 Walrus 承诺打开：把采样数据绑定到密文 blob id
    let blob_opening: WalrusBlobOpening = sp1_zkvm::io::read();
    if let Err(e) = verify_walrus_blob_opening(&blob_opening) {
        panic!("Walrus-Open Failure: {}", e);
    }
    if blob_opening.blob_id != c_cipher_bytes {
        panic!("Walrus-Binding Failure: opening blob id does not match cipher commitment");
    }

    // 2.75 绑定：RSLH 列证明的 cipher_shard 必须与真实密文列聚合一致
    let symbol_size = walrus_symbol_size(blob_opening.unencoded_length);
    if let Err(e) = verify_cipher_column_bound(&key, &aux_data, &blob_opening, &proofs, symbol_size) {
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
