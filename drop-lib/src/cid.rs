#![no_std]
use cid::Cid;
use sha2::{ Digest, Sha256 };

extern crate alloc;

use alloc::vec::Vec;
use alloc::vec;
use alloc::string::String;

const CHUNK_SIZE: usize = 262144; // 256KB

pub fn compute_ipfs_cid(data: &[u8]) -> String {
    if data.len() <= CHUNK_SIZE {
        // 小文件：直接 Raw 哈希
        return compute_raw_cid(data);
    }

    // 大文件：分块并构建 DAG-PB Root
    let mut links = Vec::new();
    let mut blocksizes = Vec::new();
    let mut total_len = 0u64;

    for chunk in data.chunks(CHUNK_SIZE) {
        let chunk_len = chunk.len() as u64;
        let mut hasher = Sha256::new();
        hasher.update(chunk);
        let hash = hasher.finalize();

        // 叶子节点 CID (Raw 0x55)
        let mh = cid::multihash::Multihash
            ::from_bytes(&[vec![0x12, 0x20], hash.to_vec()].concat())
            .unwrap();
        let leaf_cid = Cid::new_v1(0x55, mh);

        links.push((leaf_cid, chunk_len));
        blocksizes.push(chunk_len);
        total_len += chunk_len;
    }

    compute_root_dag_pb(links, blocksizes, total_len)
}

fn compute_raw_cid(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let hash = hasher.finalize();
    let mh = cid::multihash::Multihash
        ::from_bytes(&[vec![0x12, 0x20], hash.to_vec()].concat())
        .unwrap();
    Cid::new_v1(0x55, mh).to_string()
}

fn compute_root_dag_pb(links: Vec<(Cid, u64)>, blocksizes: Vec<u64>, total_len: u64) -> String {
    let mut dag_pb_bytes = Vec::new();

    // 1. 序列化 Links (Tag 2)
    // IPFS 规范：Links 必须排在 Data 之前
    for (cid, size) in links {
        let mut link = Vec::new();
        // Hash (Tag 1)
        link.push(0x0a);
        let cid_bytes = cid.to_bytes();
        write_varint(cid_bytes.len() as u64, &mut link);
        link.extend_from_slice(&cid_bytes);
        // Name (Tag 2, 默认空字符串 0x12 0x00)
        link.extend_from_slice(&[0x12, 0x00]);
        // Tsize (Tag 3, Raw 块的 Tsize 就是数据长度)
        link.push(0x18);
        write_varint(size, &mut link);

        dag_pb_bytes.push(0x12);
        write_varint(link.len() as u64, &mut dag_pb_bytes);
        dag_pb_bytes.extend_from_slice(&link);
    }

    // 2. 序列化 UnixFS Data (Tag 1)
    let mut unix_fs_inner = Vec::new();
    // Type: File (Tag 1, Value 2)
    unix_fs_inner.extend_from_slice(&[0x08, 0x02]);
    // Filesize (Tag 3)
    unix_fs_inner.push(0x18);
    write_varint(total_len, &mut unix_fs_inner);
    // Blocksizes (Tag 4, repeated)
    for size in blocksizes {
        unix_fs_inner.push(0x20);
        write_varint(size, &mut unix_fs_inner);
    }

    dag_pb_bytes.push(0x0a);
    write_varint(unix_fs_inner.len() as u64, &mut dag_pb_bytes);
    dag_pb_bytes.extend_from_slice(&unix_fs_inner);

    // 3. 计算 Root 哈希并生成 CID (Codec 0x70)
    let mut hasher = Sha256::new();
    hasher.update(&dag_pb_bytes);
    let hash = hasher.finalize();
    let mh = cid::multihash::Multihash
        ::from_bytes(&[vec![0x12, 0x20], hash.to_vec()].concat())
        .unwrap();

    Cid::new_v1(0x70, mh).to_string()
}

fn write_varint(mut n: u64, buf: &mut Vec<u8>) {
    while n >= 0x80 {
        buf.push((n as u8) | 0x80);
        n >>= 7;
    }
    buf.push(n as u8);
}

/// 优化版：主入口，返回二进制字节而非 String
pub fn compute_ipfs_cid_zk_optimized(data: &[u8]) -> Vec<u8> {
    if data.len() <= CHUNK_SIZE {
        return compute_raw_cid_bytes(data);
    }

    let chunk_count = (data.len() + CHUNK_SIZE - 1) / CHUNK_SIZE;
    let mut links = Vec::with_capacity(chunk_count);
    let mut blocksizes = Vec::with_capacity(chunk_count);
    let mut total_len = 0u64;

    for chunk in data.chunks(CHUNK_SIZE) {
        let chunk_len = chunk.len() as u64;
        let mut hasher = Sha256::new();
        hasher.update(chunk);
        let hash = hasher.finalize();

        // --- 优化：使用固定数组消除 concat() ---
        let mut mh_bytes = [0u8; 34];
        mh_bytes[0] = 0x12; // SHA2-256 code
        mh_bytes[1] = 0x20; // Length 32
        mh_bytes[2..].copy_from_slice(&hash);

        let mh = cid::multihash::Multihash::from_bytes(&mh_bytes).unwrap();
        let leaf_cid = Cid::new_v1(0x55, mh);

        links.push((leaf_cid, chunk_len));
        blocksizes.push(chunk_len);
        total_len += chunk_len;
    }

    compute_root_dag_pb_zk_optimized(links, blocksizes, total_len)
}

/// 优化版：返回二进制 CID 字节，不执行 Base32
fn compute_raw_cid_bytes(data: &[u8]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let hash = hasher.finalize();

    let mut mh_bytes = [0u8; 34];
    mh_bytes[0] = 0x12;
    mh_bytes[1] = 0x20;
    mh_bytes[2..].copy_from_slice(&hash);

    let mh = cid::multihash::Multihash::from_bytes(&mh_bytes).unwrap();
    Cid::new_v1(0x55, mh).to_bytes()
}

/// 优化版：DAG-PB 序列化，预分配内存并减少拷贝
fn compute_root_dag_pb_zk_optimized(
    links: Vec<(Cid, u64)>,
    blocksizes: Vec<u64>,
    total_len: u64
) -> Vec<u8> {
    // 预估容量：每个 link 约 50 字节，unixfs 结构约 50 字节
    let estimated_cap = links.len() * 55 + 100;
    let mut dag_pb_bytes = Vec::with_capacity(estimated_cap);

    // 1. 序列化 Links
    for (cid, size) in links {
        let cid_bytes = cid.to_bytes();
        let mut link_inner = Vec::with_capacity(cid_bytes.len() + 20);

        // Hash (Tag 1)
        link_inner.push(0x0a);
        write_varint(cid_bytes.len() as u64, &mut link_inner);
        link_inner.extend_from_slice(&cid_bytes);

        // Name (Tag 2, 0x12 0x00)
        link_inner.extend_from_slice(&[0x12, 0x00]);

        // Tsize (Tag 3)
        link_inner.push(0x18);
        write_varint(size, &mut link_inner);

        // 写入外层 Link Tag
        dag_pb_bytes.push(0x12);
        write_varint(link_inner.len() as u64, &mut dag_pb_bytes);
        dag_pb_bytes.extend_from_slice(&link_inner);
    }

    // 2. 序列化 UnixFS Data
    let mut unix_fs_inner = Vec::with_capacity(blocksizes.len() * 10 + 20);
    // Type: File (Tag 1, Value 2)
    unix_fs_inner.extend_from_slice(&[0x08, 0x02]);
    // Filesize (Tag 3)
    unix_fs_inner.push(0x18);
    write_varint(total_len, &mut unix_fs_inner);
    // Blocksizes (Tag 4)
    for size in blocksizes {
        unix_fs_inner.push(0x20);
        write_varint(size, &mut unix_fs_inner);
    }

    dag_pb_bytes.push(0x0a);
    write_varint(unix_fs_inner.len() as u64, &mut dag_pb_bytes);
    dag_pb_bytes.extend_from_slice(&unix_fs_inner);

    // 3. 计算 Root 哈希并生成 CID 二进制
    let mut hasher = Sha256::new();
    hasher.update(&dag_pb_bytes);
    let hash = hasher.finalize();

    let mut mh_bytes = [0u8; 34];
    mh_bytes[0] = 0x12;
    mh_bytes[1] = 0x20;
    mh_bytes[2..].copy_from_slice(&hash);

    let mh = cid::multihash::Multihash::from_bytes(&mh_bytes).unwrap();
    Cid::new_v1(0x70, mh).to_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate std;

    #[test]
    fn test_small_file_consistency() {
        // 测试小文件 (<= 256KB) -> 产生 Raw Leaf CID
        let data =
            "[\"朝辞白帝彩云间\",\"千里江陵一日还\",\"两岸猿声啼不住\",\"轻舟已过万重山\"]\n".as_bytes();
        let cid = compute_ipfs_cid(data);
        std::println!("Small file CID: {}", cid);
        assert_eq!(cid, "bafkreidn5zqnpzy4kfdv3xilsucbzvube726wly5g2zmi2do2kvlag3fr4");
    }

    #[test]
    fn test_large_file_consistency() {
        let data = alloc::vec![0u8; 300 * 1024];
        let cid = compute_ipfs_cid(&data);
        std::println!("Large file CID: {}", cid);
        assert!(cid.starts_with("bafybei"));
        assert_eq!(cid.len(), 59);
    }

    #[test]
    fn test_one_gb_logic_check() {
        let size = 1024 * 1024;
        let data = alloc::vec![1u8; size];
        let cid = compute_ipfs_cid(&data);
        assert!(cid.starts_with("bafybei"));
    }

    #[test]
    fn test_consistency_optimized_vs_original() {
        use std::string::ToString;
        // 1. 准备测试数据 (跨越分块边界，测试 DAG-PB 逻辑)
        let data = alloc::vec![0u8; 512 * 1024]; // 512KB, 会产生 2 个 256KB 的块

        // 2. 使用原版函数计算 (返回 String)
        let original_cid_str = compute_ipfs_cid(&data);

        // 3. 使用 ZK 优化版计算 (返回 Vec<u8> 二进制)
        let optimized_cid_bytes = compute_ipfs_cid_zk_optimized(&data);

        // 4. 将优化后的字节还原为 Cid 对象，进而转为字符串
        // 这样可以确保二进制编码格式完全符合规范
        let cid_obj = Cid::read_bytes(optimized_cid_bytes.as_slice()).expect(
            "Failed to parse optimized bytes back to CID"
        );
        let optimized_as_str = cid_obj.to_string();

        // 5. 打印对比结果
        std::println!("Original CID String:  {}", original_cid_str);
        std::println!("Optimized CID String: {}", optimized_as_str);

        // 6. 断言逻辑一致性
        assert_eq!(
            original_cid_str,
            optimized_as_str,
            "The optimized binary CID must match the original string representation"
        );

        // 7. 额外检查：验证二进制结果是否符合 CID 规范长度 (v1, raw/dag-pb, sha2-256)
        // 典型的二进制长度为 36 字节左右
        std::println!("Optimized CID Binary Length: {} bytes", optimized_cid_bytes.len());
        assert!(optimized_cid_bytes.len() >= 34);
    }
}
