use sha2::{Digest, Sha256};
use core::result::Result;

// 定义错误类型，用于函数返回
type KeyDeriveResult<T> = Result<T, &'static str>;

/// 利用 ECC 私钥和消息哈希确定性地推导出 32 字节的 ChaCha8 密钥。
/// 
/// 遵循 HKDF 的 'Extract-Then-Expand' 思想，但简化为单次 ZK 友好的哈希链操作。
/// 
/// # 参数
/// * `ecc_sk`: 32 字节的 ECC 私钥 (通用标量)。
/// * `msg_hash`: 32 字节的消息哈希 (通用哈希，如 Blake3/Keccak)。
/// 
/// # 返回值
/// 32 字节的 ChaCha8 密钥 ([u8; 32])。
pub fn chacha8_key_derive(
    ecc_sk: &[u8; 32],
    msg_hash: &[u8; 32],
) -> KeyDeriveResult<[u8; 32]> {
    
    // 确保输入长度符合预期
    if ecc_sk.len() != 32 || msg_hash.len() != 32 {
        return Err("输入密钥和哈希长度必须为 32 字节");
    }

    // 核心 KDF 逻辑：
    // Key = Hash(ECC_SK || MSG_HASH)
    // 这种简单的串联哈希是 ZK 证明中最轻量级、最通用的 KDF 实现。
    
    let mut hasher = Sha256::new(); // 假设这是 ZK 友好的哈希函数
    
    // 1. 串联输入
    hasher.update(ecc_sk);
    hasher.update(msg_hash);
    
    // 2. 最终哈希推导
    let result_bytes = hasher.finalize();
    
    // 3. 截取/复制结果（SHA-256 结果已经是 32 字节）
    let mut derived_key: [u8; 32] = [0; 32];
    derived_key.copy_from_slice(result_bytes.as_slice());

    Ok(derived_key)
}
