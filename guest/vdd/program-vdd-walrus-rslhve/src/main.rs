#![no_main]
sp1_zkvm::entrypoint!(main);

use maenad_lib::rslh_ve::{
    verify_rslh_ve_combat_raw, 
    VeShardProof, 
    ParityType, 
    DEFAULT_SAMPLE_COUNT,
    ROW_WIDTH_PRIMARY
};

pub fn main() {
    // 1. 读取基础参数 (固定长度使用 read, 动态长度使用 read::<Vec<u8>>)
    let c_origin_bytes = sp1_zkvm::io::read::<[u8; 32]>();
    let c_cipher_bytes = sp1_zkvm::io::read::<[u8; 32]>();
    let c_key = sp1_zkvm::io::read::<[u8; 32]>();
    let aux_data = sp1_zkvm::io::read::<Vec<u8>>();
    let key = sp1_zkvm::io::read::<[u8; 32]>();

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

    // 3. 核心验证
    if let Err(e) = verify_rslh_ve_combat_raw(
        &key,
        &c_key,
        &c_origin_bytes,
        &c_cipher_bytes,
        &aux_data,
        1000, 
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