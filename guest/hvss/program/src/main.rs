#![no_main]
sp1_zkvm::entrypoint!(main);

use sp1_zkvm::io;
use maenad_lib::chacha8;

pub fn main() {
    // 输入：原文数据块（Msg）
    let msg: Vec<u8> = io::read_vec();
    // 输入：ChaCha8 密钥 (K_KEY)，长度应为 32 字节
    let key: Vec<u8> = io::read_vec();
    // 输入：Nonce (12 字节) 和 Counter (u32)，用于加密
    let nonce: Vec<u8> = io::read_vec();

    const INITIAL_COUNTER: u32 = 0;

    // ZK Program 核心计算 (加密 & 承诺)

    // 检查输入
    if key.len() != 32 || nonce.len() != 12 {
        panic!("invalid chacha8 key or nonce");
    }
    
    // 计算密钥承诺 H_K 
    let h_k = blake3::hash(&key).as_bytes().to_vec();

    // 准备底层 ChaCha8 函数所需的输入引用
    let key_array: &[u8; 32] = key.as_slice().try_into().expect("Key长度错误");
    let nonce_array: &[u8; 12] = nonce.as_slice().try_into().expect("Nonce长度错误");

    // 执行底层加密 (ChaCha8 运算验证)
    let ciphertext: Vec<u8> = match chacha8::chacha8_encrypt(
        &msg,
        key_array,
        nonce_array,
        INITIAL_COUNTER
    ) {
        Ok(c) => c,
        Err(_) => {
            panic!("ChaCha8 加密计算失败"); 
        }
    };

    let h_cipher = blake3::hash(&ciphertext).as_bytes().to_vec();

    // 承诺密文哈希 (H(Ciphertext))
    io::commit_slice(&h_cipher);
    
    // 承诺密钥哈希 (H_K)
    io::commit_slice(&h_k);
}