use alloy_sol_types::sol;

use core::result::Result;
use chacha20::cipher::KeyIvInit;
use chacha20::{
    cipher::{
        StreamCipher
    },
    ChaCha8,
    Key, Nonce
};
use chacha20::cipher::StreamCipherSeek;

use sha2::{Digest, Sha256};

use hex;
use std::convert::TryInto;
use std::error::Error;
use std::fmt;

sol! {
    /// ZK Program 1 的公开输出值结构，用于 Solidity 合约验证。
    /// 字段顺序严格匹配 ZKVM 的 io::commit_vec 输出顺序。
    struct PublicZKOutputs {
        bytes32 hCipherBlock;    // 密文块的哈希
        bytes32 hKCommitment;    // ChaCha8 密钥的承诺哈希 (H_K)
        // bytes32 hOrigBlock;      // 原文块的哈希 (H_ORIG Block)
    }
}

// ZK Program 1 输出的固定哈希长度
const HASH_LEN: usize = 32;

// --- 1. 输出数据结构 ---
/// ZK Program 1 的公开输出结构，用于 Rust 端的解码和处理。
#[derive(Debug, Clone)]
pub struct DecodedZKOutputs {
    /// 密文块的哈希 (H(Ciphertext Block))
    pub h_cipher_block: [u8; HASH_LEN],
    /// ChaCha8 密钥的承诺哈希 (H_K)
    pub h_k_commitment: [u8; HASH_LEN],
    ///// 原文块的哈希 (H_ORIG Block)
    // pub h_orig_block: [u8; HASH_LEN],
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
            //DecodingError::InvalidLength => write!(f, "Invalid output length: expected {} bytes.", 3 * HASH_LEN),
            DecodingError::InvalidLength => write!(f, "Invalid output length: expected {} bytes.", 2 * HASH_LEN),
            DecodingError::IoError(e) => write!(f, "I/O error during decoding: {}", e),
        }
    }
}

impl Error for DecodingError {}


// --- 3. 解码函数 (Decode Logic) ---
/// 将 ZKVM 提交的原始字节流解码为 DecodedZKOutputs 结构体。
pub fn decode_public_outputs(output_bytes: &[u8]) -> Result<DecodedZKOutputs, DecodingError> {
    //const TOTAL_LEN: usize = 3 * HASH_LEN;
    const TOTAL_LEN: usize = 2 * HASH_LEN;

    if output_bytes.len() < TOTAL_LEN {
        return Err(DecodingError::InvalidLength);
    }

    let mut cursor = 0;

    // 1. 解码 hCipherBlock (0..32)
    let h_cipher_block_slice: [u8; HASH_LEN] = output_bytes[cursor..cursor + HASH_LEN]
        .try_into()
        .map_err(|e| DecodingError::IoError(format!("Failed to read hCipherBlock: {:?}", e)))?;
    cursor += HASH_LEN;

    // 2. 解码 hKCommitment (32..64)
    let h_k_commitment_slice: [u8; HASH_LEN] = output_bytes[cursor..cursor + HASH_LEN]
        .try_into()
        .map_err(|e| DecodingError::IoError(format!("Failed to read hKCommitment: {:?}", e)))?;
    cursor += HASH_LEN;

    // 3. 解码 hOrigBlock (64..96)
    // let h_orig_block_slice: [u8; HASH_LEN] = output_bytes[cursor..cursor + HASH_LEN]
    //    .try_into()
    //    .map_err(|e| DecodingError::IoError(format!("Failed to read hOrigBlock: {:?}", e)))?;

    Ok(DecodedZKOutputs {
        h_cipher_block: h_cipher_block_slice,
        h_k_commitment: h_k_commitment_slice,
        //h_orig_block: h_orig_block_slice,
    })
}

// --- 4. 打印函数 (Output Logic) ---
/// 格式化并打印解码后的 ZK Program 输出。
pub fn print_public_outputs(decoded_output: &DecodedZKOutputs) {
    println!("\n=== ZK Program Public Output ===");
    println!("1. H_CIPHER_BLOCK (密文块哈希): {}", hex::encode(decoded_output.h_cipher_block));
    println!("2. H_K_COMMITMENT (密钥承诺哈希): {}", hex::encode(decoded_output.h_k_commitment));
    // println!("3. H_ORIG_BLOCK (原文块哈希):   {}", hex::encode(decoded_output.h_orig_block));
    println!("================================");
}

// --- 包装函数 1: 加密/封装 (Seal) ---
/// 简化后的 ChaCha8 封装接口。
/// 自动推导 Nonce (依赖 Key 和 Msg)，并固定 initial_counter=0。
/// 在 zkvm 中使用底层函数 chacha8_encrypt，节省操作。
///
/// # 参数
/// * `msg`: 待加密的原始数据。
/// * `key`: 32 字节的 ChaCha8 密钥。
pub fn chacha8_seal(
    msg: &[u8],
    key: &[u8; 32],
    aux_data: &[u8],
) -> Result<Vec<u8>, &'static str> {
    
    // 1. 确定性 Nonce 推导 (依赖 Key 和 Plaintext/Msg)
    let binding = derive_nonce(key, aux_data);
    let nonce_ref: &[u8; 12] = binding.as_slice().try_into().unwrap();
    
    // 2. 固定 Counter
    const INITIAL_COUNTER: u32 = 0;

    // 3. 调用原有的复杂函数签名
    // 注意：这里的 my_crypto_lib::chacha8_encrypt 假设是您之前提供的那个函数签名。
    chacha8_encrypt(
        msg,
        key,
        nonce_ref,
        INITIAL_COUNTER,
    )
}

// --- 包装函数 2: 解密/解封装 (Unseal) ---
/// 简化后的 ChaCha8 解封装接口。
/// 自动推导 Nonce (依赖 Key 和 Ciphertext)，并固定 initial_counter=0。
/// 在 zkvm 中使用底层函数 chacha8_decrypt，节省操作。
///
/// # 参数
/// * `msg`: 待解密的密文数据。
/// * `key`: 32 字节的 ChaCha8 密钥。
pub fn chacha8_unseal(
    msg: &[u8],
    key: &[u8; 32],
    aux_data: &[u8],
) -> Result<Vec<u8>, &'static str> {
    
    // 1. 确定性 Nonce 推导 (依赖 Key 和 Ciphertext/Msg)
    // 必须使用 Key 和密文 (msg) 来推导，以保证与加密时的密钥流一致。
    let binding = derive_nonce(key, aux_data);
    let nonce_ref: &[u8; 12] = binding.as_slice().try_into().unwrap();

    // 2. 固定 Counter
    const INITIAL_COUNTER: u32 = 0;

    // 3. 调用原有的复杂函数签名
    // 注意：这里的 my_crypto_lib::chacha8_decrypt 假设是您之前提供的那个函数签名。
    chacha8_decrypt(
        msg,
        key,
        nonce_ref,
        INITIAL_COUNTER,
    )
}

// --- 确定性 Nonce 导出函数 ---
pub fn derive_nonce(key: &[u8; 32], aux_data: &[u8]) -> Nonce {
    let mut hasher = Sha256::new();
    
    // 1. 输入 Key 和 Msg
    hasher.update(key);
    hasher.update(aux_data);
    
    let full_hash = hasher.finalize();
    
    // 2. 截断/转换：ChaCha8 Nonce 要求是 12 字节。
    // 我们从 32 字节的 SHA-256 哈希结果中取前 12 字节。
    let mut nonce_bytes = [0u8; 12];
    nonce_bytes.copy_from_slice(&full_hash[0..12]);
    
    *Nonce::from_slice(&nonce_bytes)
}

/// 执行 ChaCha8 流式加密。
/// 
/// ZK Program 1 必须证明该函数使用了正确的 K_KEY、H_ORIG 
/// (作为 msg) 和已承诺的 Nonce/Counter。
///
/// # 参数
/// * `msg`: 待加密的原始数据 (H_ORIG 的数据块)。
/// * `key`: 32 字节的 ChaCha8 密钥 (K_KEY)。
/// * `nonce`: 12 字节的 Nonce/IV。
/// * `initial_counter`: 4 字节的初始块计数器。
///
/// # 返回值
/// 包含加密后数据的 Vec<u8> 或加密错误。
pub fn chacha8_encrypt(
    msg: &[u8],
    key: &[u8; 32],
    nonce: &[u8; 12],
    initial_counter: u32,
) -> Result<Vec<u8>, &'static str> {
    let key_ref = Key::from_slice(key);
    let nonce_ref = Nonce::from_slice(nonce);

    let mut cipher = ChaCha8::new(key_ref, nonce_ref);

    let block_index = initial_counter as u64;
    cipher.seek(block_index);

    let mut buffer = msg.to_vec();
    
    match cipher.apply_keystream(&mut buffer) {
        () => Ok(buffer),
    }
}

/// 执行 ChaCha8 流式解密。
/// 
/// ChaCha8 是流式密码，解密操作与加密操作完全相同（XOR）。
/// ZK Program 1 必须证明该函数使用了正确的 K_KEY、H_CIPHER (作为 msg) 
/// 和已承诺的 Nonce/Counter，得到 H_ORIG 的数据块。
///
/// # 参数
/// * `msg`: 待解密的密文数据 (H_CIPHER 的数据块)。
/// * `key`: 32 字节的 ChaCha8 密钥 (K_KEY)。
/// * `nonce`: 12 字节的 Nonce/IV。
/// * `initial_counter`: 4 字节的初始块计数器。
///
/// # 返回值
/// 包含解密后数据的 Vec<u8> (即原始数据块) 或解密错误。
pub fn chacha8_decrypt(
    msg: &[u8],
    key: &[u8; 32],
    nonce: &[u8; 12],
    initial_counter: u32,
) -> Result<Vec<u8>, &'static str> {
    // 1. 类型转换：将 &[u8] 转换为 Key 和 Nonce 类型
    let key_ref = Key::from_slice(key);
    let nonce_ref = Nonce::from_slice(nonce);

    // 2. 初始化 ChaCha8 Cipher
    let mut cipher = ChaCha8::new(key_ref, nonce_ref);

    // 3. 设置初始计数器
    // 必须使用与加密时相同的初始计数器
    let block_index = initial_counter as u64;
    cipher.seek(block_index);

    // 4. 执行解密 (apply_keystream 是双向操作)
    let mut buffer = msg.to_vec();
    
    match cipher.apply_keystream(&mut buffer) {
        () => Ok(buffer),
    }
}
