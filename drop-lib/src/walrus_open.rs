//! Walrus blob-id 承诺的打开信息（Merkle openings）。
//!
//! VDD guest 将使用这些打开把采样 shard 的符号层层绑定到公开的 Walrus blob id：
//! 符号在 shard 树中打开到 primary/secondary 根，根对在“对叶树”中打开到根，
//! 对叶树根 + encoding + unencoded_length 重算 blob id。本模块负责构造、序列化
//! 与验证这些打开；guest 侧只调用验证函数，host 侧用构造器准备输入。

extern crate alloc;

use alloc::vec::Vec;
use serde::{Deserialize, Serialize};
use walrus_core::{
    fastcrypto::Blake2b256,
    merkle::{MerkleAuth, MerkleProof, MerkleTree, Node},
    metadata::{BlobMetadata, SliverPairMetadata},
    BlobId, EncodingType,
};

use crate::walrus_blob_id::{compute_blob_id_from_sliver_pair_roots, SliverPairRoots};

/// 单个采样符号在某个 shard 树中的 Merkle 打开。
///
/// Walrus shard 树的叶子是 `blake2b(0x00 || symbol)`，因此验证方持有原始
/// `symbol` 字节 + `path`（自底向上的兄弟节点）即可重算出该树根，
/// 再与 opener 声称的 shard 根比较。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalrusSymbolOpening {
    /// 该符号在对应 shard 树中的叶子索引（0..n_shards）。
    pub leaf_index: u32,
    /// 符号原始字节（SYMBOL_SIZE 字节）。
    pub symbol: Vec<u8>,
    /// 自底向上的兄弟节点，与 walrus-core `MerkleProof::path()` 一致。
    pub path: Vec<Node>,
}

/// 单个采样 shard 的 blob-id 承诺打开。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalrusShardOpening {
    pub shard_index: u32,
    /// primary 树根（编码矩阵第 `shard_index` 行的 Merkle 根）。
    pub primary_root: [u8; 32],
    /// secondary 树根（编码矩阵第 `n_shards-1-shard_index` 列的 Merkle 根）。
    pub secondary_root: [u8; 32],
    /// primary 树中采样的符号打开。
    pub primary_symbols: Vec<WalrusSymbolOpening>,
    /// secondary 树中采样的符号打开。
    pub secondary_symbols: Vec<WalrusSymbolOpening>,
    /// 对叶 `primary_root || secondary_root`（64B）在对叶树中的 Merkle 路径。
    pub pair_leaf_path: Vec<Node>,
    /// RSLH 绑定使用的完整 primary sliver（667 个原始 symbols）。
    /// Guest 校验这些 symbols 对应的 leaf hashes，再只重建一次 Merkle tree。
    pub column_symbols: Vec<Vec<u8>>,
    /// sampled primary 树的全部 1000 个 leaf hashes；后 333 个叶子没有对应
    /// primary sliver 原文，但仍用于一次性重建被 blob commitment 绑定的树根。
    pub column_leaf_hashes: Vec<Node>,
}

/// 一个 blob 的完整打开信息。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalrusBlobOpening {
    /// 目标 blob id（公开值，最终与链上清单一致）。
    pub blob_id: [u8; 32],
    /// `EncodingType` 的字节值（RS2 = 1）。
    pub encoding_type: u8,
    /// 未编码长度（blob id 计算的输入之一）。
    pub unencoded_length: u64,
    /// shard 数量（1000）。
    pub n_shards: u32,
    /// 全部 sliver pair 根（按 shard index 排列）；guest 用它重算对叶树根与 blob id。
    pub sliver_pair_roots: Vec<SliverPairRoots>,
    /// 采样 shard 的打开信息。
    pub shards: Vec<WalrusShardOpening>,
}

/// 某个树中一个待打开叶子的请求。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalrusSymbolRequest {
    /// 叶子索引。
    pub leaf_index: u32,
    /// 原始符号字节。
    pub symbol: Vec<u8>,
}

/// 采样请求：对某个 shard 的 primary/secondary 树分别请求若干叶子打开。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalrusSampleRequest {
    pub shard_index: u32,
    pub primary_symbols: Vec<WalrusSymbolRequest>,
    pub secondary_symbols: Vec<WalrusSymbolRequest>,
    /// RSLH column-binding: entry r = real cipher symbol at matrix (row r, col),
    /// i.e. the leaf of row r's primary tree at the RSLH column.
    pub column_symbols: Vec<WalrusSymbolRequest>,
}

/// 由编码矩阵的符号哈希构造 blob 打开。
///
/// `symbol_hashes` 是 `n_shards*n_shards`（行优先）的叶子哈希矩阵，即
/// `EncodingFactory::compute_metadata_with_symbol_hashes` 返回的第二项；
/// 其 `[row * n_shards + col]` 元素等于 `blake2b(0x00 || symbol(row, col))`，
/// 与 walrus-core `compute_metadata_from_symbol_hashes` 的输入完全一致。
pub fn build_walrus_blob_opening(
    symbol_hashes: &[Node],
    encoding_type: EncodingType,
    unencoded_length: u64,
    samples: &[WalrusSampleRequest],
) -> Result<WalrusBlobOpening, &'static str> {
    let n_shards = usize::isqrt(symbol_hashes.len());
    if n_shards == 0 || n_shards * n_shards != symbol_hashes.len() {
        return Err("walrus_open: symbol_hashes length must be n_shards*n_shards");
    }
    if n_shards > u32::MAX as usize {
        return Err("walrus_open: n_shards too large");
    }
    for sample in samples {
        if sample.shard_index as usize >= n_shards {
            return Err("walrus_open: shard_index out of range");
        }
        for request in sample
            .primary_symbols
            .iter()
            .chain(sample.secondary_symbols.iter())
        {
            if request.leaf_index as usize >= n_shards {
                return Err("walrus_open: leaf_index out of range");
            }
        }
    }

    // 1. 全部 shard 根（与 walrus-core 的 metadata 计算同构）：
    //    primary = 行 i 的 Merkle 根，secondary = 逆序列 n-1-i 的 Merkle 根。
    let mut roots = Vec::with_capacity(n_shards);
    for i in 0..n_shards {
        let primary_tree = MerkleTree::<Blake2b256>::build_from_leaf_hashes(
            symbol_hashes[i * n_shards..(i + 1) * n_shards].iter().cloned(),
        );
        let secondary_tree = MerkleTree::<Blake2b256>::build_from_leaf_hashes(
            (0..n_shards).map(|row| symbol_hashes[row * n_shards + n_shards - 1 - i].clone()),
        );
        roots.push(SliverPairRoots::new(
            primary_tree.root().bytes(),
            secondary_tree.root().bytes(),
        ));
    }

    // 2. blob id（复用已测试的库函数：对叶树根 + encoding + length）。
    let blob_id = compute_blob_id_from_sliver_pair_roots(&roots, encoding_type, unencoded_length);

    // 3. 对叶树：n_shards 个叶子，叶内容 = primary_root || secondary_root。
    let pair_tree = MerkleTree::<Blake2b256>::build(roots.iter().map(|pair| {
        let mut leaf = [0u8; 64];
        leaf[..32].copy_from_slice(&pair.primary);
        leaf[32..].copy_from_slice(&pair.secondary);
        leaf
    }));

    // 4. 采样 shard 的打开。
    let mut shards = Vec::with_capacity(samples.len());
    for sample in samples {
        let i = sample.shard_index as usize;
        let primary_tree = MerkleTree::<Blake2b256>::build_from_leaf_hashes(
            symbol_hashes[i * n_shards..(i + 1) * n_shards].iter().cloned(),
        );
        let secondary_tree = MerkleTree::<Blake2b256>::build_from_leaf_hashes(
            (0..n_shards).map(|row| symbol_hashes[row * n_shards + n_shards - 1 - i].clone()),
        );

        let primary_symbols = sample
            .primary_symbols
            .iter()
            .map(|request| {
                let proof = primary_tree
                    .get_proof(request.leaf_index as usize)
                    .map_err(|_| "walrus_open: failed to build primary symbol proof")?;
                Ok(WalrusSymbolOpening {
                    leaf_index: request.leaf_index,
                    symbol: request.symbol.clone(),
                    path: proof.path().to_vec(),
                })
            })
            .collect::<Result<Vec<_>, &'static str>>()?;

        let secondary_symbols = sample
            .secondary_symbols
            .iter()
            .map(|request| {
                let proof = secondary_tree
                    .get_proof(request.leaf_index as usize)
                    .map_err(|_| "walrus_open: failed to build secondary symbol proof")?;
                Ok(WalrusSymbolOpening {
                    leaf_index: request.leaf_index,
                    symbol: request.symbol.clone(),
                    path: proof.path().to_vec(),
                })
            })
            .collect::<Result<Vec<_>, &'static str>>()?;

        if sample.column_symbols.len() != crate::rslh_ve::COL_HEIGHT_SECONDARY as usize {
            return Err("walrus_open: full primary sliver must contain 667 symbols");
        }
        let column_symbols = sample
            .column_symbols
            .iter()
            .enumerate()
            .map(|(leaf_index, request)| {
                if request.leaf_index as usize != leaf_index {
                    return Err("walrus_open: full primary sliver symbols must be ordered");
                }
                Ok(request.symbol.clone())
            })
            .collect::<Result<Vec<_>, &'static str>>()?;
        let column_leaf_hashes =
            symbol_hashes[i * n_shards..(i + 1) * n_shards].to_vec();

        let pair_leaf_path = pair_tree
            .get_proof(i)
            .map_err(|_| "walrus_open: failed to build pair leaf proof")?
            .path()
            .to_vec();

        shards.push(WalrusShardOpening {
            shard_index: sample.shard_index,
            primary_root: roots[i].primary,
            secondary_root: roots[i].secondary,
            primary_symbols,
            secondary_symbols,
            pair_leaf_path,
            column_symbols,
            column_leaf_hashes,
        });
    }

    Ok(WalrusBlobOpening {
        blob_id: blob_id.0,
        encoding_type: encoding_type.into(),
        unencoded_length,
        n_shards: n_shards as u32,
        sliver_pair_roots: roots,
        shards,
    })
}

/// 验证一个 blob 打开是否自洽，并逐层绑定到 blob id。
///
/// - 用全部 `sliver_pair_roots` 重算对叶树根与 blob id，与 `opening.blob_id` 比较；
/// - 对每个采样 shard：检查其根与 `sliver_pair_roots[i]` 一致，并验证
///   primary/secondary 符号打开与对叶打开。
pub fn verify_walrus_blob_opening(opening: &WalrusBlobOpening) -> Result<(), &'static str> {
    let n_shards = opening.n_shards as usize;
    if n_shards == 0 || opening.sliver_pair_roots.len() != n_shards {
        return Err("walrus_open: n_shards mismatch");
    }

    let encoding_type = EncodingType::try_from(opening.encoding_type)
        .map_err(|_| "walrus_open: invalid encoding type")?;
    let metadata = BlobMetadata::new(
        encoding_type,
        opening.unencoded_length,
        opening
            .sliver_pair_roots
            .iter()
            .map(|pair| SliverPairMetadata {
                primary_hash: Node::Digest(pair.primary),
                secondary_hash: Node::Digest(pair.secondary),
            })
            .collect(),
    );

    let expected_blob_id = BlobId::from_sliver_pair_metadata(&metadata);
    if expected_blob_id.0 != opening.blob_id {
        return Err("walrus_open: blob id mismatch");
    }

    let pair_tree = MerkleTree::<Blake2b256>::build(opening.sliver_pair_roots.iter().map(
        |pair| {
            let mut leaf = [0u8; 64];
            leaf[..32].copy_from_slice(&pair.primary);
            leaf[32..].copy_from_slice(&pair.secondary);
            leaf
        },
    ));
    let pair_root = pair_tree.root();

    for shard in &opening.shards {
        let i = shard.shard_index as usize;
        if i >= n_shards {
            return Err("walrus_open: shard index out of range");
        }
        let pair = &opening.sliver_pair_roots[i];
        if pair.primary != shard.primary_root || pair.secondary != shard.secondary_root {
            return Err("walrus_open: shard root mismatch");
        }

        let mut pair_leaf = [0u8; 64];
        pair_leaf[..32].copy_from_slice(&shard.primary_root);
        pair_leaf[32..].copy_from_slice(&shard.secondary_root);
        MerkleProof::<Blake2b256>::new(&shard.pair_leaf_path)
            .verify_proof(&pair_root, n_shards, &pair_leaf, i)
            .map_err(|_| "walrus_open: pair leaf opening failure")?;

        for symbol in &shard.primary_symbols {
            if symbol.leaf_index as usize >= n_shards {
                return Err("walrus_open: symbol leaf index out of range");
            }
            MerkleProof::<Blake2b256>::new(&symbol.path)
                .verify_proof(
                    &Node::Digest(shard.primary_root),
                    n_shards,
                    &symbol.symbol,
                    symbol.leaf_index as usize,
                )
                .map_err(|_| "walrus_open: primary symbol opening failure")?;
        }

        for symbol in &shard.secondary_symbols {
            if symbol.leaf_index as usize >= n_shards {
                return Err("walrus_open: symbol leaf index out of range");
            }
            MerkleProof::<Blake2b256>::new(&symbol.path)
                .verify_proof(
                    &Node::Digest(shard.secondary_root),
                    n_shards,
                    &symbol.symbol,
                    symbol.leaf_index as usize,
                )
                .map_err(|_| "walrus_open: secondary symbol opening failure")?;
        }

        if shard.column_symbols.len() != crate::rslh_ve::COL_HEIGHT_SECONDARY as usize {
            return Err("walrus_open: full primary sliver length mismatch");
        }
        if shard.column_leaf_hashes.len() != n_shards {
            return Err("walrus_open: primary leaf hash count mismatch");
        }
        for (symbol, expected_hash) in shard.column_symbols.iter().zip(&shard.column_leaf_hashes) {
            let symbol_tree = MerkleTree::<Blake2b256>::build(core::iter::once(symbol.clone()));
            if symbol_tree.root() != *expected_hash {
                return Err("walrus_open: primary symbol/hash mismatch");
            }
        }
        let column_tree = MerkleTree::<Blake2b256>::build_from_leaf_hashes(
            shard.column_leaf_hashes.iter().cloned(),
        );
        if column_tree.root().bytes() != shard.primary_root {
            return Err("walrus_open: full primary sliver root mismatch");
        }
    }

    Ok(())
}

/// 把 RSLH-VE 列证明绑定到真实密文 blob。
///
/// 布局：RSLH 列 c（= `proof.col_index`，0..334）对应 Walrus 消息矩阵第 c 行
/// （primary sliver c）。逻辑符号 `(row r, col c)` 的平坦偏移为 `(c*667+r)*s`，
/// `s = pack_size`。opening 的 `column_symbols` 携带 sampled primary sliver 的
/// 全部 667 个 symbols；调用本函数前，opening 校验已一次重建 primary 树并
/// 将这些 symbols 绑定到 c_cipher 的承诺链。本函数：
/// 1. 使用已经完成承诺校验的完整 primary sliver；
/// 2. 重算列聚合：数据区用打开的真实符号，超界/尾部用密钥流
///    （与 `create_honest_proof` 逐字节一致），与 `proof.cipher_shard` 比较。
pub fn verify_cipher_column_bound(
    key: &[u8; 32],
    aux_data: &[u8],
    opening: &WalrusBlobOpening,
    proofs: &[crate::rslh_ve::VeShardProof],
    pack_size: usize,
) -> Result<(), &'static str> {
    use crate::rslh_ve::{
        derive_rslh_nonce, gf256_mul_walrus, COL_HEIGHT_SECONDARY, GF_EXP, ROW_WIDTH_PRIMARY,
    };
    use chacha20::cipher::{KeyIvInit, StreamCipher, StreamCipherSeek};
    use chacha20::{ChaCha8, Key, Nonce};

    if proofs.len() != opening.shards.len() {
        return Err("rslh_walrus: proof/opening sample count mismatch");
    }
    if opening.n_shards != 1000 {
        return Err("rslh_walrus: only 1000-shard RSLH-VE supported");
    }
    let data_len = opening.unencoded_length as usize;
    let cols = COL_HEIGHT_SECONDARY as usize;
    // s 必须与 blob 长度推导一致（guest 侧用 walrus_symbol_size(blob_len) 重算）
    if pack_size != crate::rslh_ve::walrus_symbol_size(opening.unencoded_length) {
        return Err("rslh_walrus: pack size mismatch");
    }
    let nonce = derive_rslh_nonce(key, aux_data);

    for (proof, shard) in proofs.iter().zip(&opening.shards) {
        let c = proof.col_index as usize;
        if c >= ROW_WIDTH_PRIMARY as usize {
            return Err("rslh_walrus: col index out of range");
        }
        if shard.shard_index as usize != c {
            return Err("rslh_walrus: shard/column mismatch");
        }
        if proof.cipher_shard.len() != pack_size || proof.origin_shard.len() != pack_size {
            return Err("rslh_walrus: shard length mismatch");
        }

        if shard.column_symbols.len() != cols {
            return Err("rslh_walrus: full primary sliver length mismatch");
        }

        let mut expected = vec![0u8; pack_size];
        let mut chacha = ChaCha8::new(Key::from_slice(key), Nonce::from_slice(&nonce));
        for r in 0..cols {
            let byte_off = (c * cols + r) * pack_size;
            let beta = GF_EXP[(r % 255) as usize];
            let in_data = byte_off < data_len;
            let data_end = (byte_off + pack_size).min(data_len);
            if in_data {
                let sym = &shard.column_symbols[r];
                if sym.len() != pack_size {
                    return Err("rslh_walrus: full primary sliver symbol size mismatch");
                }
                for j in 0..(data_end - byte_off) {
                    expected[j] ^= gf256_mul_walrus(sym[j], beta);
                }
            }
            let ks_start = if in_data { data_end - byte_off } else { 0 };
            let mut block = vec![0u8; pack_size];
            chacha.seek(byte_off as u64);
            chacha.apply_keystream(&mut block);
            for j in ks_start..pack_size {
                expected[j] ^= gf256_mul_walrus(block[j], beta);
            }
        }
        if expected[..] != proof.cipher_shard[..] {
            return Err("rslh_walrus: cipher column aggregate mismatch");
        }
    }
    Ok(())
}

/// 把 RSLH origin_shard 绑定到公开原文 blob 的真实列符号。
/// Walrus RS2 和 create_honest_proof 对原文尾部都使用零填充。
pub fn verify_origin_column_bound(
    opening: &WalrusBlobOpening,
    proofs: &[crate::rslh_ve::VeShardProof],
    pack_size: usize,
) -> Result<(), &'static str> {
    use crate::rslh_ve::{gf256_mul_walrus, COL_HEIGHT_SECONDARY, GF_EXP, ROW_WIDTH_PRIMARY};

    if proofs.len() != opening.shards.len() {
        return Err("rslh_walrus: origin proof/opening sample count mismatch");
    }
    if opening.n_shards != 1000 {
        return Err("rslh_walrus: only 1000-shard RSLH-VE supported");
    }
    if pack_size != crate::rslh_ve::walrus_symbol_size(opening.unencoded_length) {
        return Err("rslh_walrus: origin pack size mismatch");
    }

    let data_len = opening.unencoded_length as usize;
    let rows = COL_HEIGHT_SECONDARY as usize;
    for (proof, shard) in proofs.iter().zip(&opening.shards) {
        let column = proof.col_index as usize;
        if column >= ROW_WIDTH_PRIMARY as usize {
            return Err("rslh_walrus: origin column index out of range");
        }
        if shard.shard_index as usize != column {
            return Err("rslh_walrus: origin shard/column mismatch");
        }
        if proof.origin_shard.len() != pack_size {
            return Err("rslh_walrus: origin shard length mismatch");
        }

        if shard.column_symbols.len() != rows {
            return Err("rslh_walrus: origin full primary sliver length mismatch");
        }

        let mut expected = vec![0u8; pack_size];
        for (row, symbol) in shard.column_symbols.iter().take(rows).enumerate() {
            let byte_off = (column * rows + row) * pack_size;
            if byte_off >= data_len {
                break;
            }
            if symbol.len() != pack_size {
                return Err("rslh_walrus: origin full primary sliver symbol size mismatch");
            }
            let meaningful = (data_len - byte_off).min(pack_size);
            let beta = GF_EXP[row % 255];
            for i in 0..meaningful {
                expected[i] ^= gf256_mul_walrus(symbol[i], beta);
            }
        }
        if expected != proof.origin_shard {
            return Err("rslh_walrus: origin column aggregate mismatch");
        }
    }
    Ok(())
}

/// 为密文数据构造 blob 打开（host 侧构造器，vdd-script 与 drop-script 共用）。
///
/// 每个采样 i（`idx = sha256(seed||i) % 1000`，RSLH 列 `c = idx % 334`）对应
/// Walrus 消息矩阵第 c 行（primary sliver c）：
/// - `column_symbols`：sampled primary sliver 的完整 667 个 symbols；
/// - `primary_symbols`/`secondary_symbols`：该 shard 树的少量示例叶子；
/// - 对叶打开绑定 root pair c → 对叶树 → blob id。
#[cfg(not(feature = "guest"))]
fn build_column_blob_opening(
    blob_data: &[u8],
    seed: &[u8; 32],
    pack_size: usize,
) -> Result<WalrusBlobOpening, &'static str> {
    use crate::rslh_ve::{COL_HEIGHT_SECONDARY, DEFAULT_SAMPLE_COUNT, ROW_WIDTH_PRIMARY};
    use core::num::NonZeroU16;
    use sha2::{Digest, Sha256};
    use walrus_core::{
        encoding::{EncodingConfig, EncodingFactory as _},
        metadata::BlobMetadataApi as _,
    };

    let encoding_config =
        EncodingConfig::new(NonZeroU16::new(1000).expect("n_shards must be nonzero"))
            .get_for_type(EncodingType::RS2);
    let (metadata_with_id, symbol_hashes) = encoding_config
        .compute_metadata_with_symbol_hashes(blob_data)
        .map_err(|_| "walrus_open: blob metadata computation failed")?;
    let (slivers, encoded_metadata) = encoding_config
        .encode_with_metadata(blob_data.to_vec())
        .map_err(|_| "walrus_open: blob encoding failed")?;
    if metadata_with_id.blob_id() != encoded_metadata.blob_id() {
        return Err("walrus_open: metadata/blob id divergence");
    }
    if pack_size != crate::rslh_ve::walrus_symbol_size(blob_data.len() as u64) {
        return Err("walrus_open: pack size mismatch");
    }
    if pack_size >= 65535 {
        return Err("walrus_open: pack size out of walrus range");
    }

    let mut samples = Vec::with_capacity(DEFAULT_SAMPLE_COUNT);
    for i in 0..DEFAULT_SAMPLE_COUNT {
        let mut h = Sha256::new();
        h.update(seed);
        h.update(&(i as u32).to_le_bytes());
        let idx = u32::from_le_bytes(h.finalize()[0..4].try_into().expect("4 bytes")) % 1000;
        let c = idx % ROW_WIDTH_PRIMARY;
        let c_idx = c as usize;

        let column_symbols = (0..COL_HEIGHT_SECONDARY as usize)
            .map(|r| WalrusSymbolRequest {
                leaf_index: r as u32,
                symbol: slivers[c_idx].primary.symbols[r].to_vec(),
            })
            .collect::<Vec<_>>();

        samples.push(WalrusSampleRequest {
            shard_index: c,
            // primary sliver c 树的示例叶（leaf 0 始终存在）
            primary_symbols: vec![WalrusSymbolRequest {
                leaf_index: 0,
                symbol: slivers[c_idx].primary.symbols[0].to_vec(),
            }],
            secondary_symbols: vec![WalrusSymbolRequest {
                leaf_index: 0,
                symbol: slivers[c_idx].secondary.symbols[0].to_vec(),
            }],
            column_symbols,
        });
    }

    build_walrus_blob_opening(
        &symbol_hashes,
        metadata_with_id.metadata().encoding_type(),
        metadata_with_id.metadata().unencoded_length(),
        &samples,
    )
}

#[cfg(not(feature = "guest"))]
pub fn build_origin_blob_opening(
    origin_data: &[u8],
    seed: &[u8; 32],
    pack_size: usize,
) -> Result<WalrusBlobOpening, &'static str> {
    build_column_blob_opening(origin_data, seed, pack_size)
}

#[cfg(not(feature = "guest"))]
pub fn build_cipher_blob_opening(
    cipher_data: &[u8],
    seed: &[u8; 32],
    pack_size: usize,
) -> Result<WalrusBlobOpening, &'static str> {
    build_column_blob_opening(cipher_data, seed, pack_size)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::num::NonZeroU16;
    use sha2::Digest as _;
    use walrus_core::{
        encoding::{EncodingConfig, EncodingFactory as _, SliverPair},
        metadata::BlobMetadataApi as _,
    };

    fn blob_id_of(data: &[u8]) -> [u8; 32] {
        (*crate::walrus_address::compute_blob_id_default(data)
            .expect("blob id computation should succeed")
            .as_ref())
        .try_into()
        .expect("32 bytes")
    }

    fn encoded_fixture(
        data: &[u8],
    ) -> (
        Vec<SliverPair>,
        walrus_core::metadata::VerifiedBlobMetadataWithId,
        Vec<Node>,
        usize,
    ) {
        let n_shards = NonZeroU16::new(1000).expect("n_shards must be nonzero");
        let encoding_config = EncodingConfig::new(n_shards).get_for_type(EncodingType::RS2);
        let (metadata, symbol_hashes) = encoding_config
            .compute_metadata_with_symbol_hashes(data)
            .expect("metadata computation should succeed");
        let (slivers, encoded_metadata) = encoding_config
            .encode_with_metadata(data.to_vec())
            .expect("encoding should succeed");
        assert_eq!(metadata.blob_id(), encoded_metadata.blob_id());
        (slivers, metadata, symbol_hashes, usize::from(n_shards.get()))
    }

    fn sample_requests(slivers: &[SliverPair], n_shards: usize) -> Vec<WalrusSampleRequest> {
        // primary 叶号必须在 [0, 667)，secondary 叶号必须在 [0, 334)。
        [(0usize, 0usize), (333, 5), (999, 666)]
            .into_iter()
            .map(|(shard, primary_leaf)| WalrusSampleRequest {
                shard_index: shard as u32,
                primary_symbols: vec![WalrusSymbolRequest {
                    leaf_index: primary_leaf as u32,
                    symbol: slivers[shard].primary.symbols[primary_leaf].to_vec(),
                }],
                // pair p 存储的 secondary = 矩阵列 n_shards-1-p（expanded_column_symbols 逆序），
                // 因此 shard s 的 secondary 列 (n-1-s) 的符号位于 slivers[s].secondary。
                secondary_symbols: vec![WalrusSymbolRequest {
                    leaf_index: 7,
                    symbol: slivers[shard].secondary.symbols[7].to_vec(),
                }],
                column_symbols: (0..slivers[shard].primary.symbols.len())
                    .map(|leaf| WalrusSymbolRequest {
                        leaf_index: leaf as u32,
                        symbol: slivers[shard].primary.symbols[leaf].to_vec(),
                    })
                    .collect(),
            })
            .collect()
    }

    fn build_fixture_opening(
        metadata: &walrus_core::metadata::VerifiedBlobMetadataWithId,
        symbol_hashes: &[Node],
        samples: &[WalrusSampleRequest],
    ) -> WalrusBlobOpening {
        build_walrus_blob_opening(
            symbol_hashes,
            metadata.metadata().encoding_type(),
            metadata.metadata().unencoded_length(),
            samples,
        )
        .expect("opening construction should succeed")
    }

    #[test]
    fn openings_bind_symbols_to_blob_id_correctly() {
        let data: Vec<u8> = (0u8..=255).cycle().take(200 * 1024).collect();
        let (slivers, metadata, symbol_hashes, n_shards) = encoded_fixture(&data);
        let samples = sample_requests(&slivers, n_shards);
        let opening = build_fixture_opening(&metadata, &symbol_hashes, &samples);

        assert_eq!(
            &opening.blob_id[..],
            metadata.blob_id().as_ref(),
            "blob id must match walrus-core computation"
        );
        assert_eq!(opening.n_shards as usize, n_shards);
        assert_eq!(opening.encoding_type, u8::from(EncodingType::RS2));
        assert_eq!(opening.unencoded_length, metadata.metadata().unencoded_length());
        assert_eq!(opening.sliver_pair_roots.len(), n_shards);

        let hashes = metadata.metadata().hashes();
        assert_eq!(opening.sliver_pair_roots.len(), hashes.len());
        for (i, roots) in opening.sliver_pair_roots.iter().enumerate() {
            assert_eq!(roots.primary, hashes[i].primary_hash.bytes());
            assert_eq!(roots.secondary, hashes[i].secondary_hash.bytes());
        }

        let pair_tree_root = metadata.metadata().compute_root_hash();
        for (sample, shard) in samples.iter().zip(&opening.shards) {
            let i = sample.shard_index as usize;
            assert_eq!(shard.shard_index as usize, i);
            assert_eq!(shard.primary_root, hashes[i].primary_hash.bytes());
            assert_eq!(shard.secondary_root, hashes[i].secondary_hash.bytes());

            for (request, symbol_opening) in sample
                .primary_symbols
                .iter()
                .zip(shard.primary_symbols.iter())
            {
                MerkleProof::<Blake2b256>::new(&symbol_opening.path)
                    .verify_proof(
                        &Node::Digest(shard.primary_root),
                        n_shards,
                        &request.symbol,
                        request.leaf_index as usize,
                    )
                    .expect("primary symbol opening must verify against primary root");
            }

            for (request, symbol_opening) in sample
                .secondary_symbols
                .iter()
                .zip(shard.secondary_symbols.iter())
            {
                MerkleProof::<Blake2b256>::new(&symbol_opening.path)
                    .verify_proof(
                        &Node::Digest(shard.secondary_root),
                        n_shards,
                        &request.symbol,
                        request.leaf_index as usize,
                    )
                    .expect("secondary symbol opening must verify against secondary root");
            }

            let mut pair_leaf = [0u8; 64];
            pair_leaf[..32].copy_from_slice(&shard.primary_root);
            pair_leaf[32..].copy_from_slice(&shard.secondary_root);
            MerkleProof::<Blake2b256>::new(&shard.pair_leaf_path)
                .verify_proof(&pair_tree_root, n_shards, &pair_leaf, i)
                .expect("pair leaf opening must verify against pair tree root");
        }
    }

    #[test]
    fn tampered_symbol_fails_opening_verification() {
        let data: Vec<u8> = (0u8..=255).cycle().take(128 * 1024).collect();
        let (slivers, metadata, symbol_hashes, n_shards) = encoded_fixture(&data);
        let mut samples = sample_requests(&slivers, n_shards);
        assert!(!samples[0].primary_symbols[0].symbol.is_empty());
        samples[0].primary_symbols[0].symbol[0] ^= 0x01;

        let opening = build_fixture_opening(&metadata, &symbol_hashes, &samples);
        let shard = &opening.shards[0];
        let tampered_symbol = &samples[0].primary_symbols[0].symbol;
        let result = MerkleProof::<Blake2b256>::new(&shard.primary_symbols[0].path).verify_proof(
            &Node::Digest(shard.primary_root),
            n_shards,
            tampered_symbol,
            samples[0].primary_symbols[0].leaf_index as usize,
        );
        assert!(result.is_err(), "tampered symbol must fail the opening");
    }

    #[test]
    fn rejects_out_of_range_leaf() {
        let data = vec![1u8; 1024];
        let (slivers, metadata, symbol_hashes, n_shards) = encoded_fixture(&data);
        let mut samples = sample_requests(&slivers, n_shards);
        samples[0].primary_symbols[0].leaf_index = n_shards as u32;

        let result = build_walrus_blob_opening(
            &symbol_hashes,
            metadata.metadata().encoding_type(),
            metadata.metadata().unencoded_length(),
            &samples,
        );
        assert!(result.is_err(), "out-of-range leaf must be rejected");
    }

    #[test]
    fn verify_accepts_honest_opening() {
        let data: Vec<u8> = (0u8..=255).cycle().take(160 * 1024).collect();
        let (slivers, metadata, symbol_hashes, n_shards) = encoded_fixture(&data);
        let samples = sample_requests(&slivers, n_shards);
        let opening = build_fixture_opening(&metadata, &symbol_hashes, &samples);

        verify_walrus_blob_opening(&opening).expect("honest opening must verify");
        assert_eq!(opening.n_shards as usize, n_shards);
    }

    #[test]
    fn verify_rejects_tampered_openings() {
        let data: Vec<u8> = (0u8..=255).cycle().take(96 * 1024).collect();
        let (slivers, metadata, symbol_hashes, n_shards) = encoded_fixture(&data);
        let samples = sample_requests(&slivers, n_shards);

        // (a) 篡改 blob id
        let mut opening = build_fixture_opening(&metadata, &symbol_hashes, &samples);
        assert!(!opening.blob_id.is_empty());
        opening.blob_id[0] ^= 0x01;
        assert!(
            verify_walrus_blob_opening(&opening).is_err(),
            "tampered blob id must fail"
        );

        // (b) 篡改 sliver pair 根（对应对叶树根与 blob id 均改变）
        let mut opening = build_fixture_opening(&metadata, &symbol_hashes, &samples);
        opening.sliver_pair_roots[0].primary[0] ^= 0x01;
        assert!(
            verify_walrus_blob_opening(&opening).is_err(),
            "tampered pair root must fail"
        );

        // (c) 篡改 shard 打开里的符号
        let mut opening = build_fixture_opening(&metadata, &symbol_hashes, &samples);
        assert!(!opening.shards[0].primary_symbols[0].symbol.is_empty());
        opening.shards[0].primary_symbols[0].symbol[0] ^= 0x01;
        assert!(
            verify_walrus_blob_opening(&opening).is_err(),
            "tampered symbol must fail"
        );

        // (d) 篡改 shard 根，使其与 sliver_pair_roots 不一致
        let mut opening = build_fixture_opening(&metadata, &symbol_hashes, &samples);
        opening.shards[0].primary_root[0] ^= 0x01;
        assert!(
            verify_walrus_blob_opening(&opening).is_err(),
            "root/roots mismatch must fail"
        );

        // (e) n_shards 与 roots 长度不一致
        let mut opening = build_fixture_opening(&metadata, &symbol_hashes, &samples);
        opening.n_shards = n_shards as u32 + 1;
        assert!(
            verify_walrus_blob_opening(&opening).is_err(),
            "n_shards mismatch must fail"
        );
    }

    #[test]
    fn walrus_symbol_layout_matches_data() {
        // 几何前提：primary sliver c 的第 r 个符号 = 平坦字节区
        // [(c*667+r)*s, +(s))，超出 blob 长度补零 —— 这是 RSLH 列绑定
        // （RSLH 列 c = Walrus 消息矩阵行 c）与 c_cipher 一致的基础。
        use crate::rslh_ve::{COL_HEIGHT_SECONDARY, ROW_WIDTH_PRIMARY};
        let data: Vec<u8> = (0u8..=255).cycle().take(180 * 1024).collect();
        let (slivers, _, _, _) = encoded_fixture(&data);
        let s = crate::rslh_ve::walrus_symbol_size(data.len() as u64);
        let cols = COL_HEIGHT_SECONDARY as usize;
        let mut checked = 0usize;
        for c in 0..ROW_WIDTH_PRIMARY as usize {
            for r in 0..cols {
                let off = (c * cols + r) * s;
                let mut expect = vec![0u8; s];
                let take = data.len().saturating_sub(off).min(s);
                if take > 0 {
                    expect[..take].copy_from_slice(&data[off..off + take]);
                }
                let got = slivers[c].primary.symbols[r].to_vec();
                assert_eq!(got.len(), s, "symbol byte length must equal pack size");
                assert_eq!(
                    &expect[..],
                    &got[..],
                    "row {} col {} mismatch: off={} got={:?} expect={:?}",
                    c,
                    r,
                    off,
                    &got[..got.len().min(16)],
                    &expect[..expect.len().min(16)],
                );
                checked += 1;
            }
        }
        assert_eq!(checked, ROW_WIDTH_PRIMARY as usize * cols);
    }

    #[test]
    fn cipher_column_bound_rejects_tampered_cipher() {
        use crate::rslh_ve::{create_honest_proof, derive_rslh_nonce, DEFAULT_SAMPLE_COUNT};
        use sha2::Digest as _;

        let data: Vec<u8> = (0u8..=255).cycle().take(180 * 1024).collect();
        let mut seed_h = sha2::Sha256::new();
        seed_h.update(&[7u8; 32]);
        let seed: [u8; 32] = seed_h.finalize().into();

        let aux = b"trustdrop_asset_v1";
        let key = [0x5au8; 32];
        let nonce = derive_rslh_nonce(&key, aux);
        let mut cipher = data.clone();
        {
            use chacha20::cipher::{KeyIvInit, StreamCipher};
            use chacha20::{ChaCha8, Key, Nonce};
            let mut c = ChaCha8::new(Key::from_slice(&key), Nonce::from_slice(&nonce));
            c.apply_keystream(&mut cipher);
        }

        let s = crate::rslh_ve::walrus_symbol_size(cipher.len() as u64);
        let origin_opening = build_origin_blob_opening(&data, &seed, s).expect("origin opening");
        let opening = build_cipher_blob_opening(&cipher, &seed, s).expect("opening");
        let mut proofs = Vec::with_capacity(DEFAULT_SAMPLE_COUNT);
        for i in 0..DEFAULT_SAMPLE_COUNT {
            let mut h = sha2::Sha256::new();
            h.update(&seed);
            h.update(&(i as u32).to_le_bytes());
            let idx = u32::from_le_bytes(h.finalize()[0..4].try_into().unwrap()) % 1000;
            proofs.push(create_honest_proof(&key, &nonce, idx, s, &data, &cipher));
        }
        verify_cipher_column_bound(&key, aux, &opening, &proofs, s)
            .expect("honest cipher must bind");
        verify_origin_column_bound(&origin_opening, &proofs, s)
            .expect("honest origin must bind");
        verify_walrus_blob_opening(&opening).expect("opening must verify");

        // A self-consistent proof derived from uncommitted plaintext must fail
        // against the opening for c_origin.
        let mut uncommitted_origin = data.clone();
        for b in uncommitted_origin.iter_mut() {
            *b ^= 0xff;
        }
        let mut bad_origin_proofs = Vec::with_capacity(DEFAULT_SAMPLE_COUNT);
        for i in 0..DEFAULT_SAMPLE_COUNT {
            let mut h = sha2::Sha256::new();
            h.update(&seed);
            h.update(&(i as u32).to_le_bytes());
            let idx = u32::from_le_bytes(h.finalize()[0..4].try_into().unwrap()) % 1000;
            bad_origin_proofs.push(create_honest_proof(
                &key,
                &nonce,
                idx,
                s,
                &uncommitted_origin,
                &cipher,
            ));
        }
        assert!(
            verify_origin_column_bound(&origin_opening, &bad_origin_proofs, s).is_err(),
            "uncommitted origin proofs must be rejected by the binding"
        );

        // 篡改密文数据，用同一批采样生成“错误密文”的证明 → 绑定必须失败
        let mut tampered = cipher.clone();
        for b in tampered.iter_mut() {
            *b ^= 0xff;
        }
        let mut bad_proofs = Vec::with_capacity(DEFAULT_SAMPLE_COUNT);
        for i in 0..DEFAULT_SAMPLE_COUNT {
            let mut h = sha2::Sha256::new();
            h.update(&seed);
            h.update(&(i as u32).to_le_bytes());
            let idx = u32::from_le_bytes(h.finalize()[0..4].try_into().unwrap()) % 1000;
            bad_proofs.push(create_honest_proof(&key, &nonce, idx, s, &data, &tampered));
        }
        assert!(
            verify_cipher_column_bound(&key, aux, &opening, &bad_proofs, s).is_err(),
            "tampered cipher proofs must be rejected by the binding"
        );
    }

    #[test]
    fn cipher_opening_builder_matches_host_blob_id() {
        let data: Vec<u8> = (0u8..=255).cycle().take(180 * 1024).collect();
        let mut seed_h = sha2::Sha256::new();
        seed_h.update(&[1u8; 32]);
        let seed: [u8; 32] = seed_h.finalize().into();

        let s = crate::rslh_ve::walrus_symbol_size(data.len() as u64);
        let opening = build_cipher_blob_opening(&data, &seed, s).expect("cipher opening builder");
        assert_eq!(opening.blob_id[..], blob_id_of(&data), "builder blob id mismatch");
        verify_walrus_blob_opening(&opening).expect("builder output must verify");
    }
}
