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

use std::convert::TryInto;

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
