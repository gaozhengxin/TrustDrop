use chacha20::cipher::{KeyIvInit, StreamCipher, StreamCipherSeek};
use chacha20::{ChaCha8, Key, Nonce};
use sha2::{Digest, Sha256};
use walrus_core::BlobId;

// --- [核心常量：对齐 Walrus 1000 节点 BFT 配置] ---
pub const ROW_WIDTH_PRIMARY: u32 = 334;    
pub const COL_HEIGHT_SECONDARY: u32 = 667; 
pub const DEFAULT_SAMPLE_COUNT: usize = 15; 
pub const SYMBOL_SIZE: usize = 1024;
// 关键探测结果：物理密钥流与逻辑 Symbol 的映射步长
pub const KEYSTREAM_SYMBOL_STEP: u64 = 64; 

// --- [数学底座：GF(2^8) 0x11d 标准表] ---
static GF_LOG: [u8; 256] = [
    0, 0, 1, 25, 2, 50, 26, 198, 3, 223, 51, 238, 27, 104, 199, 75, 4, 100, 224, 14, 52, 141, 239, 129, 28, 193, 105, 248, 200, 8, 76, 113, 5, 138, 101, 47, 225, 36, 15, 33, 53, 147, 142, 218, 240, 18, 130, 69, 29, 181, 194, 125, 106, 39, 249, 185, 201, 154, 9, 120, 77, 228, 114, 166, 6, 191, 139, 98, 102, 221, 48, 253, 226, 152, 37, 179, 16, 145, 34, 136, 54, 208, 148, 206, 143, 150, 219, 189, 241, 210, 19, 92, 131, 56, 70, 64, 30, 66, 182, 163, 195, 72, 126, 110, 107, 58, 40, 84, 250, 133, 186, 61, 202, 94, 155, 159, 10, 21, 121, 43, 78, 212, 229, 172, 115, 243, 167, 87, 7, 112, 192, 247, 140, 128, 99, 13, 103, 74, 222, 237, 49, 197, 254, 24, 227, 165, 153, 119, 38, 184, 180, 124, 17, 68, 146, 217, 35, 32, 137, 46, 55, 63, 209, 91, 149, 188, 207, 205, 144, 135, 151, 178, 220, 252, 190, 97, 242, 86, 211, 171, 20, 42, 93, 158, 132, 60, 57, 83, 71, 109, 65, 162, 31, 45, 67, 216, 183, 123, 164, 118, 196, 23, 73, 236, 127, 12, 111, 246, 108, 161, 59, 82, 41, 157, 85, 170, 251, 96, 134, 177, 187, 204, 62, 90, 203, 89, 95, 176, 156, 169, 160, 81, 11, 245, 22, 235, 122, 117, 44, 215, 79, 174, 213, 233, 230, 231, 173, 232, 116, 214, 244, 234, 168, 80, 88, 175,
];

static GF_EXP: [u8; 256] = [
    1, 2, 4, 8, 16, 32, 64, 128, 29, 58, 116, 232, 205, 135, 19, 38, 76, 152, 45, 90, 180, 117, 234, 201, 143, 3, 6, 12, 24, 48, 96, 192, 157, 39, 78, 156, 37, 74, 148, 53, 106, 212, 181, 119, 238, 193, 159, 35, 70, 140, 5, 10, 20, 40, 80, 160, 93, 186, 105, 210, 185, 111, 222, 161, 95, 190, 97, 194, 153, 47, 94, 188, 101, 202, 137, 15, 30, 60, 120, 240, 253, 231, 211, 187, 107, 214, 177, 127, 254, 225, 223, 163, 91, 182, 113, 226, 217, 175, 67, 134, 17, 34, 68, 136, 13, 26, 52, 104, 208, 189, 103, 206, 129, 31, 62, 124, 248, 237, 199, 147, 59, 118, 236, 197, 151, 51, 102, 204, 133, 23, 46, 92, 184, 109, 218, 169, 79, 158, 33, 66, 132, 21, 42, 84, 168, 77, 154, 41, 82, 164, 85, 170, 73, 146, 57, 114, 228, 213, 183, 115, 230, 209, 191, 99, 198, 145, 63, 126, 252, 229, 215, 179, 123, 246, 241, 255, 227, 219, 171, 75, 150, 49, 98, 196, 149, 55, 110, 220, 165, 87, 174, 65, 130, 25, 50, 100, 200, 141, 7, 14, 28, 56, 112, 224, 221, 167, 83, 166, 81, 162, 89, 178, 121, 242, 249, 239, 195, 155, 43, 86, 172, 69, 138, 9, 18, 36, 72, 144, 61, 122, 244, 245, 247, 243, 251, 235, 203, 139, 11, 22, 44, 88, 176, 125, 250, 233, 207, 131, 27, 54, 108, 216, 173, 71, 142, 1,
];

#[inline]
pub fn gf256_mul_walrus(a: u8, b: u8) -> u8 {
    if a == 0 || b == 0 { return 0; }
    let sum = (GF_LOG[a as usize] as u16 + GF_LOG[b as usize] as u16) % 255;
    GF_EXP[sum as usize]
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ParityType { Col, Row, None }

#[derive(Debug, Clone)]
pub struct VeShardProof {
    pub global_index: u32,
    pub col_index: u32,
    pub parity_offset: u32,
    pub origin_shard: Vec<u8>,
    pub cipher_shard: Vec<u8>,
    pub p_type: ParityType,
}

// --- [核心验证逻辑：完全对齐 64 步长物理布局] ---

pub fn verify_rslh_ve_combat(
    key: &[u8; 32],
    c_key: &[u8; 32],
    c_origin: &BlobId,
    c_cipher: &BlobId,
    aux_data: &[u8],
    total_shards: u32,
    proofs: &[VeShardProof],
) -> Result<(), &'static str> {
    let mut seed_h = Sha256::new();
    seed_h.update(c_origin.as_ref()); seed_h.update(c_cipher.as_ref()); seed_h.update(c_key);
    let seed = seed_h.finalize();
    let nonce = derive_rslh_nonce(key, aux_data);

    for (i, proof) in proofs.iter().enumerate() {
        let mut h = Sha256::new(); h.update(seed); h.update(&(i as u32).to_le_bytes());
        let expected_idx = u32::from_le_bytes(h.finalize()[0..4].try_into().unwrap()) % total_shards;
        if proof.global_index != expected_idx { return Err("RSLH_VE: INDEX_MISMATCH"); }

        verify_col_homomorphism(key, &nonce, proof)?;
    }
    Ok(())
}

pub fn verify_rslh_ve_combat_raw(
    key: &[u8; 32],
    c_key: &[u8; 32],
    c_origin_bytes: &[u8; 32],
    c_cipher_bytes: &[u8; 32],
    aux_data: &[u8],
    total_shards: u32,
    proofs: &[VeShardProof],
) -> Result<(), &'static str> {
    let c_origin = walrus_core::BlobId::try_from(&c_origin_bytes[..])
        .map_err(|_| "c_origin_cast_err")?;
    let c_cipher = walrus_core::BlobId::try_from(&c_cipher_bytes[..])
        .map_err(|_| "c_cipher_cast_err")?;

    verify_rslh_ve_combat(
        key,
        c_key,
        &c_origin,
        &c_cipher,
        aux_data,
        total_shards,
        proofs,
    )
}

fn verify_col_homomorphism(key: &[u8; 32], nonce: &[u8; 12], proof: &VeShardProof) -> Result<(), &'static str> {
    let mut p_ks = vec![0u8; SYMBOL_SIZE];
    let mut cipher = ChaCha8::new(Key::from_slice(key), Nonce::from_slice(nonce));

    for row in 0..COL_HEIGHT_SECONDARY {
        // 重要：逻辑索引 -> 物理密钥流偏移
        let logic_idx = row * ROW_WIDTH_PRIMARY + proof.col_index;
        let stream_idx = (logic_idx as u64) * KEYSTREAM_SYMBOL_STEP;
        
        let mut s_block = [0u8; SYMBOL_SIZE];
        cipher.seek(stream_idx * 16); // 16 blocks per symbol step (1024 bytes)
        cipher.apply_keystream(&mut s_block);
        
        let beta = GF_EXP[((row as u64 * (proof.parity_offset + 1) as u64) % 255) as usize];
        for i in 0..SYMBOL_SIZE {
            p_ks[i] ^= gf256_mul_walrus(s_block[i], beta);
        }
    }

    for i in 0..SYMBOL_SIZE {
        if (proof.origin_shard[i] ^ p_ks[i]) != proof.cipher_shard[i] {
            return Err("RSLH_VE: COL_HOMOMORPHISM_FAILURE");
        }
    }
    Ok(())
}

pub fn derive_rslh_nonce(key: &[u8; 32], aux_data: &[u8]) -> [u8; 12] {
    let mut hasher = Sha256::new();
    hasher.update(key); hasher.update(aux_data);
    let mut n = [0u8; 12]; n.copy_from_slice(&hasher.finalize()[0..12]);
    n
}

pub fn create_honest_proof(key: &[u8; 32], nonce: &[u8; 12], idx: u32, origin: &[u8], cipher: &[u8]) -> VeShardProof {
    let col_idx = idx % ROW_WIDTH_PRIMARY;
    let mut p_m = vec![0u8; SYMBOL_SIZE];
    let mut p_c = vec![0u8; SYMBOL_SIZE];
    let mut chacha = ChaCha8::new(Key::from_slice(key), Nonce::from_slice(nonce));

    for row in 0..COL_HEIGHT_SECONDARY {
        let logic_idx = row * ROW_WIDTH_PRIMARY + col_idx;
        let stream_idx = (logic_idx as u64) * KEYSTREAM_SYMBOL_STEP;
        let data_pos = (logic_idx as usize) * SYMBOL_SIZE;
        let beta = GF_EXP[((row as u64 * 1) % 255) as usize];

        if data_pos < origin.len() {
            let end = (data_pos + SYMBOL_SIZE).min(origin.len());
            let len = end - data_pos;
            for j in 0..len {
                p_m[j] ^= gf256_mul_walrus(origin[data_pos + j], beta);
                p_c[j] ^= gf256_mul_walrus(cipher[data_pos + j], beta);
            }
        } else {
            let mut s = [0u8; SYMBOL_SIZE];
            chacha.seek(stream_idx * 16);
            chacha.apply_keystream(&mut s);
            for j in 0..SYMBOL_SIZE { p_c[j] ^= gf256_mul_walrus(s[j], beta); }
        }
    }
    VeShardProof {
        global_index: idx, col_index: col_idx, parity_offset: 0,
        origin_shard: p_m, cipher_shard: p_c, p_type: ParityType::Col
    }
}

// --- [自动化测试：成功用例 + 失败用例] ---

#[cfg(test)]
mod tests {
    use super::*;
    use core::num::NonZeroU16;
    use walrus_core::encoding::{EncodingConfig, EncodingFactory as _};

    pub fn compute_blob_id(data: &[u8], n_shards: u16) -> Result<BlobId, walrus_core::encoding::DataTooLargeError> {
        let n_shards = NonZeroU16::new(n_shards).expect("n_shards must be > 0");
        let config = EncodingConfig::new(n_shards);
        let encoding_config = config.get_for_type(walrus_core::EncodingType::RS2);
        let metadata_with_id = encoding_config.compute_metadata(data)?;
        Ok(*metadata_with_id.blob_id())
    }

    #[test]
    fn test_combat() {
        use std::time::Instant;
        use rand::{RngCore, thread_rng};

        // 定义测试档次
        let tiers = vec![
            ("100B", 100),
            ("1MB", 1024 * 1024),
            ("1GB", 1024 * 1024 * 1024),
        ];

        println!("\n{:<10} | {:<15} | {:<15}", "Size", "Setup Time", "Verify Time (Avg)");
        println!("{:-<45}", "");

        for (label, size) in tiers {
            // --- 1. Setup 阶段 (不计入验证时间) ---
            let setup_start = Instant::now();
            
            let mut data = vec![0u8; size];
            thread_rng().fill_bytes(&mut data);
            
            // 计算 Blob ID (这一步在 1GB 时会很慢)
            let c_origin = compute_blob_id(&data, 1000).unwrap();
            
            let mut key = [0u8; 32];
            thread_rng().fill_bytes(&mut key);
            let c_key = Sha256::digest(&key).into();
            
            let aux = b"benchmark_v2";
            let nonce = derive_rslh_nonce(&key, aux);
            
            // 加密数据
            let mut cipher_data = data.clone();
            ChaCha8::new(Key::from_slice(&key), Nonce::from_slice(&nonce)).apply_keystream(&mut cipher_data);
            let c_cipher = compute_blob_id(&cipher_data, 1000).unwrap();

            // 生成证据 (模拟 Prover 的工作)
            let mut seed_h = Sha256::new();
            seed_h.update(c_origin.as_ref()); seed_h.update(c_cipher.as_ref()); seed_h.update(c_key);
            let seed = seed_h.finalize();

            let proofs: Vec<VeShardProof> = (0..DEFAULT_SAMPLE_COUNT).map(|i| {
                let mut h = Sha256::new();
                h.update(seed);
                h.update(&(i as u32).to_le_bytes());
                let idx = u32::from_le_bytes(h.finalize()[0..4].try_into().unwrap()) % 1000;
                create_honest_proof(&key, &nonce, idx, &data, &cipher_data)
            }).collect();
            
            let setup_duration = setup_start.elapsed();

            // --- 2. 核心验证阶段 (这是 SP1 VM 实际运行的部分) ---
            // 执行 50 次取平均值以消除系统误差
            let iterations = 50;
            let verify_start = Instant::now();
            for _ in 0..iterations {
                let _ = verify_rslh_ve_combat(&key, &c_key, &c_origin, &c_cipher, aux, 1000, &proofs);
            }
            let avg_verify_time = verify_start.elapsed() / iterations;

            println!("{:<10} | {:<15?} | {:<15?}", label, setup_duration, avg_verify_time);

            // 验证正确性
            assert!(verify_rslh_ve_combat(&key, &c_key, &c_origin, &c_cipher, aux, 1000, &proofs).is_ok());
        }
    }

    #[test]
    fn test_combat_tamper_failure() {
        use rand::{RngCore, thread_rng};

        println!("Testing Tamper Failure Detection...");
        let size = 100 * 1024;
        let mut data = vec![0u8; size]; thread_rng().fill_bytes(&mut data);
        let c_origin = compute_blob_id(&data, 1000).unwrap();
        let mut key = [0u8; 32]; thread_rng().fill_bytes(&mut key);
        let c_key = Sha256::digest(&key).into();
        let aux = b"combat_v2";
        let nonce = derive_rslh_nonce(&key, aux);
        let mut cipher_data = data.clone();
        ChaCha8::new(Key::from_slice(&key), Nonce::from_slice(&nonce)).apply_keystream(&mut cipher_data);
        let c_cipher = compute_blob_id(&cipher_data, 1000).unwrap();

        let mut seed_h = Sha256::new();
        seed_h.update(c_origin.as_ref()); seed_h.update(c_cipher.as_ref()); seed_h.update(c_key);
        let seed = seed_h.finalize();

        let mut proofs: Vec<VeShardProof> = (0..DEFAULT_SAMPLE_COUNT).map(|i| {
            let mut h = Sha256::new(); h.update(seed); h.update(&(i as u32).to_le_bytes());
            let idx = u32::from_le_bytes(h.finalize()[0..4].try_into().unwrap()) % 1000;
            create_honest_proof(&key, &nonce, idx, &data, &cipher_data)
        }).collect();

        // --- 篡改第 0 个证据：修改 origin_shard 的一个字节 ---
        proofs[0].origin_shard[5] ^= 0xFF;

        let res = verify_rslh_ve_combat(&key, &c_key, &c_origin, &c_cipher, aux, 1000, &proofs);
        
        // 断言验证必须失败
        assert!(res.is_err(), "Security Failure: Verification should have failed due to tampering!");
        assert_eq!(res.unwrap_err(), "RSLH_VE: COL_HOMOMORPHISM_FAILURE");
        println!("SUCCESS: Tampering detected correctly.");
    }
}