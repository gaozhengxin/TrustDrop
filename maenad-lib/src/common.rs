use alloy_sol_types::sol;

use core::result::Result;

use hex;
use std::convert::TryInto;
use std::error::Error;
use std::fmt;

sol! {
    /// ZK Program 1 的公开输出值结构，用于 Solidity 合约验证。
    /// 字段顺序严格匹配 ZKVM 的 io::commit_vec 输出顺序。
    struct PublicZKOutputsWithHash {
        bytes32 hOrigBlock;      // 原文块的哈希 (H_ORIG Block)
        bytes32 hCipherBlock;    // 密文块的哈希
        bytes32 hKCommitment;    // ChaCha8 密钥的承诺哈希 (H_K)
    }

    struct PublicZKOutputsWithCipher {
        bytes32 hOrigBlock;      // 原文块的哈希 (H_ORIG Block)
        bytes32 cipherBlock;    // 密文块的哈希
        bytes32 hKCommitment;    // ChaCha8 密钥的承诺哈希 (H_K)
    }
}

// ZK Program 1 输出的固定哈希长度
const HASH_LEN: usize = 32;
const BLOCK_LEN: usize = 32;

// --- 1. 输出数据结构 ---
/// ZK Program 的公开输出结构，用于 Rust 端的解码和处理。
#[derive(Debug, Clone)]
pub struct DecodedZKOutputsWithHash {
    // 原文块的哈希 (H_ORIG Block)
    pub h_orig_block: [u8; HASH_LEN],
    /// 密文块的哈希 (H(Ciphertext Block))
    pub h_cipher_block: [u8; HASH_LEN],
    /// ChaCha8 密钥的承诺哈希 (H_K)
    pub h_k_commitment: [u8; HASH_LEN],
}

#[derive(Debug, Clone)]
pub struct DecodedZKOutputsWithCipher {
    // 原文块的哈希 (H_ORIG Block)
    pub h_orig_block: [u8; HASH_LEN],
    /// 密文块的哈希 (H(Ciphertext Block))
    pub cipher_block: [u8; BLOCK_LEN],
    /// 对称加密密钥的承诺哈希 (H_K)
    pub h_k_commitment: [u8; HASH_LEN],
}

// --- 2. 自定义错误类型 ---
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


// --- 3. 解码函数 (Decode Logic) ---
/// 将 ZKVM 提交的原始字节流解码为 DecodedZKOutputsWithHash 结构体。
pub fn decode_public_outputs_with_hash(output_bytes: &[u8]) -> Result<DecodedZKOutputsWithHash, DecodingError> {
    const TOTAL_LEN: usize = 3 * HASH_LEN;

    if output_bytes.len() < TOTAL_LEN {
        return Err(DecodingError::InvalidLength);
    }

    let mut cursor = 0;

    // 1. 解码 hOrigBlock (64..96)
    let h_orig_block_slice: [u8; HASH_LEN] = output_bytes[cursor..cursor + HASH_LEN]
        .try_into()
        .map_err(|e| DecodingError::IoError(format!("Failed to read hOrigBlock: {:?}", e)))?;
    cursor += HASH_LEN;

    // 2. 解码 hCipherBlock (0..32)
    let h_cipher_block_slice: [u8; HASH_LEN] = output_bytes[cursor..cursor + HASH_LEN]
        .try_into()
        .map_err(|e| DecodingError::IoError(format!("Failed to read hCipherBlock: {:?}", e)))?;
    cursor += HASH_LEN;

    // 3. 解码 hKCommitment (32..64)
    let h_k_commitment_slice: [u8; HASH_LEN] = output_bytes[cursor..cursor + HASH_LEN]
        .try_into()
        .map_err(|e| DecodingError::IoError(format!("Failed to read hKCommitment: {:?}", e)))?;

    Ok(DecodedZKOutputsWithHash {
        h_orig_block: h_orig_block_slice,
        h_cipher_block: h_cipher_block_slice,
        h_k_commitment: h_k_commitment_slice,
    })
}

// --- 3. 解码函数 (Decode Logic) ---
/// 将 ZKVM 提交的原始字节流解码为 DecodedZKOutputsWithHash 结构体。
pub fn decode_public_outputs_with_cipher(output_bytes: &[u8]) -> Result<DecodedZKOutputsWithCipher, DecodingError> {
    //const TOTAL_LEN: usize = 3 * HASH_LEN;
    const TOTAL_LEN: usize = 2 * HASH_LEN;

    if output_bytes.len() < TOTAL_LEN {
        return Err(DecodingError::InvalidLength);
    }

    let mut cursor = 0;

    // 1. 解码 hOrigBlock (64..96)
    let h_orig_block_slice: [u8; HASH_LEN] = output_bytes[cursor..cursor + HASH_LEN]
        .try_into()
        .map_err(|e| DecodingError::IoError(format!("Failed to read hOrigBlock: {:?}", e)))?;
    cursor += HASH_LEN;

    // 2. 解码 cipherBlock (0..32)
    let cipher_block_slice: [u8; HASH_LEN] = output_bytes[cursor..cursor + HASH_LEN]
        .try_into()
        .map_err(|e| DecodingError::IoError(format!("Failed to read hCipherBlock: {:?}", e)))?;
    cursor += HASH_LEN;

    // 3. 解码 hKCommitment (32..64)
    let h_k_commitment_slice: [u8; HASH_LEN] = output_bytes[cursor..cursor + HASH_LEN]
        .try_into()
        .map_err(|e| DecodingError::IoError(format!("Failed to read hKCommitment: {:?}", e)))?;

    Ok(DecodedZKOutputsWithCipher {
        h_orig_block: h_orig_block_slice,
        cipher_block: cipher_block_slice,
        h_k_commitment: h_k_commitment_slice,
    })
}

// --- 4. 打印函数 (Output Logic) ---
/// 格式化并打印解码后的 ZK Program 输出。
pub fn print_public_outputs_with_hash(decoded_output: &DecodedZKOutputsWithHash) {
    println!("\n=== ZK Program Public Output ===");
    println!("1. H_ORIG_BLOCK (原文块哈希):   {}", hex::encode(decoded_output.h_orig_block));
    println!("2. H_CIPHER_BLOCK (密文块哈希): {}", hex::encode(decoded_output.h_cipher_block));
    println!("3. H_K_COMMITMENT (密钥承诺哈希): {}", hex::encode(decoded_output.h_k_commitment));
    println!("================================");
}

pub fn print_public_outputs_with_cipher(decoded_output: &DecodedZKOutputsWithCipher) {
    println!("\n=== ZK Program Public Output ===");
    println!("1. H_ORIG_BLOCK (原文块哈希):   {}", hex::encode(decoded_output.h_orig_block));
    println!("2. H_CIPHER_BLOCK (密文块): {}", hex::encode(decoded_output.cipher_block));
    println!("3. H_K_COMMITMENT (密钥承诺哈希): {}", hex::encode(decoded_output.h_k_commitment));
    println!("================================");
}