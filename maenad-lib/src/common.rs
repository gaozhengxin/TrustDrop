use alloy_sol_types::sol;

use core::result::Result;

use crate::chacha8;

use hex;
use std::convert::TryInto;
use std::error::Error;
use std::fmt;

sol! {
    /// ZK Program 的公开输出值结构，用于 Solidity 合约验证。
    struct PublicZKOutputsWithCipher {
        uint8 length; // Chacha8 密钥数量
        bytes32 hOrigBlock;      // 原文块的哈希 (H_ORIG Block)
        bytes32[] cipherBlock;    // 密文块
        bytes32[] hKCommitment;    // ChaCha8 密钥的承诺哈希 (H_K)
        bytes12[] nonce;          // ChaCha8 加密使用的 Nonce
    }
}

// ZK Program 1 输出的固定哈希长度
const HASH_LEN: usize = 32;
const BLOCK_LEN: usize = 32;

#[derive(Debug, Clone)]
pub struct DecodedZKOutputsWithCipher {
    pub length: u8,
    // 原文块的哈希 (H_ORIG Block)
    pub h_orig_block: [u8; HASH_LEN],
    /// 密文块的哈希 (H(Ciphertext Block))
    pub cipher_block: Vec<[u8; BLOCK_LEN]>,
    /// 对称加密密钥的承诺哈希 (H_K)
    pub h_k_commitment: Vec<[u8; HASH_LEN]>,
    /// Chacha8 加密使用的 Nonce
    pub nonce: Vec<[u8; 12]>,
}

impl DecodedZKOutputsWithCipher {
    pub fn decryptContent(&self, keys: Vec<[u8; 32]>) -> Result<Vec<Vec<u8>>, &'static str> {
        let mut decrypted = Vec::new();

        const INITIAL_COUNTER: u32 = 0;

        for i in 0..self.length as usize {
            let decrypted_block = chacha8::chacha8_decrypt(
                &self.cipher_block[i],
                &keys[i],
                &self.nonce[i],
                INITIAL_COUNTER,
            )?;

            decrypted.push(decrypted_block);
        }

        Ok(decrypted)
    }
}

// --- 自定义错误类型 ---
#[derive(Debug)]
pub enum DecodingError {
    InvalidLength,
    IoError(String),
}

impl fmt::Display for DecodingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecodingError::InvalidLength => write!(f, "Invalid output length: expected {} bytes.", 3 * HASH_LEN),
            DecodingError::IoError(e) => write!(f, "I/O error during decoding: {}", e),
        }
    }
}

impl Error for DecodingError {}

// ─────────────────────────────────────────────────────────────────────────────
// 解码函数：直接输出密文块的版本
// ─────────────────────────────────────────────────────────────────────────────
pub fn decode_public_outputs_with_cipher(
    output_bytes: &[u8],
) -> Result<DecodedZKOutputsWithCipher, DecodingError> {
    let mut cursor = 0usize;

    let length = output_bytes[cursor + 31];

    cursor += 32;

    let h_orig_block: [u8; 32] = output_bytes[cursor..cursor + 32].try_into().unwrap();
    cursor += 32;

    // cipherBlock[]
    let mut pos = cursor;
    let array_len = usize::from_be_bytes(output_bytes[pos + 24..pos + 32].try_into().unwrap());
    pos += 32;
    assert_eq!(array_len, length as usize);
    let mut cipher_block = vec![[0u8; 32]; length as usize];
    for i in 0..length as usize {
        cipher_block[i].copy_from_slice(&output_bytes[pos..pos + 32]);
        pos += 32;
    }

    // hKCommitment[]
    let array_len = usize::from_be_bytes(output_bytes[pos + 24..pos + 32].try_into().unwrap());
    pos += 32;
    let mut h_k_commitment = vec![[0u8; 32]; length as usize];
    for i in 0..length as usize {
        h_k_commitment[i].copy_from_slice(&output_bytes[pos..pos + 32]);
        pos += 32;
    }

    // nonce[]
    let array_len = usize::from_le_bytes(output_bytes[pos + 24..pos + 32].try_into().unwrap());
    pos += 32;
    let mut nonce = vec![[0u8; 12]; length as usize];
    for i in 0..length as usize {
        nonce[i].copy_from_slice(&output_bytes[pos..pos + 12]);
        pos += 32;
    }

    Ok(DecodedZKOutputsWithCipher {
        length,
        h_orig_block,
        cipher_block,
        h_k_commitment,
        nonce,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// 打印函数（带明文密文块版）
// ─────────────────────────────────────────────────────────────────────────────
pub fn print_public_outputs_with_cipher(decoded: &DecodedZKOutputsWithCipher) {
    println!("\n╔════════════════════════════════════╗");
    println!("║       ZK Program Public Output       ║");
    println!("╚════════════════════════════════════╝");
    println!("Length (number of keys/blocks) : {}", decoded.length);
    println!("H_ORIG_BLOCK                   : 0x{}", hex::encode(decoded.h_orig_block));
    println!("────────────────────────────────────");
    for (i, c) in decoded.cipher_block.iter().enumerate() {
        println!("CipherBlock[{i}]                 : 0x{}", hex::encode(c));
    }
    println!("────────────────────────────────────");
    for (i, h) in decoded.h_k_commitment.iter().enumerate() {
        println!("hKCommitment[{i}]                : 0x{}", hex::encode(h));
    }
    println!("────────────────────────────────────");
    for (i, n) in decoded.nonce.iter().enumerate() {
        println!("nonce[{i}]                       : 0x{}", hex::encode(n));
    }
}
/*
0000000000000000000000000000000000000000000000000000000000000004  32
5c7d17a31fa4989f6e52ebb3ccc673d859324cac5227008eb5f919b837a682b2  64
0000000000000000000000000000000000000000000000000000000000000004  96
206b922bbb9358ee545f32b105c82c63852cd7646143967b47c80fee3dda065f  128
46f4b45955bed5fe563740c31988f262f0773fbd5c5131f971f1a10d4cf3f83e  160
842765892617140c30a56c5176575f28f8c8c7f5e7c4ba8dcbb725bd5a3ba99c  192
44f0c10c000ac9b17abb4e56772222d3bfbaab39748c178497c2119212334d67  224
0000000000000000000000000000000000000000000000000000000000000004  256
076cee4f5c927dd17e4491e882b8a6ebf706da5de472047fb85fac3963e88e12  288
8c4ddf743f70d0b95e4114edc3770bfb1e89b9f4e60573fe36e2ad2c5fa0b004  320
08821c2c560b579c20f0ba49bcced98b370eafb52c482c4430420284f206570d  352
d1274eea40a4328634383cd872f5675655aaac092b45fbd1a842cf3a60f2e2d6  384
0000000000000000000000000000000000000000000000000000000000000004  416
22e762ee23d8c3ceb8ad35bf0000000000000000000000000000000000000000  448
bb1ec6926ff18455775a057d0000000000000000000000000000000000000000  480
bf609b5f4d9cbb4e2ad4eccc0000000000000000000000000000000000000000  512
f20d8260674a29f898933e3f0000000000000000000000000000000000000000  544
*/