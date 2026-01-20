#![no_main]
sp1_zkvm::entrypoint!(main);

use maenad_lib::rslh_ve::{
    verify_rslh_ve_combat_raw,
    VeShardProof, 
    ParityType,
    ROW_WIDTH_PRIMARY,
    DEFAULT_SAMPLE_COUNT,
};
use sha2::{Digest, Sha256};

pub fn main() {
    use sp1_zkvm::io::{commit_slice, read_vec};

    // --- 1. 读取公开输入 (原始字节) ---
    let c_origin_bytes: [u8; 32] = read_vec_to_array();
    let c_cipher_bytes: [u8; 32] = read_vec_to_array();
    let c_key: [u8; 32] = read_vec_to_array();
    let aux_data = read_vec();

    // --- 2. 读取隐私输入 (原始字节) ---
    let key: [u8; 32] = read_vec_to_array();

    // --- 3. 内部生成索引并读取 Shards ---
    // 为了防止 Host 欺骗，在 VM 内部派生随机种子
    let seed = {
        let mut h = Sha256::new();
        h.update(&c_origin_bytes); 
        h.update(&c_cipher_bytes); 
        h.update(&c_key);
        h.finalize()
    };

    let mut proofs = Vec::with_capacity(DEFAULT_SAMPLE_COUNT);

    for i in 0..DEFAULT_SAMPLE_COUNT {
        let mut h = Sha256::new();
        h.update(&seed);
        h.update(&(i as u32).to_le_bytes());
        let expected_idx = u32::from_le_bytes(h.finalize()[0..4].try_into().unwrap()) % 1000;
        
        let col_idx = expected_idx % ROW_WIDTH_PRIMARY;

        // 仅通过基本类型 read_vec 读取 Shards
        let origin_shard = read_vec(); 
        let cipher_shard = read_vec();

        proofs.push(VeShardProof {
            global_index: expected_idx,
            col_index: col_idx,
            parity_offset: 0,
            origin_shard,
            cipher_shard,
            p_type: ParityType::Col,
        });
    }

    // --- 4. 调用 lib 里的转换接口 ---
    // 所有的 BlobId 转换都在这个函数内部，Guest 不感知
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

    // --- 5. 提交结果 ---
    let mut combined = Vec::with_capacity(96);
    combined.extend_from_slice(&c_origin_bytes);
    combined.extend_from_slice(&c_key);
    combined.extend_from_slice(&c_cipher_bytes);
    commit_slice(&combined);
}

/// 辅助函数：处理 read_vec 到 [u8; 32] 的转换，兼容 8 字节长度头
fn read_vec_to_array() -> [u8; 32] {
    let raw = sp1_zkvm::io::read_vec();
    if raw.len() == 40 {
        raw[8..].try_into().unwrap()
    } else {
        raw.try_into().expect("Expected 32 bytes")
    }
}