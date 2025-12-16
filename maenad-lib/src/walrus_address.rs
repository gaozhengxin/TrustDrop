use sha2::{Digest, Sha256};
use hex;

/// Walrus 风格 Blob Address 的哈希输出类型（SHA-256，32 字节）
pub type BlobAddress = [u8; 32];

/// 协议假设的 Sliver（数据片段）大小。
/// Walrus 协议通常使用 64KB 或 128KB，我们使用 64KB 作为示例。
const SLIVER_SIZE: usize = 64 * 1024; // 65536 字节

/// 计算任意字节切片的 SHA-256 哈希值。
/// 这是 Merkle 叶子节点和父节点计算的基础。
fn hash_data(data: &[u8]) -> BlobAddress {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

/// 计算 Merkle Tree 的父节点哈希。
/// 父哈希 = HASH(左子节点哈希 || 右子节点哈希)
fn hash_parent(left: &BlobAddress, right: &BlobAddress) -> BlobAddress {
    let mut hasher = Sha256::new();
    // 拼接 (||) 左哈希和右哈希
    hasher.update(left);
    hasher.update(right);
    hasher.finalize().into()
}

/**
 * @brief 计算给定 Blob 数据的 Walrus 风格 Blob Address (Merkle Tree Root)。
 *
 * @param blob_data 要计算地址的字节切片（最大 500MB）。
 * @return 成功返回 BlobAddress (32 字节的 SHA-256 哈希)，失败返回 None (例如空数据)。
 */
pub fn calculate_blob_address(blob_data: &[u8]) -> Option<BlobAddress> {
    if blob_data.is_empty() {
        return None;
    }

    // 1. 数据分片 (Slicing) 和计算叶子节点 (Sliver 哈希)
    // Walrus 协议：数据被切片成 SLIVER_SIZE 的块，然后计算每个 Sliver 的哈希。
    let num_slivers = (blob_data.len() + SLIVER_SIZE - 1) / SLIVER_SIZE;
    // 使用 Vec<BlobAddress> 存储当前层的哈希值
    let mut current_hashes: Vec<BlobAddress> = Vec::with_capacity(num_slivers);

    // 对每个 Sliver (数据块) 计算其叶子哈希
    for chunk in blob_data.chunks(SLIVER_SIZE) {
        current_hashes.push(hash_data(chunk));
    }

    // 

    // 2. Merkle Tree 构建 (从叶子节点到根节点)
    // 循环直到只剩下一个哈希值（即 Merkle Root）
    while current_hashes.len() > 1 {
        let mut next_layer: Vec<BlobAddress> = Vec::new();
        let mut i = 0;
        
        while i < current_hashes.len() {
            let left_hash = &current_hashes[i];
            
            // Merkle Tree 填充策略：如果只剩一个节点，复制自身进行哈希 (Hash(H_L || H_L))
            // 否则，取下一个节点。
            let right_hash = current_hashes.get(i + 1).unwrap_or(left_hash);

            let parent_hash = hash_parent(left_hash, right_hash);
            next_layer.push(parent_hash);
            
            i += 2; // 跳到下一对
        }

        // 用下一层哈希替换当前层
        current_hashes = next_layer;
    }

    // 根节点即为 Blob Address。如果 current_hashes.len() == 1，则 pop() 返回 Some(Address)
    current_hashes.pop()
}


// --- 实用工具函数 ---

/// 将 32 字节的 BlobAddress 转换为十六进制字符串。
pub fn address_to_hex(address: &BlobAddress) -> String {
    hex::encode(address)
}

/// 模块的单元测试
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    // 1. 测试小数据 Blob (刚好一个 Sliver)
    #[test]
    fn test_small_blob_address() {
        // 64KB 数据
        let data = vec![0xCC; SLIVER_SIZE];
        // 预期：Merkle Root 就是这 64KB 数据的 SHA-256
        let expected_hash = hash_data(&data);

        let address = calculate_blob_address(&data).unwrap();
        assert_eq!(address, expected_hash, "单 Sliver Blob 的地址应等于其数据的直接哈希");
    }

    // 2. 测试刚好两倍 Sliver 大小的 Blob
    #[test]
    fn test_two_sliver_blob_address() {
        let size = SLIVER_SIZE * 2;
        let mut data = vec![0; size];
        // 区分两个 Sliver
        for i in 0..SLIVER_SIZE { data[i] = 0xAA; }
        for i in SLIVER_SIZE..size { data[i] = 0xBB; }
        
        let hash_a = hash_data(&data[0..SLIVER_SIZE]);
        let hash_b = hash_data(&data[SLIVER_SIZE..size]);
        let expected_root = hash_parent(&hash_a, &hash_b);

        let address = calculate_blob_address(&data).unwrap();
        assert_eq!(address, expected_root, "两 Sliver Blob 的地址应是两 Sliver 哈希的父哈希");
    }

    // 3. 测试奇数个 Sliver 的 Blob (需要填充/复制)
    #[test]
    fn test_odd_sliver_blob_address() {
        let size = SLIVER_SIZE * 3;
        let data = vec![0xDD; size]; // 3 个相同的 Sliver
        
        let address = calculate_blob_address(&data).unwrap();
        // 验证 Merkle 树层级计算 (H1, H2, H3) -> (H12, H33) -> (H1233)
        // 步骤：
        // L0: H1, H2, H3
        // L1: H(H1 || H2), H(H3 || H3)
        // Root: H(H12 || H33)
        
        let h1 = hash_data(&data[0..SLIVER_SIZE]); // 都是一样的
        let h12 = hash_parent(&h1, &h1);
        let h33 = hash_parent(&h1, &h1); // 这里的 h3 也等于 h1，所以是 H(H1||H1)

        let expected_root = hash_parent(&h12, &h33);
        assert_eq!(address, expected_root, "奇数 Sliver 的 Merkle 根应正确填充");
    }

    // 4. 压力测试 (模拟 100MB 左右的大 Blob)
    #[test]
    fn test_large_blob_performance() {
        const TEST_SIZE: usize = 100 * 1024 * 1024; // 100MB
        let data = vec![0xEF; TEST_SIZE];
        println!("开始计算 100MB Blob Address...");

        let start = Instant::now();
        let address = calculate_blob_address(&data).unwrap();
        let duration = start.elapsed();
        
        println!("100MB Blob Address 计算耗时: {:?}", duration);
        println!("Address: {}", address_to_hex(&address));

        // 验证计算结果（100MB 统一数据计算的哈希应是固定的）
        let expected_fixed_address = hex::decode("7141f26d7f950669b02a2cc1222479e000720464c09d5718a385f0962817d2a5").unwrap();
        assert_eq!(address.to_vec(), expected_fixed_address, "100MB 固定数据计算结果应一致");
    }
}