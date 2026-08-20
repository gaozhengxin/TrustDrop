use chacha20::cipher::{KeyIvInit, StreamCipher};
use chacha20::{ChaCha8, Key, Nonce};
use clap::{Parser, ValueEnum};
use rand::{rng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sp1_sdk::network::NetworkMode;
use sp1_sdk::{
    include_elf, Elf, HashableKey, ProveRequest, Prover, ProverClient, ProvingKey,
    SP1ProofWithPublicValues, SP1Stdin, SP1VerifyingKey,
};
use std::path::PathBuf;

// 引用你的 lib 逻辑
use drop_lib::rslh_ve::{
    create_honest_proof, derive_rslh_nonce, walrus_symbol_size, DEFAULT_SAMPLE_COUNT,
};
use drop_lib::walrus_address::compute_blob_id_default;
use drop_lib::walrus_open::build_cipher_blob_opening;

/// ELF of the Walrus VDD program
pub const VDD_WALRUS_RSLHVE_ELF: Elf = include_elf!("program-vdd-walrus-rslhve");

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct EVMArgs {
    #[arg(long, value_enum, default_value = "groth16")]
    system: ProofSystem,
    #[arg(long)]
    idle: bool,
    /// 复用已有 asset 文件作为原始数据；缺省时生成随机数据。
    #[arg(long)]
    asset: Option<PathBuf>,
    /// 负例测试：cipher=提交真实 blob id 但用篡改后的密文生成证明输入；key=提交真实 c_key 但输入篡改密钥。
    #[arg(long, value_enum, default_value = "none")]
    neg_case: NegCase,
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum, Debug)]
enum NegCase {
    None,
    Cipher,
    Key,
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum, Debug)]
enum ProofSystem {
    Plonk,
    Groth16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SP1VDDProofFixture {
    c_origin: String,
    c_key: String,
    c_cipher: String,
    vkey: String,
    public_values: String,
    proof: String,
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    sp1_sdk::utils::setup_logger();

    let args = EVMArgs::parse();

    // 初始化 Network Prover 客户端
    let client = ProverClient::builder()
        .network_for(NetworkMode::Mainnet)
        .build()
        .await;

    // --- 1. 准备输入数据（复用已有 asset 或随机生成） ---
    let mut origin_data = match &args.asset {
        Some(path) => std::fs::read(path).expect("failed to read asset file"),
        None => {
            const DEFAULT_DATA_SIZE: usize = 64 * 1024;
            let data_size = std::env::var("VDD_RSLHVE_DATA_SIZE")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(DEFAULT_DATA_SIZE);
            let mut data = vec![0u8; data_size];
            rand::thread_rng().fill_bytes(&mut data);
            data
        }
    };
    if origin_data.is_empty() {
        eprintln!("error: asset/data must not be empty");
        std::process::exit(1);
    }

    let mut key = [0u8; 32];
    rng().fill_bytes(&mut key);

    // 计算三方绑定承诺
    let c_origin = compute_blob_id_default(&origin_data).unwrap();
    let c_origin_bytes: [u8; 32] = (*c_origin.as_ref()).try_into().unwrap();

    let c_key_bytes: [u8; 32] = *blake3::hash(&key).as_bytes();

    let aux_data = b"trustdrop_asset_v1";
    let nonce = derive_rslh_nonce(&key, aux_data);
    let symbol_size = walrus_symbol_size(origin_data.len() as u64);

    // 线性加密
    let mut cipher_data = origin_data.clone();
    let mut cipher_stream = ChaCha8::new(Key::from_slice(&key), Nonce::from_slice(&nonce));
    cipher_stream.apply_keystream(&mut cipher_data);

    let c_cipher = compute_blob_id_default(&cipher_data).unwrap();
    let c_cipher_bytes: [u8; 32] = (*c_cipher.as_ref()).try_into().unwrap();

    // 与 RSLH-VE 采样相同种子
    let mut seed_h = Sha256::new();
    seed_h.update(&c_origin_bytes);
    seed_h.update(&c_cipher_bytes);
    seed_h.update(&c_key_bytes);
    let seed: [u8; 32] = seed_h.finalize().into();

    // Walrus 承诺打开（基于真实密文）
    let cipher_opening = build_cipher_blob_opening(&cipher_data, &seed, symbol_size)
        .expect("cipher blob opening construction failed");
    assert_eq!(
        &cipher_opening.blob_id[..],
        c_cipher.as_ref(),
        "opening blob id must match c_cipher"
    );

    // --- 2. 构建 SP1 Stdin ---
    let mut stdin = SP1Stdin::new();
    stdin.write(&c_origin_bytes);
    stdin.write(&c_cipher_bytes);
    stdin.write(&c_key_bytes);
    stdin.write(&aux_data.to_vec());

    if args.neg_case == NegCase::Key {
        key[0] ^= 0xFF;
    }
    stdin.write(&key);

    // 负例 cipher：证明数据使用被篡改的密文（公开承诺与打开仍为真实密文）
    let mut proof_cipher_data = cipher_data.clone();
    if args.neg_case == NegCase::Cipher {
        for b in proof_cipher_data.iter_mut() {
            *b ^= 0xFF;
        }
    }

    for i in 0..DEFAULT_SAMPLE_COUNT {
        let mut h = Sha256::new();
        h.update(&seed);
        h.update(&(i as u32).to_le_bytes());
        let idx = u32::from_le_bytes(h.finalize()[0..4].try_into().unwrap()) % 1000;

        let proof = create_honest_proof(
            &key,
            &nonce,
            idx,
            symbol_size,
            &origin_data,
            &proof_cipher_data,
        );
        stdin.write(&proof.global_index);
        stdin.write(&proof.origin_shard);
        stdin.write(&proof.cipher_shard);
    }

    stdin.write(&cipher_opening);

    // --- 3. Setup & Prove ---
    let pk = client.setup(VDD_WALRUS_RSLHVE_ELF).await.unwrap();
    println!(
        "SP1 Setup finished. Program VKey: {}",
        pk.verifying_key().bytes32()
    );

    if args.idle {
        write_fixture_data(
            &c_origin_bytes,
            &c_key_bytes,
            &c_cipher_bytes,
            None,
            &pk.verifying_key(),
            args.system,
        );
        return;
    }

    println!(
        "Submitting proof request to SP1 Network ({:?}, neg_case={:?})...",
        args.system, args.neg_case
    );

    let proof_result = match args.system {
        ProofSystem::Plonk => client.prove(&pk, stdin).skip_simulation(true).compressed().plonk().await,
        ProofSystem::Groth16 => client.prove(&pk, stdin).skip_simulation(true).compressed().groth16().await,
    };

    let proof = match proof_result {
        Ok(proof) if args.neg_case != NegCase::None => {
            eprintln!(
                "FAIL: negative case {:?} unexpectedly produced a proof",
                args.neg_case
            );
            std::process::exit(1);
        }
        Ok(proof) => proof,
        Err(error) if args.neg_case != NegCase::None => {
            println!(
                "✔ Negative case {:?} correctly rejected by prove network: {error:?}",
                args.neg_case
            );
            std::process::exit(0);
        }
        Err(error) => {
            eprintln!("failed to generate proof: {error:?}");
            std::process::exit(1);
        }
    };

    println!("✔ Network proof generated successfully!");

    // --- 4. 保存 Fixture ---
    write_fixture_data(
        &c_origin_bytes,
        &c_key_bytes,
        &c_cipher_bytes,
        Some(&proof),
        &pk.verifying_key(),
        args.system,
    );
}

fn write_fixture_data(
    c_origin: &[u8; 32],
    c_key: &[u8; 32],
    c_cipher: &[u8; 32],
    proof_data: Option<&SP1ProofWithPublicValues>,
    vk: &SP1VerifyingKey,
    system: ProofSystem,
) {
    let (public_values_hex, proof_hex) = if let Some(p) = proof_data {
        (
            format!("0x{}", hex::encode(p.public_values.as_slice())),
            format!("0x{}", hex::encode(p.bytes())),
        )
    } else {
        ("0x".to_string(), "0x".to_string())
    };

    let fixture = SP1VDDProofFixture {
        c_origin: format!("0x{}", hex::encode(c_origin)),
        c_key: format!("0x{}", hex::encode(c_key)),
        c_cipher: format!("0x{}", hex::encode(c_cipher)),
        vkey: vk.bytes32().to_string(),
        public_values: public_values_hex,
        proof: proof_hex,
    };

    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../contracts/src/fixtures");
    std::fs::create_dir_all(&fixture_path).expect("failed to create fixture directory");

    let filename = format!("vdd-walrus-rslh-{:?}-fixture.json", system).to_lowercase();

    std::fs::write(
        fixture_path.join(&filename),
        serde_json::to_string_pretty(&fixture).unwrap(),
    )
    .expect("failed to write fixture");

    println!("✔ Fixture saved to contracts/src/fixtures/{}", filename);
}
