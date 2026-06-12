use blake3;
use crate::chacha8;

pub const DEFAULT_CHUNK_SIZE: usize = 1024 * 1024;

pub struct MerkleTree {
    pub chunks: Vec<Vec<u8>>,         // 分割后的 chunk
    pub layers: Vec<Vec<[u8; 32]>>,   // Merkle tree 每一层，从叶到根
}

impl MerkleTree {
    pub fn root(&self) -> [u8; 32] {
        self.layers.last().unwrap()[0]
    }

    pub fn assemble(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for c in &self.chunks {
            out.extend_from_slice(c);
        }
        out
    }

    pub fn new_from_data(data: &[u8], chunk_size: usize) -> Self {
        build_merkle_tree(data, chunk_size)
    }

    pub fn get_chunk(&self, index: usize) -> &[u8] {
        &self.chunks[index]
    }

    pub fn decrypt_chunk(
        &self,
        index: usize,
        key: &[u8; 32],
        origin_root: [u8; 32],
    ) -> Vec<u8> {
        let mut aux = Vec::with_capacity(32 + 8);
        aux.extend_from_slice(&origin_root);
        aux.extend_from_slice(&(index as u64).to_le_bytes());

        chacha8::chacha8_unseal(&self.chunks[index], key, &aux)
            .expect("single chunk decrypt failed")
    }

    pub fn reconstruct_data(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for c in &self.chunks {
            out.extend_from_slice(c);
        }
        out
    }
}


pub fn build_merkle_tree(data: &[u8], chunk_size: usize) -> MerkleTree {
    // split into chunks
    let mut chunks = Vec::new();
    let mut offset = 0;
    while offset < data.len() {
        let end = usize::min(offset + chunk_size, data.len());
        chunks.push(data[offset..end].to_vec());
        offset = end;
    }

    // leaf hashes
    let mut layer: Vec<[u8; 32]> = chunks
        .iter()
        .map(|c| *blake3::hash(c).as_bytes())
        .collect();

    let mut layers = vec![layer.clone()];

    // build upper layers
    while layer.len() > 1 {
        if layer.len() % 2 == 1 {
            layer.push(*layer.last().unwrap());
        }

        let mut parent = Vec::with_capacity(layer.len() / 2);
        for j in (0..layer.len()).step_by(2) {
            let mut buf = Vec::with_capacity(64);
            buf.extend_from_slice(&layer[j]);
            buf.extend_from_slice(&layer[j + 1]);
            parent.push(*blake3::hash(&buf).as_bytes());
        }

        layers.push(parent.clone());
        layer = parent;
    }

    MerkleTree { chunks, layers }
}

pub fn encrypt_merkle_tree(
    origin_tree: &MerkleTree,
    key: &[u8; 32],
) -> Result<MerkleTree, &'static str> {
    let origin_root = origin_tree.root();
    let mut cipher_chunks = Vec::with_capacity(origin_tree.chunks.len());

    for (i, chunk) in origin_tree.chunks.iter().enumerate() {
        let mut aux = Vec::with_capacity(32 + 8);
        aux.extend_from_slice(&origin_root);
        aux.extend_from_slice(&(i as u64).to_le_bytes());

        let cipher = chacha8::chacha8_seal(chunk, key, &aux)?;
        cipher_chunks.push(cipher);
    }

    Ok(build_merkle_tree_from_chunks(cipher_chunks))
}

pub fn decrypt_cipher_merkle_tree<'a>(
    cipher_tree: &'a MerkleTree,
    origin_root: &[u8; 32],
    key: &[u8; 32],
) -> Result<MerkleTree, &'static str> {
    let mut plain_chunks = Vec::with_capacity(cipher_tree.chunks.len());

    for (i, chunk) in cipher_tree.chunks.iter().enumerate() {
        let mut aux = Vec::with_capacity(32 + 8);
        aux.extend_from_slice(origin_root);
        aux.extend_from_slice(&(i as u64).to_le_bytes());

        let plain = chacha8::chacha8_unseal(chunk, key, &aux)?;
        plain_chunks.push(plain);
    }

    Ok(build_merkle_tree_from_chunks(plain_chunks))
}

fn build_merkle_tree_from_chunks(chunks: Vec<Vec<u8>>) -> MerkleTree {
    let mut layer: Vec<[u8; 32]> = chunks
        .iter()
        .map(|c| *blake3::hash(c).as_bytes())
        .collect();

    let mut layers = vec![layer.clone()];

    while layer.len() > 1 {
        if layer.len() % 2 == 1 {
            layer.push(*layer.last().unwrap());
        }

        let mut parent = Vec::with_capacity(layer.len() / 2);
        for j in (0..layer.len()).step_by(2) {
            let mut buf = Vec::with_capacity(64);
            buf.extend_from_slice(&layer[j]);
            buf.extend_from_slice(&layer[j + 1]);
            parent.push(*blake3::hash(&buf).as_bytes());
        }

        layers.push(parent.clone());
        layer = parent;
    }

    MerkleTree { chunks, layers }
}

#[cfg(test)]
mod tests {
    use rand::RngCore;

    use crate::merkle::{
        build_merkle_tree,
        encrypt_merkle_tree,
        decrypt_cipher_merkle_tree,
        MerkleTree,
    };

    const CHUNK_SIZE: usize = 1024 * 1024; // 1MB
    const TEST_SIZE: usize = 5 * 1024 * 1024; // 5MB

    #[test]
    fn test_merkle_chacha8_encrypt_decrypt() {
        // -----------------------------------------------
        // 1. 随机生成 5MB bytes
        // -----------------------------------------------
        let mut origin = vec![0u8; TEST_SIZE];
        rand::thread_rng().fill_bytes(&mut origin);
        
        println!("origin.len() = {}", origin.len());
        println!("origin (hex) head = {}", hex::encode(&origin[..32.min(origin.len())]));
        println!("origin (hex) tail = {}", hex::encode(&origin[origin.len().saturating_sub(32)..]));

        // -----------------------------------------------
        // 2. 构建 Merkle Tree
        // -----------------------------------------------
        let origin_tree = build_merkle_tree(&origin, CHUNK_SIZE);
        let origin_root = origin_tree.root();

        // -----------------------------------------------
        // 3. 使用固定 key 加密 Merkle Tree
        // -----------------------------------------------
        let key: [u8; 32] = [7u8; 32];
        let cipher_tree = encrypt_merkle_tree(&origin_tree, &key).expect("encrypt ok");
        if !cipher_tree.chunks.is_empty() {
            let c0 = &cipher_tree.chunks[0];
            println!(
                "cipher chunk[0] len = {} | hex head = {} | hex tail = {}",
                c0.len(),
                hex::encode(&c0[..std::cmp::min(32, c0.len())]),
                hex::encode(&c0[c0.len().saturating_sub(32)..c0.len()])
            );
        }

        // -----------------------------------------------
        // 4. 测试解密某一个 chunk
        // -----------------------------------------------
        let chunk_index = 2;

        // 构造 aux
        let mut aux = Vec::with_capacity(32 + 8);
        aux.extend_from_slice(&origin_root);
        aux.extend_from_slice(&(chunk_index as u64).to_le_bytes());

        // 调用 chacha8_unseal 手动解密
        let decrypted_one = crate::chacha8::chacha8_unseal(
            &cipher_tree.chunks[chunk_index],
            &key,
            &aux,
        ).expect("unseal ok");

        let orig_slice = &origin_tree.chunks[chunk_index];

        assert_eq!(
            decrypted_one.as_slice(),
            orig_slice,
            "single chunk decrypt mismatch"
        );

        // -----------------------------------------------
        // 5. 解密整个 cipher merkle tree 并恢复数据
        // -----------------------------------------------
        let decrypted_tree = decrypt_cipher_merkle_tree(
            &cipher_tree,
            &origin_root,
            &key
        ).expect("decrypt cipher tree ok");

        let reconstructed = decrypted_tree.assemble();
        assert_eq!(
            reconstructed.as_slice(),
            origin.as_slice(),
            "full decrypt reconstruct mismatch"
        );

        println!(
            "reconstructed hex head = {} | reconstructed hex tail = {}",
            hex::encode(&reconstructed[..std::cmp::min(32, reconstructed.len())]),
            hex::encode(
                &reconstructed[reconstructed.len().saturating_sub(32)..reconstructed.len()]
            )
        );
    }
}
