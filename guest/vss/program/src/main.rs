#![no_main]
sp1_zkvm::entrypoint!(main);

use maenad_lib::chacha8;

pub fn main() {
    use sp1_zkvm::io::{read, commit_slice};
    use blake3;

    // 1. 输入
    let length = read::<u8>();
    let length_usize = length as usize;

    let msg = read::<Vec<u8>>();

    // 读取 keys
    let mut keys = Vec::with_capacity(length_usize);
    for _ in 0..length_usize {
        let k = read::<[u8; 32]>();
        keys.push(k);
    }

    // 读取 nonces
    let mut nonces = Vec::with_capacity(length_usize);
    for _ in 0..length_usize {
        let n = read::<[u8; 12]>();
        nonces.push(n);
    }

    const INITIAL_COUNTER: u32 = 0;

    // ---------- 开始构造 ABI 编码的 public output ----------
    let mut public_output = Vec::with_capacity(4096);

    // 1. length (uint8 → 32 字节，右对齐)
    let mut tmp32 = [0u8; 32];
    tmp32[31] = length;
    public_output.extend_from_slice(&tmp32);

    // 2. h_orig_block = blake3(msg)
    public_output.extend_from_slice(blake3::hash(&msg).as_bytes());

    // 动态数据起始位置
    let dynamic_start = public_output.len();

    // ---------- 准备三个动态数组的内容 ----------
    let mut cipher_data   = Vec::with_capacity(length_usize * 32 + 32);
    let mut hk_data       = Vec::with_capacity(length_usize * 32 + 32);
    let mut nonce_data    = Vec::with_capacity(length_usize * 32 + 32);

    // 每个数组前面都要放长度（uint256）
    tmp32[31] = length;                           // 复用 tmp32
    let len_bytes = tmp32.to_vec();
    cipher_data.extend_from_slice(&len_bytes);
    hk_data.extend_from_slice(&len_bytes);
    nonce_data.extend_from_slice(&len_bytes);

    // 填充实际数据
    for (key, nonce) in keys.iter().zip(nonces.iter()) {
        // 密钥承诺
        let h_k: [u8; 32] = blake3::hash(key).into();

        // 加密（长度不变）
        let ciphertext = chacha8::chacha8_encrypt(&msg, key, nonce, INITIAL_COUNTER)
            .expect("encrypt failed");

        cipher_data.extend_from_slice(&ciphertext);
        hk_data.extend_from_slice(&h_k);

        // nonce → bytes12 在 ABI 中占 32 字节（左对齐）
        let mut padded = [0u8; 32];
        padded[..12].copy_from_slice(nonce);
        nonce_data.extend_from_slice(&padded);
    }

    // ---------- 追加动态数据 ----------
    public_output.extend_from_slice(&cipher_data);
    public_output.extend_from_slice(&hk_data);
    public_output.extend_from_slice(&nonce_data);

    // ---------- 提交 ----------
    commit_slice(&public_output);
}