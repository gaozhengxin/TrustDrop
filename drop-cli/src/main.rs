use anyhow::{anyhow, bail, Result};
use drop_lib::rslh_ve::{derive_rslh_nonce, SYMBOL_SIZE};
use drop_sdk::{
    abi::{exchange_channel_contract as channel_abi, exchange_hub_contract as hub_abi},
    chacha8::chacha8_encrypt,
    config::DropCliConfig,
    oracle::OracleWorkerClient,
    state::{default_state_dir, load_sale_state, save_sale_state, SaleState, TxRecord, TxStatus},
    walrus::compute_rs_id,
};
use ethers::abi::RawLog;
use ethers::prelude::*;
use k256::{elliptic_curve::sec1::ToEncodedPoint, SecretKey};
use sha3::{Digest, Keccak256};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use storage::{StorageNetwork, WalrusClient, WalrusConfig};

const DEFAULT_ENV_FILE: &str = "drop-script/.env";
const ARBITRUM_SEPOLIA_CHAIN_ID: u64 = 421614;

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        print_help();
        return Ok(());
    }

    match args[0].as_str() {
        "init" => cmd_init(&args[1..]),
        "doctor" => cmd_doctor().await,
        "status" => cmd_status(&args[1..]).await,
        "next" => cmd_next(&args[1..]).await,
        "oracle" => cmd_oracle(&args[1..]).await,
        "asset" => cmd_asset(&args[1..]).await,
        "channel" => cmd_channel(&args[1..]).await,
        "sale" => cmd_sale(&args[1..]).await,
        "proof" => cmd_proof(&args[1..]).await,
        "settle" => cmd_settle(&args[1..]).await,
        "recover-test" => cmd_recover_test(&args[1..]).await,
        "phase" => cmd_phase(&args[1..]).await,
        "tx" => cmd_tx(&args[1..]).await,
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        command => bail!("unknown command: {command}"),
    }
}

fn print_help() {
    println!(
        r#"drop-cli

Usage:
  drop-cli init
  drop-cli doctor
  drop-cli status <sale-id>
  drop-cli next <sale-id>
  drop-cli oracle check <sale-id|--blob-id <id>|--c-cipher <0x...>>
  drop-cli asset prepare <file>
  drop-cli asset upload <sale-id>
  drop-cli channel create
  drop-cli sale list <sale-id>
  drop-cli sale submit-key-commitment <sale-id>
  drop-cli proof vss <sale-id>
  drop-cli proof vdd <sale-id>
  drop-cli settle <sale-id>
  drop-cli recover-test <sale-id>
  drop-cli phase prepare <file>
  drop-cli phase publish <sale-id>
  drop-cli phase complete-test-flow <sale-id>
  drop-cli phase prove <sale-id>
  drop-cli phase settle <sale-id>
  drop-cli phase verify <sale-id>
  drop-cli tx status <tx-hash>
  drop-cli tx resume <sale-id>

Prototype target:
  Arbitrum Sepolia + centralized Oracle Worker.
"#
    );
}

fn cmd_init(_args: &[String]) -> Result<()> {
    let config = load_config()?;
    let dir = state_dir(&config)?;
    fs::create_dir_all(&dir)?;
    println!("created state dir: {}", dir.display());
    println!("config source for prototype: {}", config_source());
    println!("next: drop-cli doctor");
    Ok(())
}

async fn cmd_doctor() -> Result<()> {
    let config = load_config()?;
    println!("drop-cli doctor");
    println!("chain: arbitrum-sepolia ({})", config.chain_id);
    if config.chain_id != ARBITRUM_SEPOLIA_CHAIN_ID {
        println!("WARN chain id is not {ARBITRUM_SEPOLIA_CHAIN_ID}");
    }

    match &config.rpc_url {
        Some(rpc_url) => {
            let provider = Provider::<Http>::try_from(rpc_url.as_str())?;
            match provider.get_chainid().await {
                Ok(chain_id) => println!("PASS rpc chain id: {chain_id}"),
                Err(error) => println!("WARN rpc check failed: {error}"),
            }
        }
        None => println!("WARN ARBITRUM_SEPOLIA_RPC_URL is missing"),
    }

    if config.hub_address.is_some() {
        println!("PASS HUB_ADDRESS configured");
    } else {
        println!("WARN HUB_ADDRESS is missing");
    }

    let worker = oracle_worker(&config)?;
    match worker.health().await {
        Ok(true) => println!("PASS oracle worker health"),
        Ok(false) => println!("WARN oracle worker health returned non-success"),
        Err(error) => println!("WARN oracle worker health failed: {error}"),
    }
    match worker.status().await {
        Ok(status) => {
            println!("oracle worker ok: {}", status.ok);
            println!(
                "oracle relayer ready: matches={}, balance={}, pending={}",
                status.relayer_matches_oracle_proxy.unwrap_or(false),
                status.relayer_balance_sufficient.unwrap_or(false),
                status.relayer_has_pending_tx.unwrap_or(true)
            );
        }
        Err(error) => println!("WARN oracle worker status failed: {error}"),
    }

    println!("state dir: {}", state_dir(&config)?.display());
    println!("next: drop-cli phase prepare <file>");
    Ok(())
}

async fn cmd_status(args: &[String]) -> Result<()> {
    let sale_id = require_arg(args, "sale-id")?;
    let config = load_config()?;
    let state = load_sale_state(state_dir(&config)?, sale_id)?;
    print_state(&state);
    Ok(())
}

async fn cmd_next(args: &[String]) -> Result<()> {
    let sale_id = require_arg(args, "sale-id")?;
    let config = load_config()?;
    let state = load_sale_state(state_dir(&config)?, sale_id)?;
    if state.next_actions.is_empty() {
        println!("No next action recorded.");
        println!("{}", infer_next_action(&state));
    } else {
        println!("Next actions:");
        for action in &state.next_actions {
            println!("  {action}");
        }
    }
    Ok(())
}

async fn cmd_oracle(args: &[String]) -> Result<()> {
    if args.first().map(String::as_str) != Some("check") {
        bail!("usage: drop-cli oracle check <sale-id|--blob-id <id>|--c-cipher <0x...>>");
    }
    let config = load_config()?;
    let worker = oracle_worker(&config)?;
    let rest = &args[1..];
    let status = if rest.first().map(String::as_str) == Some("--blob-id") {
        let blob_id = require_arg(&rest[1..], "blob-id")?;
        worker.blob_status_by_blob_id(blob_id).await?
    } else if rest.first().map(String::as_str) == Some("--c-cipher") {
        let c_cipher = require_arg(&rest[1..], "c-cipher")?;
        worker.blob_status_by_c_cipher(c_cipher).await?
    } else {
        let sale_id = require_arg(rest, "sale-id")?;
        let state = load_sale_state(state_dir(&config)?, sale_id)?;
        if let Some(blob_id) = state.walrus_blob_id.as_deref() {
            worker.blob_status_by_blob_id(blob_id).await?
        } else if let Some(c_cipher) = state.encrypted_blob_id.as_deref() {
            worker.blob_status_by_c_cipher(c_cipher).await?
        } else {
            bail!("state has neither walrus_blob_id nor encrypted_blob_id");
        }
    };
    println!("{}", serde_json::to_string_pretty(&status)?);
    Ok(())
}

async fn cmd_asset(args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("prepare") => {
            let file = require_arg(&args[1..], "file")?;
            asset_prepare(file)
        }
        Some("upload") => {
            let sale_id = require_arg(&args[1..], "sale-id")?;
            asset_upload(sale_id).await
        }
        _ => bail!("usage: drop-cli asset prepare <file> | asset upload <sale-id>"),
    }
}

async fn asset_upload(sale_id: &str) -> Result<()> {
    let config = load_config()?;
    let state_dir = state_dir(&config)?;
    let mut state = load_sale_state(&state_dir, sale_id)?;
    if let Some(blob_id) = &state.walrus_blob_id {
        println!("walrusBlobId already recorded: {blob_id}");
        println!("next: drop-cli oracle check {sale_id}");
        return Ok(());
    }

    let encrypted_asset_path = state
        .encrypted_asset_path
        .as_deref()
        .ok_or_else(|| anyhow!("state missing encrypted_asset_path; run drop-cli asset prepare"))?;
    let encrypted_payload = fs::read(encrypted_asset_path)?;
    let publisher_url = config
        .walrus_publisher_url
        .clone()
        .unwrap_or_else(|| "http://localhost:31415".to_string());
    let aggregator_url = config
        .walrus_aggregator_url
        .clone()
        .unwrap_or_else(|| publisher_url.clone());
    let walrus = WalrusClient::new(WalrusConfig {
        aggregator_url,
        publisher_url,
        api_key: String::new(),
        blockberry_base: String::new(),
        send_object_to: None,
    });

    println!("Uploading encrypted asset to Walrus. This consumes Walrus storage.");
    let blob_id = walrus
        .upload_blob(encrypted_payload.into(), Some("1"))
        .await
        .map_err(|error| anyhow!("walrus upload failed: {}", error))?
        .0;
    state.walrus_blob_id = Some(blob_id.clone());
    state.next_actions = vec![format!("drop-cli oracle check {sale_id}")];
    save_sale_state(&state_dir, &state)?;
    println!("walrusBlobId: {blob_id}");

    if let Ok(worker) = oracle_worker(&config) {
        match worker.blob_status_by_blob_id(&blob_id).await {
            Ok(status) => {
                println!("oracleBlobStatus: {}", status.status_name);
                println!("endEpoch: {:?}", status.end_epoch);
            }
            Err(error) => println!("WARN oracle blob status check failed: {error}"),
        }
    }
    println!("next: drop-cli phase publish {sale_id}");
    Ok(())
}

async fn channel_create(sale_id: Option<&str>) -> Result<()> {
    let config = load_config()?;
    let client = signer_client(&config).await?;
    let hub_address = parse_address(
        config
            .hub_address
            .as_deref()
            .ok_or_else(|| anyhow!("HUB_ADDRESS is missing"))?,
    )?;
    let hub = hub_abi::ExchangeHubContract::new(hub_address, client);
    let owner_pubkey = owner_public_key_bytes(&config.owner_secret_key)?;
    let pubkey = hub_abi::Pubkey {
        data: owner_pubkey.into(),
    };

    println!("sending createExchangeChannel transaction...");
    let call = hub.create_exchange_channel(pubkey);
    let pending = call.send().await?;
    let tx_hash = pending.tx_hash();
    println!("txHash: {tx_hash:?}");
    let receipt = pending
        .await?
        .ok_or_else(|| anyhow!("create channel receipt missing"))?;
    if receipt.status != Some(U64::from(1u64)) {
        bail!(
            "create channel transaction reverted: {:?}",
            receipt.transaction_hash
        );
    }

    let mut channel_address = None;
    for log in &receipt.logs {
        if let Ok(parsed) =
            <hub_abi::ExchangeChannelCreatedFilter as ethers::contract::EthEvent>::decode_log(
                &RawLog {
                    topics: log.topics.clone(),
                    data: log.data.to_vec(),
                },
            )
        {
            channel_address = Some(parsed.channel);
            break;
        }
    }
    let channel_address =
        channel_address.ok_or_else(|| anyhow!("ExchangeChannelCreated event not found"))?;
    println!("channel: {channel_address:?}");

    if let Some(sale_id) = sale_id {
        let state_dir = state_dir(&config)?;
        let mut state = load_sale_state(&state_dir, sale_id)?;
        state.channel_address = Some(format!("{channel_address:?}"));
        state.transactions.push(tx_record(
            "channel_create",
            Some(format!("{:?}", receipt.transaction_hash)),
            TxStatus::Confirmed,
            receipt.block_number.map(|value| value.as_u64()),
        ));
        state.next_actions = vec![format!("drop-cli sale list {sale_id} --yes")];
        save_sale_state(state_dir, &state)?;
        println!("state updated");
    }
    Ok(())
}

async fn sale_list(sale_id: &str) -> Result<String> {
    let config = load_config()?;
    let state_dir = state_dir(&config)?;
    let mut state = load_sale_state(&state_dir, sale_id)?;
    let channel_address = parse_address(state.channel_address.as_deref().ok_or_else(|| {
        anyhow!("state missing channel_address; run drop-cli channel create {sale_id} --yes")
    })?)?;
    let original_asset_id = parse_hex32_state(
        state
            .original_asset_id
            .as_deref()
            .ok_or_else(|| anyhow!("state missing original_asset_id"))?,
    )?;
    let client = signer_client(&config).await?;
    let channel = channel_abi::ExchangeChannelContract::new(channel_address, client);
    let nonce = channel.nonce().call().await?;
    let onchain_sale_id = compute_sale_id(channel_address, config.chain_id, nonce);
    let data_version = ethers::utils::keccak256(original_asset_id);
    let commitment = channel_abi::DataCommitment {
        data: original_asset_id.to_vec().into(),
    };
    let price = U256::from(10u128.pow(16));
    let info = "TrustDrop Asset v1".to_string();

    println!("sending listFile transaction...");
    let call = channel.list_file(commitment, price, info);
    let pending = call.send().await?;
    let tx_hash = pending.tx_hash();
    println!("txHash: {tx_hash:?}");
    let receipt = pending
        .await?
        .ok_or_else(|| anyhow!("list file receipt missing"))?;
    if receipt.status != Some(U64::from(1u64)) {
        bail!(
            "list file transaction reverted: {:?}",
            receipt.transaction_hash
        );
    }

    state.sale_id = format!("0x{}", hex::encode(onchain_sale_id));
    state.data_version = Some(format!("0x{}", hex::encode(data_version)));
    state.transactions.push(tx_record(
        "sale_list",
        Some(format!("{:?}", receipt.transaction_hash)),
        TxStatus::Confirmed,
        receipt.block_number.map(|value| value.as_u64()),
    ));
    state.next_actions = vec![format!("drop-cli proof vss {} --yes", state.sale_id)];
    save_sale_state(&state_dir, &state)?;
    println!("saleId: {}", state.sale_id);
    println!(
        "dataVersion: {}",
        state.data_version.as_deref().unwrap_or("-")
    );
    println!("state updated");
    Ok(state.sale_id)
}

async fn sale_submit_key_commitment(sale_id: &str) -> Result<()> {
    let config = load_config()?;
    let state_dir = state_dir(&config)?;
    let mut state = load_sale_state(&state_dir, sale_id)?;
    let channel_address = parse_address(state.channel_address.as_deref().ok_or_else(|| {
        anyhow!("state missing channel_address; run drop-cli phase publish <local-sale-id> --yes")
    })?)?;
    let commitment = *blake3::hash(&config.asset_encryption_key).as_bytes();
    let client = signer_client(&config).await?;
    let channel = channel_abi::ExchangeChannelContract::new(channel_address, client);
    let current = channel.data_key_commitment().call().await?;

    if current == commitment {
        state.data_commitment = Some(format!("0x{}", hex::encode(commitment)));
        state.next_actions = vec![format!(
            "drop-cli phase complete-test-flow {sale_id} --yes"
        )];
        save_sale_state(state_dir, &state)?;
        println!("dataKeyCommitment already set: 0x{}", hex::encode(commitment));
        return Ok(());
    }
    if current != [0u8; 32] {
        bail!(
            "channel dataKeyCommitment mismatch: onchain=0x{}, expected=0x{}",
            hex::encode(current),
            hex::encode(commitment)
        );
    }

    println!("sending submitDataKeyCommitment transaction...");
    let call = channel.submit_data_key_commitment(commitment.into());
    let pending = call.send().await?;
    let tx_hash = pending.tx_hash();
    println!("txHash: {tx_hash:?}");
    let receipt = pending
        .await?
        .ok_or_else(|| anyhow!("submitDataKeyCommitment receipt missing"))?;
    if receipt.status != Some(U64::from(1u64)) {
        bail!(
            "submitDataKeyCommitment transaction reverted: {:?}",
            receipt.transaction_hash
        );
    }

    state.data_commitment = Some(format!("0x{}", hex::encode(commitment)));
    state.transactions.push(tx_record(
        "submit_data_key_commitment",
        Some(format!("{:?}", receipt.transaction_hash)),
        TxStatus::Confirmed,
        receipt.block_number.map(|value| value.as_u64()),
    ));
    state.next_actions = vec![format!(
        "drop-cli phase complete-test-flow {sale_id} --yes"
    )];
    save_sale_state(&state_dir, &state)?;
    println!("dataKeyCommitment: 0x{}", hex::encode(commitment));
    println!("state updated");
    Ok(())
}

async fn cmd_channel(args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("create") => {
            let sale_id = args
                .get(1)
                .filter(|value| !value.starts_with("--"))
                .map(String::as_str);
            if !has_flag(args, "--yes") {
                println!("channel create requires --yes to send an Arbitrum Sepolia transaction.");
                println!("usage: drop-cli channel create [sale-id] --yes");
                return Ok(());
            }
            channel_create(sale_id).await?;
            Ok(())
        }
        _ => bail!("usage: drop-cli channel create [sale-id] --yes"),
    }
}

async fn cmd_sale(args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("list") => {
            let sale_id = require_arg(&args[1..], "sale-id")?;
            if !has_flag(args, "--yes") {
                println!("sale list requires --yes to send an Arbitrum Sepolia transaction.");
                println!("usage: drop-cli sale list <sale-id> --yes");
                return Ok(());
            }
            sale_list(sale_id).await?;
            Ok(())
        }
        Some("submit-key-commitment") => {
            let sale_id = require_arg(&args[1..], "sale-id")?;
            if !has_flag(args, "--yes") {
                println!(
                    "sale submit-key-commitment requires --yes to send an Arbitrum Sepolia transaction."
                );
                println!("usage: drop-cli sale submit-key-commitment <sale-id> --yes");
                return Ok(());
            }
            sale_submit_key_commitment(sale_id).await?;
            Ok(())
        }
        _ => bail!("usage: drop-cli sale list <sale-id> --yes | sale submit-key-commitment <sale-id> --yes"),
    }
}

async fn cmd_proof(args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("vss") | Some("vdd") => {
            let proof_kind = args[0].as_str();
            let sale_id = require_arg(&args[1..], "sale-id")?;
            println!("proof {proof_kind} is planned but not implemented yet for {sale_id}.");
            println!("This command will use SP1 Prove Network when implemented.");
            println!("Local proving is not used by default.");
            Ok(())
        }
        _ => bail!("usage: drop-cli proof vss|vdd <sale-id>"),
    }
}

async fn cmd_settle(args: &[String]) -> Result<()> {
    let sale_id = require_arg(args, "sale-id")?;
    println!("settle is planned but not implemented yet for {sale_id}.");
    println!("This command will send an Arbitrum Sepolia transaction when implemented.");
    Ok(())
}

async fn cmd_recover_test(args: &[String]) -> Result<()> {
    let sale_id = require_arg(args, "sale-id")?;
    println!("recover-test is planned but not implemented yet for {sale_id}.");
    println!("This command is for development verification only.");
    Ok(())
}

async fn cmd_phase(args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("prepare") => {
            let file = require_arg(&args[1..], "file")?;
            asset_prepare(file)
        }
        Some("publish") => {
            let sale_id = require_arg(&args[1..], "sale-id")?;
            if !has_flag(args, "--yes") {
                println!("phase publish requires --yes because it uploads to Walrus and sends chain transactions.");
                println!("usage: drop-cli phase publish <sale-id> --yes");
                return Ok(());
            }
            asset_upload(sale_id).await?;
            channel_create(Some(sale_id)).await?;
            let onchain_sale_id = sale_list(sale_id).await?;
            sale_submit_key_commitment(&onchain_sale_id).await?;
            println!("phasePublishSaleId: {onchain_sale_id}");
            println!("next: drop-cli phase complete-test-flow {onchain_sale_id} --yes");
            Ok(())
        }
        Some("complete-test-flow") => {
            let sale_id = require_arg(&args[1..], "sale-id")?;
            if !has_flag(args, "--yes") {
                println!(
                    "phase complete-test-flow requires --yes because it sends buyer/seller/oracle/settle transactions and requests SP1 network proofs."
                );
                println!("usage: drop-cli phase complete-test-flow <sale-id> --yes");
                return Ok(());
            }
            complete_test_flow(sale_id).await
        }
        Some("prove") | Some("settle") | Some("verify") => {
            let phase = args[0].as_str();
            let sale_id = require_arg(&args[1..], "sale-id")?;
            println!("phase {phase} is planned but not implemented yet for {sale_id}");
            println!("next: drop-cli status {sale_id}");
            Ok(())
        }
        _ => bail!("usage: drop-cli phase prepare <file> | publish|complete-test-flow|prove|settle|verify <sale-id>"),
    }
}

async fn cmd_tx(args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("status") => {
            let tx_hash = require_arg(&args[1..], "tx-hash")?;
            let config = load_config()?;
            let rpc_url = config
                .rpc_url
                .as_deref()
                .ok_or_else(|| anyhow!("ARBITRUM_SEPOLIA_RPC_URL is missing"))?;
            let provider = Provider::<Http>::try_from(rpc_url)?;
            let hash: H256 = tx_hash.parse()?;
            match provider.get_transaction_receipt(hash).await? {
                Some(receipt) => {
                    println!("txHash: {tx_hash}");
                    println!("blockNumber: {:?}", receipt.block_number);
                    println!("status: {:?}", receipt.status);
                }
                None => println!("tx pending or unknown: {tx_hash}"),
            }
            Ok(())
        }
        Some("resume") => {
            let sale_id = require_arg(&args[1..], "sale-id")?;
            println!("tx resume currently refreshes through: drop-cli status {sale_id}");
            Ok(())
        }
        _ => bail!("usage: drop-cli tx status <tx-hash> | tx resume <sale-id>"),
    }
}

async fn complete_test_flow(sale_id: &str) -> Result<()> {
    let config = load_config()?;
    let state_dir = state_dir(&config)?;
    let state = load_sale_state(&state_dir, sale_id)?;
    if state.channel_address.is_none()
        || state.walrus_blob_id.is_none()
        || state.data_version.is_none()
        || state.original_asset_id.is_none()
        || state.encrypted_blob_id.is_none()
    {
        bail!("sale state is not ready; run drop-cli phase publish <sale-id> --yes first");
    }

    println!("running prototype complete test flow through drop-script implementation...");
    println!("This sends buyer purchase, requests sale-bound VSS/VDD proofs, fulfills, triggers oracle, waits, and settles.");
    let mut command = Command::new("cargo");
    command
        .arg("run")
        .arg("-p")
        .arg("drop-script")
        .arg("--bin")
        .arg("resume_drop_cli_sale")
        .arg("--")
        .arg(sale_id)
        .env("DROP_CLI_STATE_DIR", state_dir)
        .env("DROP_SCRIPT_INPUT_ASSET", state.input_asset_path.unwrap_or_default())
        .env("ORACLE_MODE", "centralized");

    let status = command.status()?;
    if !status.success() {
        bail!("complete test flow failed with status: {status}");
    }
    println!("complete test flow finished");
    Ok(())
}

fn asset_prepare(file: &str) -> Result<()> {
    let config = load_config()?;
    let mut payload = fs::read(file)?;
    let original_len = payload.len();
    let padded_len = (original_len + SYMBOL_SIZE - 1) / SYMBOL_SIZE * SYMBOL_SIZE;
    payload.resize(padded_len, 0);

    let original_asset_id = compute_rs_id(&payload)?;
    let asset_nonce = derive_rslh_nonce(&config.asset_encryption_key, b"maenad_v1");
    let encrypted_payload =
        chacha8_encrypt(&payload, &config.asset_encryption_key, &asset_nonce, 0)?;
    let encrypted_blob_id = compute_rs_id(&encrypted_payload)?;

    let sale_id = sale_id_from_asset_id(&original_asset_id);
    let encrypted_asset_path = encrypted_asset_path(&config, &sale_id)?;
    if let Some(parent) = encrypted_asset_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&encrypted_asset_path, encrypted_payload)?;

    let mut state = SaleState::new(&sale_id);
    state.input_asset_path = Some(file.to_string());
    state.original_asset_id = Some(format!("0x{}", hex::encode(original_asset_id)));
    state.encrypted_blob_id = Some(format!("0x{}", hex::encode(encrypted_blob_id)));
    state.encrypted_asset_path = Some(encrypted_asset_path.display().to_string());
    state.next_actions = vec![
        format!("drop-cli asset upload {sale_id}"),
        format!("drop-cli phase publish {sale_id}"),
    ];
    save_sale_state(state_dir(&config)?, &state)?;
    println!("saleId: {sale_id}");
    println!("inputAsset: {file}");
    println!("originalLength: {original_len}");
    println!("paddedLength: {padded_len}");
    println!("originalAssetId: 0x{}", hex::encode(original_asset_id));
    println!("encryptedBlobId: 0x{}", hex::encode(encrypted_blob_id));
    println!("encryptedAsset: {}", encrypted_asset_path.display());
    println!("state saved");
    println!("next: drop-cli phase publish {sale_id}");
    Ok(())
}

fn encrypted_asset_path(config: &DropCliConfig, sale_id: &str) -> Result<PathBuf> {
    Ok(state_dir(config)?
        .join("assets")
        .join(format!("{}.cipher", sanitize_path_component(sale_id))))
}

fn sanitize_path_component(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn sale_id_from_asset_id(asset_id: &[u8; 32]) -> String {
    let mut hasher = Keccak256::new();
    hasher.update(b"drop-cli-sale");
    hasher.update(asset_id);
    format!("0x{}", hex::encode(hasher.finalize()))
}

fn compute_sale_id(channel_address: Address, chain_id: u64, nonce: U256) -> [u8; 32] {
    let mut packed_bytes = Vec::new();
    packed_bytes.extend_from_slice(channel_address.as_bytes());
    let mut chain_id_bytes = [0u8; 32];
    U256::from(chain_id).to_big_endian(&mut chain_id_bytes);
    packed_bytes.extend_from_slice(&chain_id_bytes);
    let mut nonce_bytes = [0u8; 32];
    nonce.to_big_endian(&mut nonce_bytes);
    packed_bytes.extend_from_slice(&nonce_bytes);
    ethers::utils::keccak256(packed_bytes)
}

async fn signer_client(
    config: &DropCliConfig,
) -> Result<Arc<SignerMiddleware<Provider<Http>, LocalWallet>>> {
    let rpc_url = config
        .rpc_url
        .as_deref()
        .ok_or_else(|| anyhow!("ARBITRUM_SEPOLIA_RPC_URL is missing"))?;
    let key = config
        .seller_private_key
        .as_deref()
        .ok_or_else(|| anyhow!("SELLER_KEY is missing"))?;
    let provider = Provider::<Http>::try_from(rpc_url)?;
    let wallet = key.parse::<LocalWallet>()?.with_chain_id(config.chain_id);
    Ok(Arc::new(SignerMiddleware::new(provider, wallet)))
}

fn owner_public_key_bytes(sk_bytes: &[u8; 32]) -> Result<Vec<u8>> {
    let sk = SecretKey::from_slice(sk_bytes)
        .map_err(|error| anyhow!("invalid owner secret key: {error}"))?;
    Ok(sk.public_key().to_encoded_point(true).as_bytes().to_vec())
}

fn parse_address(value: &str) -> Result<Address> {
    value.parse::<Address>().map_err(|error| anyhow!("{error}"))
}

fn parse_hex32_state(value: &str) -> Result<[u8; 32]> {
    let clean = value.strip_prefix("0x").unwrap_or(value);
    let bytes = hex::decode(clean)?;
    bytes
        .try_into()
        .map_err(|_| anyhow!("expected 32-byte hex value"))
}

fn tx_record(
    kind: &str,
    tx_hash: Option<String>,
    status: TxStatus,
    block_number: Option<u64>,
) -> TxRecord {
    let now = unix_timestamp_string();
    TxRecord {
        id: format!("{}_{}", kind, now),
        kind: kind.to_string(),
        chain_id: ARBITRUM_SEPOLIA_CHAIN_ID,
        tx_hash,
        status,
        required_confirmations: 1,
        block_number,
        receipt_status: None,
        created_at: now.clone(),
        updated_at: now,
        next_command: None,
    }
}

fn unix_timestamp_string() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|arg| arg == flag)
}

fn print_state(state: &SaleState) {
    println!("saleId: {}", state.sale_id);
    println!(
        "channel: {}",
        state.channel_address.as_deref().unwrap_or("-")
    );
    println!(
        "input: {}",
        state.input_asset_path.as_deref().unwrap_or("-")
    );
    println!(
        "originalAssetId: {}",
        state.original_asset_id.as_deref().unwrap_or("-")
    );
    println!(
        "encryptedBlobId: {}",
        state.encrypted_blob_id.as_deref().unwrap_or("-")
    );
    println!(
        "encryptedAsset: {}",
        state.encrypted_asset_path.as_deref().unwrap_or("-")
    );
    println!(
        "walrusBlobId: {}",
        state.walrus_blob_id.as_deref().unwrap_or("-")
    );
    if let Some(error) = &state.last_error {
        println!("lastError: {error}");
    }
    if !state.transactions.is_empty() {
        println!("transactions:");
        for tx in &state.transactions {
            println!("  {} {:?} {:?}", tx.kind, tx.status, tx.tx_hash);
        }
    }
    println!("{}", infer_next_action(state));
}

fn infer_next_action(state: &SaleState) -> String {
    if state.input_asset_path.is_none() {
        return "next: drop-cli phase prepare <file>".to_string();
    }
    if state.walrus_blob_id.is_none() {
        return format!("next: drop-cli phase publish {}", state.sale_id);
    }
    if state.encrypted_blob_id.is_none() {
        return format!("next: drop-cli asset upload {}", state.sale_id);
    }
    format!("next: drop-cli phase prove {}", state.sale_id)
}

fn load_config() -> Result<DropCliConfig> {
    DropCliConfig::from_env_file(config_source())
}

fn config_source() -> String {
    env::var("DROP_CLI_ENV").unwrap_or_else(|_| DEFAULT_ENV_FILE.to_string())
}

fn state_dir(config: &DropCliConfig) -> Result<PathBuf> {
    match &config.state_dir {
        Some(path) => Ok(Path::new(path).to_path_buf()),
        None => default_state_dir(),
    }
}

fn oracle_worker(config: &DropCliConfig) -> Result<OracleWorkerClient> {
    Ok(OracleWorkerClient::new(
        config.default_oracle_worker_url(),
        config.require_oracle_worker_token()?,
    ))
}

fn require_arg<'a>(args: &'a [String], name: &str) -> Result<&'a str> {
    args.first()
        .map(String::as_str)
        .ok_or_else(|| anyhow!("missing {name}"))
}
