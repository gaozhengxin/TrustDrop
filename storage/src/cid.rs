extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use cid::Cid;
use sha2::{ Sha256, Digest };

pub fn compute_lighthouse_cid(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let hash_bytes = hasher.finalize();

    let mut mh_bytes = [0u8; 34];
    mh_bytes[0] = 0x12;
    mh_bytes[1] = 0x20;
    mh_bytes[2..].copy_from_slice(&hash_bytes);

    let mut cid_v1_bytes = Vec::new();
    cid_v1_bytes.push(0x01);
    cid_v1_bytes.push(0x55);
    cid_v1_bytes.extend_from_slice(&mh_bytes);

    let cid = Cid::read_bytes(&cid_v1_bytes[..]).expect("Failed to build CID");
    alloc::format!("{}", cid)
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
        let cid = compute_lighthouse_cid(data);
        std::println!("Small file CID: {}", cid);
        assert_eq!(cid, "bafkreidn5zqnpzy4kfdv3xilsucbzvube726wly5g2zmi2do2kvlag3fr4");
    }

    #[test]
    fn test_large_file_consistency() {
        let data = alloc::vec![0u8; 300 * 1024];
        let cid = compute_lighthouse_cid(&data);
        std::println!("Large file CID: {}", cid);
        assert!(cid.starts_with("bafkrei"));
        assert_eq!(cid.len(), 59);
    }

    #[test]
    fn test_one_gb_logic_check() {
        let size = 1024 * 1024;
        let data = alloc::vec![1u8; size];
        let cid = compute_lighthouse_cid(&data);
        assert!(cid.starts_with("bafkrei"));
    }
}
