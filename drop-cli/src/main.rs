use anyhow::{anyhow, bail, Result};
use drop_lib::rslh_ve::{derive_rslh_nonce, SYMBOL_SIZE};
use drop_sdk::{
    abi::{exchange_channel_contract as channel_abi, exchange_hub_contract as hub_abi},
    chacha8::chacha8_encrypt,
    config::DropCliConfig,
    oracle::OracleWorkerClient,
    state::{
        default_state_dir, load_all_sale_states, load_all_thread_states, load_sale_state,
        load_thread_state, save_sale_state, save_thread_state, thread_state_dir,
        PurchaseContextRecord, SaleState, ThreadPurchase, ThreadState, ThreadStatus, TxRecord,
        TxStatus,
    },
    walrus::{compute_rs_id, upload_data_idempotent_with_end_epoch},
};
use ethers::abi::RawLog;
use ethers::prelude::*;
use k256::{elliptic_curve::sec1::ToEncodedPoint, SecretKey};
use serde::Deserialize;
use serde_json::json;
use sha3::{Digest, Keccak256};
use std::{
    collections::HashSet,
    env, fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use storage::{WalrusClient, WalrusConfig};

const DEFAULT_ENV_FILE: &str = "drop-script/.env";
const ARBITRUM_SEPOLIA_CHAIN_ID: u64 = 421614;
const DEFAULT_SUBGRAPH_QUERY_URL: &str =
    "https://api.studio.thegraph.com/query/1722405/test-arbitrum-store/v0.0.12";

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
        "db" => cmd_db(&args[1..]),
        "keys" => cmd_keys(&args[1..]),
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
        "thread" => cmd_thread(&args[1..]).await,
        "purchase" => cmd_purchase(&args[1..]).await,
        "tui" => cmd_tui(&args[1..]),
        "daemon" => cmd_daemon(&args[1..]).await,
        "debug" => cmd_debug(&args[1..]).await,
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
  drop-cli db init|migrate|inspect
  drop-cli keys check
  drop-cli doctor
  drop-cli status <sale-id>
  drop-cli next <sale-id>
  drop-cli oracle check <sale-id|--blob-id <id>|--c-cipher <0x...>>
  drop-cli asset prepare <file>
  drop-cli asset upload <sale-id>
  drop-cli asset ensure <sale-id>
  drop-cli channel list|show <channel>|create
  drop-cli sale list [--channel <channel>] | show <sale-id>
  drop-cli sale list <sale-id> --yes
  drop-cli sale submit-key-commitment <sale-id>
  drop-cli purchase list [--channel <channel>] [--sale <sale-id>] [--status <status>]
  drop-cli purchase show <purchase-tx>
  drop-cli proof vss <sale-id> --yes
  drop-cli proof vdd <sale-id> --yes
  drop-cli settle <sale-id> --yes
  drop-cli thread list [--channel <channel>] [--sale <sale-id>]
  drop-cli thread show <thread-id>
  drop-cli thread cancel <thread-id>
  drop-cli tui
  drop-cli daemon run --once
  drop-cli recover-test <sale-id>
  drop-cli phase prepare <file>
  drop-cli phase publish <sale-id>
  drop-cli phase complete-test-flow <sale-id> --yes
  drop-cli phase respond <purchase-tx>...
  drop-cli phase fulfill <thread-id>
  drop-cli phase settle <thread-id|sale-id>
  drop-cli phase prove <sale-id> --yes
  drop-cli phase verify <sale-id>
  drop-cli tx status <tx-hash>
  drop-cli tx resume <sale-id>
  drop-cli debug thread resume <thread-id>

Prototype target:
  Arbitrum Sepolia + centralized Oracle Worker.
"#
    );
}

fn cmd_init(_args: &[String]) -> Result<()> {
    let config = load_config()?;
    let dir = state_dir(&config)?;
    fs::create_dir_all(&dir)?;
    fs::create_dir_all(thread_state_dir(&dir))?;
    println!("created state dir: {}", dir.display());
    println!("created thread dir: {}", thread_state_dir(&dir).display());
    println!("config source for prototype: {}", config_source());
    println!("next: drop-cli doctor");
    Ok(())
}

fn cmd_db(args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("init") | Some("migrate") => {
            let config = load_config()?;
            let dir = state_dir(&config)?;
            fs::create_dir_all(&dir)?;
            fs::create_dir_all(thread_state_dir(&dir))?;
            println!("stateDir: {}", dir.display());
            println!("threadDir: {}", thread_state_dir(&dir).display());
            println!("status: ready");
            Ok(())
        }
        Some("inspect") => {
            let config = load_config()?;
            let dir = state_dir(&config)?;
            let sales = load_all_sale_states(&dir)?;
            let threads = load_all_thread_states(&dir)?;
            println!("stateDir: {}", dir.display());
            println!("sales: {}", sales.len());
            println!("threads: {}", threads.len());
            println!("purchases: {}", collect_purchases(&sales).len());
            Ok(())
        }
        _ => bail!("usage: drop-cli db init|migrate|inspect"),
    }
}

fn cmd_keys(args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("check") => {
            let config = load_config()?;
            println!("drop-cli keys check");
            println!("config: {}", config_source());
            println!("chainId: {}", config.chain_id);

            let seller_key = config
                .seller_private_key
                .as_deref()
                .ok_or_else(|| anyhow!("SELLER_KEY is missing"))?;
            let seller_wallet = seller_key
                .parse::<LocalWallet>()?
                .with_chain_id(config.chain_id);
            println!("sellerAddress: {:?}", seller_wallet.address());

            let owner_secret_key = config.require_owner_secret_key()?;
            let owner_pubkey = owner_public_key_bytes(&owner_secret_key)?;
            println!("ownerPublicKey: 0x{}", hex::encode(owner_pubkey));

            let asset_encryption_key = config.require_asset_encryption_key()?;
            let commitment = *blake3::hash(&asset_encryption_key).as_bytes();
            println!("assetKeyCommitment: 0x{}", hex::encode(commitment));

            if config.dev_insecure_default_keys {
                println!(
                    "WARN TRUSTDROP_DEV_INSECURE_DEFAULT_KEYS is enabled; do not use this for seller production operations"
                );
            }
            println!("status: ready");
            Ok(())
        }
        _ => bail!("usage: drop-cli keys check"),
    }
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
        Some("ensure") => {
            let sale_id = require_arg(&args[1..], "sale-id")?;
            asset_upload(sale_id).await
        }
        _ => bail!("usage: drop-cli asset prepare <file> | asset upload|ensure <sale-id>"),
    }
}

async fn asset_upload(sale_id: &str) -> Result<()> {
    let config = load_config()?;
    let state_dir = state_dir(&config)?;
    let mut state = load_sale_state(&state_dir, sale_id)?;
    ensure_walrus_asset_available(&config, &state_dir, &mut state).await?;
    println!("next: drop-cli phase publish {sale_id}");
    Ok(())
}

async fn ensure_walrus_asset_available(
    config: &DropCliConfig,
    state_dir: &Path,
    state: &mut SaleState,
) -> Result<()> {
    if let Some(blob_id) = state.walrus_blob_id.as_deref() {
        println!("walrusBlobId recorded: {blob_id}");
        if walrus_blob_is_usable(config, blob_id).await? {
            wait_for_active_walrus_blob(config, blob_id).await?;
            return Ok(());
        }
        println!("walrusBlobId is unavailable or expired; attempting to re-upload local encrypted asset.");
    }

    let encrypted_asset_path = state
        .encrypted_asset_path
        .as_deref()
        .ok_or_else(|| anyhow!("state missing encrypted_asset_path; run drop-cli asset prepare on the seller machine before fulfilling"))?;
    let encrypted_payload = fs::read(encrypted_asset_path).map_err(|error| {
        anyhow!(
            "failed to read encrypted asset at {}: {}",
            encrypted_asset_path,
            error
        )
    })?;
    let walrus = drop_script_walrus_client(config);

    println!("Uploading encrypted asset to Walrus. This consumes Walrus storage.");
    let (blob_id, end_epoch) =
        upload_data_idempotent_with_end_epoch(&walrus, encrypted_payload).await?;
    let end_epoch = end_epoch.ok_or_else(|| anyhow!("Walrus upload did not return end epoch"))?;
    state.walrus_blob_id = Some(blob_id.clone());
    state.walrus_end_epoch = Some(end_epoch);
    state.next_actions = vec![format!("drop-cli oracle check {}", state.sale_id)];
    save_sale_state(state_dir, state)?;
    println!("walrusBlobId: {blob_id}");
    println!("walrusEndEpoch: {end_epoch}");

    wait_for_active_walrus_blob(config, &blob_id).await
}

async fn walrus_blob_is_usable(config: &DropCliConfig, blob_id: &str) -> Result<bool> {
    if !walrus_blob_is_retrievable(config, blob_id).await? {
        return Ok(false);
    }

    match oracle_worker(config) {
        Ok(worker) => match worker.blob_status_by_blob_id(blob_id).await {
            Ok(status) => Ok(status.found && !status.expired && status.status == 0),
            Err(error) => {
                println!("WARN oracle worker blob status check failed: {error}");
                Ok(true)
            }
        },
        Err(_) => Ok(true),
    }
}

async fn walrus_blob_is_retrievable(config: &DropCliConfig, blob_id: &str) -> Result<bool> {
    let aggregator_url = config
        .walrus_aggregator_url
        .clone()
        .or_else(|| config.walrus_publisher_url.clone())
        .unwrap_or_else(|| "http://localhost:31415".to_string());
    let status_url = format!(
        "{}/v1/blobs/{}",
        aggregator_url.trim_end_matches('/'),
        blob_id
    );
    let status = reqwest::Client::new()
        .head(&status_url)
        .send()
        .await?
        .status();
    Ok(status.is_success())
}

async fn wait_for_active_walrus_blob(config: &DropCliConfig, blob_id: &str) -> Result<()> {
    let aggregator_url = config
        .walrus_aggregator_url
        .clone()
        .or_else(|| config.walrus_publisher_url.clone())
        .unwrap_or_else(|| "http://localhost:31415".to_string());
    let status_url = format!(
        "{}/v1/blobs/{}",
        aggregator_url.trim_end_matches('/'),
        blob_id
    );
    let http = reqwest::Client::new();
    let attempts = env::var("DROP_CLI_WALRUS_STATUS_ATTEMPTS")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(24);
    let delay_secs = env::var("DROP_CLI_WALRUS_STATUS_DELAY_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(10);

    let mut last_status = None;
    for attempt in 1..=attempts {
        let status = http.head(&status_url).send().await?.status();
        println!(
            "walrusAggregatorStatusAttempt: {attempt}/{attempts} httpStatus={}",
            status.as_u16()
        );
        if status.is_success() {
            return Ok(());
        }
        last_status = Some(status.as_u16());
        if attempt < attempts {
            tokio::time::sleep(Duration::from_secs(delay_secs)).await;
        }
    }

    let status = last_status.ok_or_else(|| anyhow!("Walrus aggregator did not return a result"))?;
    bail!(
        "uploaded Walrus blob is not retrievable from configured aggregator after {} attempts: httpStatus={}",
        attempts,
        status
    );
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
    let owner_pubkey = owner_public_key_bytes(&config.require_owner_secret_key()?)?;
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
    let info = sale_metadata_json(&state)?;

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
    state.next_actions = vec![format!("drop-cli proof vdd {} --yes", state.sale_id)];
    save_sale_state(&state_dir, &state)?;
    println!("saleId: {}", state.sale_id);
    println!(
        "dataVersion: {}",
        state.data_version.as_deref().unwrap_or("-")
    );
    println!("state updated");
    Ok(state.sale_id)
}

fn sale_metadata_json(state: &SaleState) -> Result<String> {
    let input_path = state.input_asset_path.as_deref().unwrap_or("asset");
    let file_name = Path::new(input_path)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(input_path);
    let title = env::var("DROP_CLI_LIST_TITLE").unwrap_or_else(|_| file_name.to_string());
    let description = env::var("DROP_CLI_LIST_DESCRIPTION").unwrap_or_default();
    let content_type = env::var("DROP_CLI_LIST_CONTENT_TYPE")
        .unwrap_or_else(|_| guess_content_type(file_name).to_string());
    let tags = env::var("DROP_CLI_LIST_TAGS")
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    Ok(json!({
        "title": title,
        "description": description,
        "fileName": file_name,
        "fileSize": state.original_len.unwrap_or(0),
        "contentType": content_type,
        "tags": tags,
    })
    .to_string())
}

fn guess_content_type(file_name: &str) -> &'static str {
    let lower = file_name.to_ascii_lowercase();
    if lower.ends_with(".mp4") {
        "video/mp4"
    } else if lower.ends_with(".pdf") {
        "application/pdf"
    } else if lower.ends_with(".txt") {
        "text/plain"
    } else if lower.ends_with(".zip") {
        "application/zip"
    } else {
        "application/octet-stream"
    }
}

async fn sale_submit_key_commitment(sale_id: &str) -> Result<()> {
    let config = load_config()?;
    let state_dir = state_dir(&config)?;
    let mut state = load_sale_state(&state_dir, sale_id)?;
    let channel_address = parse_address(state.channel_address.as_deref().ok_or_else(|| {
        anyhow!("state missing channel_address; run drop-cli phase publish <local-sale-id> --yes")
    })?)?;
    let asset_encryption_key = config.require_asset_encryption_key()?;
    let commitment = *blake3::hash(&asset_encryption_key).as_bytes();
    let client = signer_client(&config).await?;
    let channel = channel_abi::ExchangeChannelContract::new(channel_address, client);
    let current = channel.data_key_commitment().call().await?;

    if current == commitment {
        state.data_commitment = Some(format!("0x{}", hex::encode(commitment)));
        state.next_actions = vec![format!("drop-cli proof vdd {sale_id} --yes")];
        save_sale_state(state_dir, &state)?;
        println!(
            "dataKeyCommitment already set: 0x{}",
            hex::encode(commitment)
        );
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
    state.next_actions = vec![format!("drop-cli proof vdd {sale_id} --yes")];
    save_sale_state(&state_dir, &state)?;
    println!("dataKeyCommitment: 0x{}", hex::encode(commitment));
    println!("state updated");
    Ok(())
}

async fn cmd_channel(args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("list") => {
            let config = load_config()?;
            let states = load_all_sale_states(state_dir(&config)?)?;
            let mut channels = Vec::<String>::new();
            for state in states {
                if let Some(channel) = state.channel_address {
                    if !channels.iter().any(|known| known.eq_ignore_ascii_case(&channel)) {
                        channels.push(channel);
                    }
                }
            }
            if channels.is_empty() {
                println!("No channels recorded locally.");
                println!("next: drop-cli phase publish <sale-id> --yes");
            } else {
                for channel in channels {
                    println!("channel: {channel}");
                    println!("next: drop-cli channel show {channel}");
                }
            }
            Ok(())
        }
        Some("show") => {
            let channel = require_arg(&args[1..], "channel")?;
            let config = load_config()?;
            let states = load_all_sale_states(state_dir(&config)?)?;
            println!("channel: {channel}");
            for state in states.iter().filter(|state| {
                state
                    .channel_address
                    .as_deref()
                    .map(|value| value.eq_ignore_ascii_case(channel))
                    .unwrap_or(false)
            }) {
                println!("  sale: {}", state.sale_id);
                println!("    walrusBlobId: {}", state.walrus_blob_id.as_deref().unwrap_or("-"));
                println!("    next: {}", infer_next_action(state).trim_start_matches("next: "));
            }
            println!("next: drop-cli purchase list --channel {channel}");
            Ok(())
        }
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
        _ => bail!("usage: drop-cli channel list | channel show <channel> | channel create [sale-id] --yes"),
    }
}

async fn cmd_sale(args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("list") => {
            let maybe_sale_id = args
                .get(1)
                .filter(|value| !value.starts_with("--"))
                .map(String::as_str);
            if has_flag(args, "--yes") {
                let sale_id = maybe_sale_id.ok_or_else(|| anyhow!("missing sale-id"))?;
                sale_list(sale_id).await?;
            } else if let Some(sale_id) = maybe_sale_id {
                println!("sale list <sale-id> requires --yes to send an Arbitrum Sepolia transaction.");
                println!("usage: drop-cli sale list <sale-id> --yes");
                println!("for local sale details use: drop-cli sale show {sale_id}");
            } else {
                list_sales(args)?;
            }
            Ok(())
        }
        Some("show") => {
            let sale_id = require_arg(&args[1..], "sale-id")?;
            let config = load_config()?;
            let state = load_sale_state(state_dir(&config)?, sale_id)?;
            print_state(&state);
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
        _ => bail!("usage: drop-cli sale list [--channel <channel>] | sale show <sale-id> | sale list <sale-id> --yes | sale submit-key-commitment <sale-id> --yes"),
    }
}

async fn cmd_purchase(args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("list") => list_purchases(args).await,
        Some("show") => {
            let tx_hash = require_arg(&args[1..], "purchase-tx")?;
            let config = load_config()?;
            let purchases = discover_purchases(&config, &args[1..]).await?;
            let purchase = purchases
                .into_iter()
                .find(|purchase| purchase.purchase_tx_hash.eq_ignore_ascii_case(tx_hash))
                .ok_or_else(|| anyhow!("purchase not found: {tx_hash}"))?;
            let purchase = enrich_purchase_or_assume_vss(&config, purchase).await;
            print_purchase(&purchase);
            println!("next: drop-cli phase respond {}", purchase.purchase_tx_hash);
            Ok(())
        }
        _ => bail!("usage: drop-cli purchase list [--channel <channel>] [--sale <sale-id>] [--status <status>] | purchase show <purchase-tx>"),
    }
}

async fn cmd_thread(args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("list") => list_threads(args),
        Some("show") => {
            let thread_id = require_arg(&args[1..], "thread-id")?;
            let config = load_config()?;
            let thread = load_thread_state(state_dir(&config)?, thread_id)?;
            print_thread(&thread);
            Ok(())
        }
        Some("cancel") => {
            let thread_id = require_arg(&args[1..], "thread-id")?;
            let config = load_config()?;
            let state_dir = state_dir(&config)?;
            let mut thread = load_thread_state(&state_dir, thread_id)?;
            thread.status = ThreadStatus::Canceled;
            thread.updated_at = unix_timestamp_string();
            thread.next_actions = Vec::new();
            save_thread_state(state_dir, &thread)?;
            println!("thread: {}", thread.thread_id);
            println!("status: canceled");
            Ok(())
        }
        _ => bail!("usage: drop-cli thread list [--channel <channel>] [--sale <sale-id>] | thread show <thread-id> | thread cancel <thread-id>"),
    }
}

fn cmd_tui(args: &[String]) -> Result<()> {
    if !args.is_empty() {
        bail!("usage: drop-cli tui");
    }
    let config = load_config()?;
    let state_dir = state_dir(&config)?;
    let sales = load_all_sale_states(&state_dir)?;
    let threads = load_all_thread_states(&state_dir)?;
    println!("TrustDrop seller console");
    println!("stateDir: {}", state_dir.display());
    println!("sales: {}", sales.len());
    println!("threads: {}", threads.len());
    println!();
    for state in &sales {
        println!("sale: {}", state.sale_id);
        println!(
            "  channel: {}",
            state.channel_address.as_deref().unwrap_or("-")
        );
        println!(
            "  walrus: {}",
            state.walrus_blob_id.as_deref().unwrap_or("-")
        );
        println!("  purchases: {}", state.purchases.len());
        println!(
            "  next: {}",
            infer_next_action(state).trim_start_matches("next: ")
        );
    }
    if !threads.is_empty() {
        println!();
        println!("threads:");
        for thread in threads {
            println!(
                "  {} {:?} sale={} purchases={}",
                thread.thread_id,
                thread.status,
                thread.sale_id,
                thread.purchases.len()
            );
            for action in thread.next_actions {
                println!("    next: {action}");
            }
        }
    }
    Ok(())
}

async fn cmd_daemon(args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("run") => {
            if has_flag(args, "--once") {
                daemon_run_once().await
            } else {
                daemon_run_forever().await
            }
        }
        _ => bail!("usage: drop-cli daemon run [--once]"),
    }
}

async fn daemon_run_once() -> Result<()> {
    daemon_tick(None).await
}

async fn daemon_run_forever() -> Result<()> {
    let interval_secs = env::var("DROP_CLI_DAEMON_INTERVAL_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(15)
        .max(5);
    println!("daemon: running");
    println!("intervalSecs: {interval_secs}");
    println!("mode: seller-channel subgraph discovery");
    let config = load_config()?;
    let baseline = discover_purchases(&config, &[]).await.unwrap_or_default();
    let mut seen_purchase_txs = baseline
        .into_iter()
        .map(|purchase| purchase.purchase_tx_hash.to_ascii_lowercase())
        .collect::<HashSet<_>>();
    println!("baselinePurchases: {}", seen_purchase_txs.len());
    loop {
        if let Err(error) = daemon_tick(Some(&mut seen_purchase_txs)).await {
            println!("daemonError: {error:#}");
        }
        tokio::time::sleep(Duration::from_secs(interval_secs)).await;
    }
}

async fn daemon_tick(mut seen_purchase_txs: Option<&mut HashSet<String>>) -> Result<()> {
    let config = load_config()?;
    let state_dir = state_dir(&config)?;
    let sales = load_all_sale_states(&state_dir)?;
    let local_sale_ids = sales
        .iter()
        .map(|state| state.sale_id.to_ascii_lowercase())
        .collect::<HashSet<_>>();
    let purchases = discover_purchases(&config, &[]).await?;
    println!("daemon: scan");
    println!("sales: {}", sales.len());
    println!("purchases: {}", purchases.len());
    let mut paid_purchases = purchases
        .into_iter()
        .filter(|purchase| {
            purchase.status == "paid"
                && local_sale_ids.contains(&purchase.sale_id.to_ascii_lowercase())
                && seen_purchase_txs
                    .as_ref()
                    .map(|seen| {
                        !seen.contains(&purchase.purchase_tx_hash.to_ascii_lowercase())
                    })
                    .unwrap_or(true)
        })
        .collect::<Vec<_>>();
    paid_purchases.sort_by(|left, right| left.purchase_tx_hash.cmp(&right.purchase_tx_hash));
    for purchase in &paid_purchases {
        println!("paidPurchase: {}", purchase.purchase_tx_hash);
        println!("  saleId: {}", purchase.sale_id);
        println!(
            "  channel: {}",
            purchase.channel_address.as_deref().unwrap_or("-")
        );
    }
    for purchase in paid_purchases {
        if let Some(seen) = seen_purchase_txs.as_deref_mut() {
            seen.insert(purchase.purchase_tx_hash.to_ascii_lowercase());
        }
        let purchase_tx_hash = purchase.purchase_tx_hash.clone();
        phase_respond(&[purchase_tx_hash.clone()]).await?;
        let thread = thread_for_purchase(&state_dir, &purchase.sale_id, &purchase_tx_hash)?
            .ok_or_else(|| anyhow!("thread not created for purchase {}", purchase.purchase_tx_hash))?;
        if !local_sale_ids.contains(&thread.sale_id.to_ascii_lowercase()) {
            println!(
                "daemonSkipThread: {} reason=missing-local-sale saleId={}",
                thread.thread_id, thread.sale_id
            );
            continue;
        }
        if matches!(thread.status, ThreadStatus::Blocked | ThreadStatus::Failed) {
            println!(
                "daemonBlockedThread: {} status={:?}",
                thread.thread_id, thread.status
            );
            continue;
        }
        if !matches!(thread.status, ThreadStatus::Completed | ThreadStatus::Canceled) {
            println!("daemonFulfillThread: {}", thread.thread_id);
            phase_fulfill(&thread.thread_id).await?;
            let refreshed = load_thread_state(&state_dir, &thread.thread_id)?;
            if refreshed.status == ThreadStatus::Fulfilled || refreshed.status == ThreadStatus::SettleReady {
                println!("daemonSettleThread: {}", refreshed.thread_id);
                phase_settle(&refreshed.thread_id).await?;
            }
        }
    }
    println!("daemon: complete");
    Ok(())
}

async fn cmd_debug(args: &[String]) -> Result<()> {
    match (
        args.first().map(String::as_str),
        args.get(1).map(String::as_str),
    ) {
        (Some("thread"), Some("resume")) => {
            let thread_id = require_arg(&args[2..], "thread-id")?;
            let config = load_config()?;
            let thread = load_thread_state(state_dir(&config)?, thread_id)?;
            println!("debug thread resume only refreshes local state and prints next action.");
            print_thread(&thread);
            Ok(())
        }
        _ => bail!("usage: drop-cli debug thread resume <thread-id>"),
    }
}

async fn cmd_proof(args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("vdd") => {
            let proof_kind = args[0].as_str();
            let sale_id = require_arg(&args[1..], "sale-id")?;
            if !has_flag(args, "--yes") {
                println!(
                    "proof {proof_kind} requires --yes because it requests SP1 Prove Network proof and sends an Arbitrum Sepolia transaction."
                );
                println!("usage: drop-cli proof {proof_kind} <sale-id> --yes");
                return Ok(());
            }
            submit_vdd_for_sale(sale_id).await?;
            Ok(())
        }
        Some("vss") => {
            let sale_id = require_arg(&args[1..], "sale-id")?;
            if !has_flag(args, "--yes") {
                println!(
                    "proof vss requires --yes because it requests SP1 Prove Network proof and sends a fulfill transaction."
                );
                println!("usage: drop-cli proof vss <sale-id> --yes");
                return Ok(());
            }
            fulfill_first_purchase_for_sale(sale_id).await?;
            Ok(())
        }
        _ => bail!("usage: drop-cli proof vss|vdd <sale-id> --yes"),
    }
}

async fn cmd_settle(args: &[String]) -> Result<()> {
    let sale_id = require_arg(args, "sale-id")?;
    if !has_flag(args, "--yes") {
        println!("settle requires --yes because it sends an Arbitrum Sepolia transaction.");
        println!("usage: drop-cli settle <sale-id> --yes");
        return Ok(());
    }
    settle_first_purchase_for_sale(sale_id).await?;
    Ok(())
}

async fn cmd_recover_test(args: &[String]) -> Result<()> {
    let sale_id = require_arg(args, "sale-id")?;
    recover_first_purchase_for_sale(sale_id).await?;
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
            println!("next: drop-cli proof vdd {onchain_sale_id} --yes");
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
        Some("respond") => phase_respond(&args[1..]).await,
        Some("fulfill") => {
            let thread_id = require_arg(&args[1..], "thread-id")?;
            phase_fulfill(thread_id).await
        }
        Some("settle") => {
            let id = require_arg(&args[1..], "thread-id|sale-id")?;
            if load_thread_state(state_dir(&load_config()?)?, id).is_ok() {
                phase_settle(id).await
            } else {
                settle_first_purchase_for_sale(id).await
            }
        }
        Some("prove") => {
            let sale_id = require_arg(&args[1..], "sale-id")?;
            if !has_flag(args, "--yes") {
                println!("phase prove requires --yes because it requests SP1 Prove Network proof and sends transactions.");
                println!("usage: drop-cli phase prove <sale-id> --yes");
                return Ok(());
            }
            submit_vdd_for_sale(sale_id).await
        }
        Some("verify") => {
            let phase = args[0].as_str();
            let sale_id = require_arg(&args[1..], "sale-id")?;
            println!("phase {phase} for {sale_id}");
            if has_confirmed_tx(&load_sale_state(state_dir(&load_config()?)?, sale_id)?, "settle") {
                println!("status: complete");
            } else {
                println!("status: not complete");
            }
            println!("next: drop-cli status {sale_id}");
            Ok(())
        }
        _ => bail!("usage: drop-cli phase prepare <file> | publish|complete-test-flow|prove|settle|verify <sale-id> | respond <purchase-tx>... | fulfill <thread-id>"),
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

#[derive(Debug, Clone)]
struct PurchaseView {
    purchase_tx_hash: String,
    sale_id: String,
    channel_address: Option<String>,
    buyer: Option<String>,
    needs_vss: Option<bool>,
    status: String,
    settle_tx_hash: Option<String>,
}

fn list_sales(args: &[String]) -> Result<()> {
    let config = load_config()?;
    let channel_filter = flag_value(args, "--channel");
    let states = load_all_sale_states(state_dir(&config)?)?;
    let mut count = 0usize;
    for state in states.iter().filter(|state| {
        channel_filter
            .map(|channel| {
                state
                    .channel_address
                    .as_deref()
                    .map(|value| value.eq_ignore_ascii_case(channel))
                    .unwrap_or(false)
            })
            .unwrap_or(true)
    }) {
        count += 1;
        println!("saleId: {}", state.sale_id);
        println!(
            "  channel: {}",
            state.channel_address.as_deref().unwrap_or("-")
        );
        println!(
            "  walrusBlobId: {}",
            state.walrus_blob_id.as_deref().unwrap_or("-")
        );
        println!(
            "  dataVersion: {}",
            state.data_version.as_deref().unwrap_or("-")
        );
        println!("  next: drop-cli sale show {}", state.sale_id);
    }
    if count == 0 {
        println!("No local sales found.");
        println!("next: drop-cli phase prepare <file>");
    }
    Ok(())
}

async fn list_purchases(args: &[String]) -> Result<()> {
    let config = load_config()?;
    let purchases = discover_purchases(&config, args).await?;
    let mut count = 0usize;
    for purchase in &purchases {
        count += 1;
        println!("purchaseTx: {}", purchase.purchase_tx_hash);
        println!("  saleId: {}", purchase.sale_id);
        println!(
            "  channel: {}",
            purchase.channel_address.as_deref().unwrap_or("-")
        );
        println!("  status: {}", purchase.status);
        println!(
            "  next: drop-cli purchase show {}",
            purchase.purchase_tx_hash
        );
    }
    if count == 0 {
        println!("No purchases found.");
        println!("Check SUBGRAPH_QUERY_URL, seller key, channel filter, or sale filter.");
    }
    Ok(())
}

async fn discover_purchases(config: &DropCliConfig, args: &[String]) -> Result<Vec<PurchaseView>> {
    let states = load_all_sale_states(state_dir(config)?)?;
    let mut purchases = collect_purchases(&states);
    let channel_filter = flag_value(args, "--channel");
    let sale_filter = flag_value(args, "--sale");
    let status_filter = flag_value(args, "--status");

    match fetch_subgraph_purchases(config, channel_filter, sale_filter).await {
        Ok(subgraph_purchases) => {
            for purchase in subgraph_purchases {
                if purchases.iter().any(|known| {
                    known
                        .purchase_tx_hash
                        .eq_ignore_ascii_case(&purchase.purchase_tx_hash)
                }) {
                    continue;
                }
                purchases.push(purchase);
            }
        }
        Err(error) => {
            if purchases.is_empty() {
                return Err(error.context("failed to discover purchases from subgraph"));
            }
            println!("WARN failed to discover purchases from subgraph: {error}");
        }
    }

    purchases.retain(|purchase| {
        channel_filter
            .map(|channel| {
                purchase
                    .channel_address
                    .as_deref()
                    .map(|value| value.eq_ignore_ascii_case(channel))
                    .unwrap_or(false)
            })
            .unwrap_or(true)
            && sale_filter
                .map(|sale| purchase.sale_id.eq_ignore_ascii_case(sale))
                .unwrap_or(true)
            && status_filter
                .map(|status| purchase.status.eq_ignore_ascii_case(status))
                .unwrap_or(true)
    });
    purchases.sort_by(|left, right| {
        left.purchase_tx_hash
            .to_ascii_lowercase()
            .cmp(&right.purchase_tx_hash.to_ascii_lowercase())
    });
    Ok(purchases)
}

fn list_threads(args: &[String]) -> Result<()> {
    let config = load_config()?;
    let threads = load_all_thread_states(state_dir(&config)?)?;
    let channel_filter = flag_value(args, "--channel");
    let sale_filter = flag_value(args, "--sale");
    let mut count = 0usize;
    for thread in threads.iter().filter(|thread| {
        channel_filter
            .map(|channel| {
                thread
                    .channel_address
                    .as_deref()
                    .map(|value| value.eq_ignore_ascii_case(channel))
                    .unwrap_or(false)
            })
            .unwrap_or(true)
            && sale_filter
                .map(|sale| thread.sale_id.eq_ignore_ascii_case(sale))
                .unwrap_or(true)
    }) {
        count += 1;
        println!("thread: {}", thread.thread_id);
        println!("  saleId: {}", thread.sale_id);
        println!(
            "  channel: {}",
            thread.channel_address.as_deref().unwrap_or("-")
        );
        println!("  status: {:?}", thread.status);
        println!("  purchases: {}", thread.purchases.len());
        println!("  next: drop-cli thread show {}", thread.thread_id);
    }
    if count == 0 {
        println!("No local threads found.");
        println!("next: drop-cli purchase list");
    }
    Ok(())
}

fn thread_for_purchase(
    state_dir: &Path,
    sale_id: &str,
    purchase_tx_hash: &str,
) -> Result<Option<ThreadState>> {
    Ok(load_all_thread_states(state_dir)?.into_iter().find(|thread| {
        thread.sale_id.eq_ignore_ascii_case(sale_id)
            && thread.purchases.iter().any(|purchase| {
                purchase
                    .purchase_tx_hash
                    .eq_ignore_ascii_case(purchase_tx_hash)
            })
    }))
}

fn collect_purchases(states: &[SaleState]) -> Vec<PurchaseView> {
    let mut purchases = Vec::new();
    for state in states {
        let settle_tx_hash = confirmed_tx_hash(state, "settle");
        for context in &state.purchases {
            purchases.push(PurchaseView {
                purchase_tx_hash: context.purchase_tx_hash.clone(),
                sale_id: state.sale_id.clone(),
                channel_address: state.channel_address.clone(),
                buyer: context.buyer.clone(),
                needs_vss: None,
                status: context.status.clone(),
                settle_tx_hash: context
                    .settle_tx_hash
                    .clone()
                    .or_else(|| settle_tx_hash.clone()),
            });
        }
        for tx in state
            .transactions
            .iter()
            .filter(|tx| tx.kind == "purchase" && tx.tx_hash.is_some())
        {
            let tx_hash = tx.tx_hash.clone().unwrap_or_default();
            if purchases
                .iter()
                .any(|purchase| purchase.purchase_tx_hash.eq_ignore_ascii_case(&tx_hash))
            {
                continue;
            }
            let status = if settle_tx_hash.is_some() {
                "settled"
            } else if has_confirmed_tx(state, "fulfill") {
                "fulfilled"
            } else {
                "paid"
            };
            purchases.push(PurchaseView {
                purchase_tx_hash: tx_hash,
                sale_id: state.sale_id.clone(),
                channel_address: state.channel_address.clone(),
                buyer: None,
                needs_vss: None,
                status: status.to_string(),
                settle_tx_hash: settle_tx_hash.clone(),
            });
        }
    }
    purchases.sort_by(|left, right| left.purchase_tx_hash.cmp(&right.purchase_tx_hash));
    purchases
}

#[derive(Debug, Deserialize)]
struct GraphqlResponse<T> {
    data: Option<T>,
    errors: Option<Vec<GraphqlError>>,
}

#[derive(Debug, Deserialize)]
struct GraphqlError {
    message: String,
}

#[derive(Debug, Deserialize)]
struct SellerChannelsData {
    channels: Vec<SubgraphChannel>,
}

#[derive(Debug, Deserialize)]
struct SubgraphChannel {
    channel: String,
}

#[derive(Debug, Deserialize)]
struct PurchasesData {
    purchases: Vec<SubgraphPurchase>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubgraphPurchase {
    channel: String,
    sale_id: String,
    buyer: String,
    tx_hash: String,
}

async fn fetch_subgraph_purchases(
    config: &DropCliConfig,
    channel_filter: Option<&str>,
    sale_filter: Option<&str>,
) -> Result<Vec<PurchaseView>> {
    let endpoint = config
        .subgraph_query_url
        .as_deref()
        .unwrap_or(DEFAULT_SUBGRAPH_QUERY_URL);
    let client = reqwest::Client::new();
    let channels = match channel_filter {
        Some(channel) => vec![normalize_graph_hex(channel, "channel")?],
        None => seller_channels_from_subgraph(config, &client, endpoint).await?,
    };
    if channels.is_empty() {
        return Ok(Vec::new());
    }

    let mut where_parts = vec![format!("channel_in: {}", graphql_string_array(&channels))];
    if let Some(sale) = sale_filter {
        where_parts.push(format!(
            "saleId: \"{}\"",
            normalize_graph_hex(sale, "sale id")?
        ));
    }
    let query = format!(
        r#"{{
          purchases(first: 100, orderBy: timestamp, orderDirection: desc, where: {{ {} }}) {{
            channel
            saleId
            buyer
            txHash
          }}
        }}"#,
        where_parts.join(", ")
    );
    let data: PurchasesData = post_graphql(&client, endpoint, &query).await?;
    Ok(data
        .purchases
        .into_iter()
        .map(|purchase| PurchaseView {
            purchase_tx_hash: purchase.tx_hash,
            sale_id: purchase.sale_id,
            channel_address: Some(purchase.channel),
            buyer: Some(purchase.buyer),
            needs_vss: None,
            status: "paid".to_string(),
            settle_tx_hash: None,
        })
        .collect())
}

async fn seller_channels_from_subgraph(
    config: &DropCliConfig,
    client: &reqwest::Client,
    endpoint: &str,
) -> Result<Vec<String>> {
    let seller = seller_address(config)?;
    let query = format!(
        r#"{{
          channels: exchangeChannels(first: 100, where: {{ owner: "{}" }}) {{
            channel
          }}
        }}"#,
        format!("{seller:?}").to_ascii_lowercase()
    );
    let data: SellerChannelsData = post_graphql(client, endpoint, &query).await?;
    Ok(data
        .channels
        .into_iter()
        .map(|channel| channel.channel)
        .collect())
}

async fn post_graphql<T: for<'de> Deserialize<'de>>(
    client: &reqwest::Client,
    endpoint: &str,
    query: &str,
) -> Result<T> {
    let response = client
        .post(endpoint)
        .json(&json!({ "query": query }))
        .send()
        .await?
        .error_for_status()?;
    let payload: GraphqlResponse<T> = response.json().await?;
    if let Some(errors) = payload.errors {
        let message = errors
            .into_iter()
            .map(|error| error.message)
            .collect::<Vec<_>>()
            .join("; ");
        bail!("subgraph query failed: {message}");
    }
    payload
        .data
        .ok_or_else(|| anyhow!("subgraph returned no data"))
}

fn seller_address(config: &DropCliConfig) -> Result<Address> {
    let key = config
        .seller_private_key
        .as_deref()
        .ok_or_else(|| anyhow!("SELLER_KEY is missing"))?;
    Ok(key
        .parse::<LocalWallet>()?
        .with_chain_id(config.chain_id)
        .address())
}

fn normalize_graph_hex(value: &str, label: &str) -> Result<String> {
    let clean = value.trim();
    let hex = clean.strip_prefix("0x").unwrap_or(clean);
    if hex.is_empty() || hex.len() % 2 != 0 || !hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
        bail!("{label} must be an even-length hex string");
    }
    Ok(format!("0x{}", hex.to_ascii_lowercase()))
}

fn graphql_string_array(values: &[String]) -> String {
    let items = values
        .iter()
        .map(|value| format!("\"{value}\""))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{items}]")
}

async fn enrich_purchase_or_assume_vss(
    config: &DropCliConfig,
    purchase: PurchaseView,
) -> PurchaseView {
    match enrich_purchase_from_chain(config, purchase.clone()).await {
        Ok(purchase) => purchase,
        Err(error) => {
            println!("WARN could not inspect purchase on chain; assuming VSS is required: {error}");
            PurchaseView {
                needs_vss: Some(true),
                ..purchase
            }
        }
    }
}

async fn enrich_purchase_from_chain(
    config: &DropCliConfig,
    mut purchase: PurchaseView,
) -> Result<PurchaseView> {
    let rpc_url = config
        .rpc_url
        .as_deref()
        .ok_or_else(|| anyhow!("ARBITRUM_SEPOLIA_RPC_URL is missing"))?;
    let hub_address = parse_address(
        config
            .hub_address
            .as_deref()
            .ok_or_else(|| anyhow!("HUB_ADDRESS is missing"))?,
    )?;
    let expected_sale_id = parse_hex32_state(&purchase.sale_id)?;
    let provider = Provider::<Http>::try_from(rpc_url)?;
    let tx_hash: H256 = purchase.purchase_tx_hash.parse()?;
    let receipt = provider
        .get_transaction_receipt(tx_hash)
        .await?
        .ok_or_else(|| anyhow!("purchase receipt not found"))?;
    let hub = hub_abi::ExchangeHubContract::new(hub_address, Arc::new(provider.clone()));

    let mut matched_event = None;
    for log in receipt.logs.iter().filter(|log| log.address == hub_address) {
        let Ok(event) = hub.decode_event::<hub_abi::PurchaseEventFilter>(
            "PurchaseEvent",
            log.topics.clone(),
            log.data.clone(),
        ) else {
            continue;
        };
        if event.sale_id != expected_sale_id {
            continue;
        }
        matched_event = Some(event);
        break;
    }
    let event = matched_event.ok_or_else(|| anyhow!("matching PurchaseEvent not found"))?;
    let channel_address = event.channel;
    let buyer = event.buyer;
    let channel = channel_abi::ExchangeChannelContract::new(channel_address, Arc::new(provider));
    let needs_vss = match channel.needs_vss(buyer).call().await {
        Ok(value) => value,
        Err(_) => !channel.is_privy(buyer).call().await?,
    };

    purchase.channel_address = Some(format!("{channel_address:?}"));
    purchase.buyer = Some(format!("{buyer:?}"));
    purchase.needs_vss = Some(needs_vss);
    Ok(purchase)
}

fn print_purchase(purchase: &PurchaseView) {
    println!("purchaseTx: {}", purchase.purchase_tx_hash);
    println!("saleId: {}", purchase.sale_id);
    println!(
        "channel: {}",
        purchase.channel_address.as_deref().unwrap_or("-")
    );
    println!("buyer: {}", purchase.buyer.as_deref().unwrap_or("-"));
    match purchase.needs_vss {
        Some(true) => println!("needsVss: true"),
        Some(false) => println!("needsVss: false"),
        None => println!("needsVss: unknown"),
    }
    println!("status: {}", purchase.status);
    println!(
        "settleTx: {}",
        purchase.settle_tx_hash.as_deref().unwrap_or("-")
    );
}

async fn phase_respond(purchase_txs: &[String]) -> Result<()> {
    if purchase_txs.is_empty() {
        bail!("usage: drop-cli phase respond <purchase-tx>...");
    }
    let config = load_config()?;
    let state_dir = state_dir(&config)?;
    let states = load_all_sale_states(&state_dir)?;
    let purchases = discover_purchases(&config, &[]).await?;
    let mut selected = Vec::new();
    for tx in purchase_txs {
        let purchase = purchases
            .iter()
            .find(|purchase| purchase.purchase_tx_hash.eq_ignore_ascii_case(tx))
            .ok_or_else(|| anyhow!("purchase not found: {tx}"))?;
        selected.push(enrich_purchase_or_assume_vss(&config, purchase.clone()).await);
    }
    let first = selected
        .first()
        .ok_or_else(|| anyhow!("at least one purchase is required"))?;
    if selected
        .iter()
        .any(|purchase| !purchase.sale_id.eq_ignore_ascii_case(&first.sale_id))
    {
        bail!("all purchases in one thread must belong to the same sale");
    }

    let mut vss_batch = Vec::new();
    let mut no_vss_singletons = Vec::new();
    for purchase in selected {
        if purchase.needs_vss == Some(false) {
            no_vss_singletons.push(purchase);
        } else {
            vss_batch.push(purchase);
        }
    }

    let mut threads = Vec::new();
    if !vss_batch.is_empty() {
        threads.push(upsert_thread_for_purchases(&state_dir, &states, vss_batch)?);
    }
    for purchase in no_vss_singletons {
        threads.push(upsert_thread_for_purchases(
            &state_dir,
            &states,
            vec![purchase],
        )?);
    }

    for thread in threads {
        println!("thread: {}", thread.thread_id);
        println!(
            "channel: {}",
            thread.channel_address.as_deref().unwrap_or("-")
        );
        println!("sale: {}", thread.sale_id);
        println!("purchases: {}", thread.purchases.len());
        println!(
            "needsVss: {}",
            thread.purchases.iter().any(|purchase| purchase.needs_vss)
        );
        for action in &thread.next_actions {
            println!("next: {action}");
        }
    }
    Ok(())
}

fn upsert_thread_for_purchases(
    state_dir: &Path,
    states: &[SaleState],
    selected: Vec<PurchaseView>,
) -> Result<ThreadState> {
    let first = selected
        .first()
        .ok_or_else(|| anyhow!("at least one purchase is required"))?;

    let existing = load_all_thread_states(&state_dir)?
        .into_iter()
        .find(|thread| {
            thread.sale_id.eq_ignore_ascii_case(&first.sale_id)
                && thread.purchases.iter().any(|thread_purchase| {
                    selected.iter().any(|purchase| {
                        thread_purchase
                            .purchase_tx_hash
                            .eq_ignore_ascii_case(&purchase.purchase_tx_hash)
                    })
                })
        });

    let mut thread = if let Some(thread) = existing {
        thread
    } else {
        let now = unix_timestamp_string();
        let thread_id = format!("th_{}", now);
        let thread_purchases = selected
            .iter()
            .map(|purchase| ThreadPurchase {
                purchase_tx_hash: purchase.purchase_tx_hash.clone(),
                buyer: purchase.buyer.clone(),
                sale_id: purchase.sale_id.clone(),
                needs_vss: purchase.needs_vss.unwrap_or(true),
                status: purchase.status.clone(),
                settle_tx_hash: purchase.settle_tx_hash.clone(),
            })
            .collect();
        ThreadState::new(
            thread_id,
            first.sale_id.clone(),
            first.channel_address.clone(),
            thread_purchases,
            now,
        )
    };

    for purchase in selected {
        if !thread.purchases.iter().any(|thread_purchase| {
            thread_purchase
                .purchase_tx_hash
                .eq_ignore_ascii_case(&purchase.purchase_tx_hash)
        }) {
            thread.purchases.push(ThreadPurchase {
                purchase_tx_hash: purchase.purchase_tx_hash,
                buyer: purchase.buyer,
                sale_id: purchase.sale_id,
                needs_vss: purchase.needs_vss.unwrap_or(true),
                status: purchase.status,
                settle_tx_hash: purchase.settle_tx_hash,
            });
        }
    }
    if let Some(sale_state) = states
        .iter()
        .find(|state| state.sale_id.eq_ignore_ascii_case(&thread.sale_id))
    {
        thread.fulfill_tx_hash = confirmed_tx_hash(sale_state, "fulfill");
        if let Some(settle_tx_hash) = confirmed_tx_hash(sale_state, "settle") {
            if !thread
                .settle_tx_hashes
                .iter()
                .any(|known| known.eq_ignore_ascii_case(&settle_tx_hash))
            {
                thread.settle_tx_hashes.push(settle_tx_hash);
            }
        }
    }
    if thread.purchases.iter().all(|purchase| {
        purchase.status == "settled" || purchase.settle_tx_hash.as_deref().is_some()
    }) {
        thread.status = ThreadStatus::Completed;
        thread.next_actions = vec![format!("drop-cli thread show {}", thread.thread_id)];
    } else if thread.fulfill_tx_hash.is_some() {
        thread.status = ThreadStatus::Fulfilled;
        thread.next_actions = vec![format!("drop-cli phase settle {}", thread.thread_id)];
    } else {
        thread.next_actions = vec![format!("drop-cli phase fulfill {}", thread.thread_id)];
    }
    thread.updated_at = unix_timestamp_string();
    save_thread_state(&state_dir, &thread)?;
    Ok(thread)
}

async fn phase_fulfill(thread_id: &str) -> Result<()> {
    let config = load_config()?;
    let state_dir = state_dir(&config)?;
    let mut thread = load_thread_state(&state_dir, thread_id)?;
    if thread.purchases.iter().all(|purchase| {
        purchase.status == "settled" || purchase.settle_tx_hash.as_deref().is_some()
    }) {
        thread.status = ThreadStatus::Completed;
        thread.next_actions = vec![format!("drop-cli thread show {}", thread.thread_id)];
        thread.updated_at = unix_timestamp_string();
        save_thread_state(&state_dir, &thread)?;
        print_thread(&thread);
        return Ok(());
    }
    if matches!(
        thread.status,
        ThreadStatus::Fulfilled
            | ThreadStatus::OraclePending
            | ThreadStatus::SettleReady
            | ThreadStatus::Settling
            | ThreadStatus::Completed
    ) {
        print_thread(&thread);
        return Ok(());
    }

    ensure_sale_data_available_for_fulfill(&thread.sale_id).await?;

    thread.status = ThreadStatus::Fulfilling;
    thread.last_error = None;
    thread.next_actions = vec![format!("drop-cli phase settle {}", thread.thread_id)];
    thread.updated_at = unix_timestamp_string();
    save_thread_state(&state_dir, &thread)?;
    fulfill_thread_purchases(&thread).await?;
    let state = load_sale_state(&state_dir, &thread.sale_id)?;
    thread.fulfill_tx_hash = confirmed_tx_hash(&state, "batch_vss_share")
        .or_else(|| confirmed_tx_hash(&state, "fulfill"));
    for purchase in &mut thread.purchases {
        if let Some(context) = state.purchases.iter().find(|context| {
            context
                .purchase_tx_hash
                .eq_ignore_ascii_case(&purchase.purchase_tx_hash)
        }) {
            purchase.status = context.status.clone();
        }
    }
    thread.status = ThreadStatus::Fulfilled;
    thread.updated_at = unix_timestamp_string();
    save_thread_state(&state_dir, &thread)?;
    print_thread(&thread);
    Ok(())
}

async fn phase_settle(thread_id: &str) -> Result<()> {
    let config = load_config()?;
    let state_dir = state_dir(&config)?;
    let mut thread = load_thread_state(&state_dir, thread_id)?;
    if thread.purchases.iter().all(|purchase| {
        purchase.status == "settled" || purchase.settle_tx_hash.as_deref().is_some()
    }) {
        thread.status = ThreadStatus::Completed;
        thread.next_actions = vec![format!("drop-cli thread show {}", thread.thread_id)];
        thread.updated_at = unix_timestamp_string();
        save_thread_state(&state_dir, &thread)?;
        print_thread(&thread);
        return Ok(());
    }

    thread.status = ThreadStatus::Settling;
    thread.last_error = None;
    thread.next_actions = vec![format!("drop-cli phase verify {}", thread.sale_id)];
    thread.updated_at = unix_timestamp_string();
    save_thread_state(&state_dir, &thread)?;
    settle_thread_purchases(&thread).await?;
    let state = load_sale_state(&state_dir, &thread.sale_id)?;
    for purchase in &mut thread.purchases {
        if let Some(context) = state.purchases.iter().find(|context| {
            context
                .purchase_tx_hash
                .eq_ignore_ascii_case(&purchase.purchase_tx_hash)
        }) {
            purchase.status = context.status.clone();
            purchase.settle_tx_hash = context.settle_tx_hash.clone();
            if let Some(settle_tx) = &context.settle_tx_hash {
                if !thread
                    .settle_tx_hashes
                    .iter()
                    .any(|known| known.eq_ignore_ascii_case(settle_tx))
                {
                    thread.settle_tx_hashes.push(settle_tx.clone());
                }
            }
        }
    }
    thread.status = ThreadStatus::Completed;
    thread.updated_at = unix_timestamp_string();
    save_thread_state(&state_dir, &thread)?;
    print_thread(&thread);
    Ok(())
}

async fn submit_vdd_for_sale(sale_id: &str) -> Result<()> {
    let config = load_config()?;
    let state_dir = state_dir(&config)?;
    let mut state = load_sale_state(&state_dir, sale_id)?;
    ensure_walrus_asset_available(&config, &state_dir, &mut state).await?;
    if let Some(input_asset_path) = state.input_asset_path.as_deref() {
        env::set_var("DROP_SCRIPT_INPUT_ASSET", input_asset_path);
    }
    let listing = drop_script_listing_from_state(&state)?;
    let seller_ctx = drop_script_seller_context(&config).await?;
    let walrus = drop_script_walrus_client(&config);

    let vdd_tx = drop_script::stage_1_6_submit_vdd_proof(&walrus, &listing, &seller_ctx).await?;
    if vdd_tx != H256::zero() {
        state.transactions.push(tx_record(
            "submit_vdd_proof",
            Some(format!("{vdd_tx:?}")),
            TxStatus::Confirmed,
            None,
        ));
        drop_script::trigger_centralized_oracle_worker_if_enabled(vdd_tx, listing.walrus_end_epoch)
            .await?;
    }
    state.next_actions = vec![format!("drop-cli purchase list --sale {}", state.sale_id)];
    save_sale_state(&state_dir, &state)?;
    println!("vddTx: {vdd_tx:?}");
    println!("next: drop-cli purchase list --sale {}", state.sale_id);
    Ok(())
}

async fn ensure_sale_data_available_for_fulfill(sale_id: &str) -> Result<()> {
    let config = load_config()?;
    let state_dir = state_dir(&config)?;
    let mut state = load_sale_state(&state_dir, sale_id)?;
    ensure_walrus_asset_available(&config, &state_dir, &mut state).await?;

    let listing = drop_script_listing_from_state(&state)?;
    let seller_ctx = drop_script_seller_context(&config).await?;
    let walrus = drop_script_walrus_client(&config);

    let vdd_tx = drop_script::stage_1_6_submit_vdd_proof(&walrus, &listing, &seller_ctx).await?;
    if vdd_tx != H256::zero() {
        state.transactions.push(tx_record(
            "submit_vdd_proof",
            Some(format!("{vdd_tx:?}")),
            TxStatus::Confirmed,
            None,
        ));
        save_sale_state(&state_dir, &state)?;
    }

    ensure_oracle_signal_for_listing(&mut state, &state_dir, &listing, &seller_ctx, vdd_tx).await?;
    Ok(())
}

async fn ensure_oracle_signal_for_listing(
    state: &mut SaleState,
    state_dir: &Path,
    listing: &drop_script::ListingState,
    seller_ctx: &drop_script::SellerContext,
    source_tx: H256,
) -> Result<()> {
    let channel_contract = channel_abi::ExchangeChannelContract::new(
        listing.channel_address,
        seller_ctx.signer.clone(),
    );
    let c_cipher: Bytes = listing.encrypted_blob_id.to_vec().into();
    let now = U256::from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs());
    let success_until = channel_contract
        .oracle_success_until(c_cipher.clone())
        .call()
        .await?;
    if success_until > now {
        println!("oracleSuccessUntil already active: {success_until}");
        return Ok(());
    }

    let oracle_source_tx = if source_tx != H256::zero() {
        source_tx
    } else {
        println!("triggering oracle for current Walrus blob before fulfill...");
        let receipt = channel_contract
            .trigger_oracle(c_cipher)
            .send()
            .await?
            .await?
            .ok_or_else(|| anyhow!("triggerOracle receipt missing"))?;
        state.transactions.push(tx_record(
            "trigger_oracle",
            Some(format!("{:?}", receipt.transaction_hash)),
            TxStatus::Confirmed,
            None,
        ));
        save_sale_state(state_dir, state)?;
        receipt.transaction_hash
    };

    drop_script::trigger_centralized_oracle_worker_if_enabled(
        oracle_source_tx,
        listing.walrus_end_epoch,
    )
    .await?;
    drop_script::wait_for_oracle_signal(
        listing.channel_address,
        listing.encrypted_blob_id,
        seller_ctx.signer.clone(),
    )
    .await
}

async fn fulfill_thread_purchases(thread: &ThreadState) -> Result<()> {
    let config = load_config()?;
    let state_dir = state_dir(&config)?;
    let mut state = load_sale_state(&state_dir, &thread.sale_id)?;
    let listing = drop_script_listing_from_state(&state)?;
    let seller_ctx = drop_script_seller_context(&config).await?;

    for thread_purchase in &thread.purchases {
        let existing = state
            .purchases
            .iter()
            .find(|context| {
                context
                    .purchase_tx_hash
                    .eq_ignore_ascii_case(&thread_purchase.purchase_tx_hash)
            })
            .cloned();
        upsert_purchase_context(
            &mut state,
            PurchaseContextRecord {
                purchase_tx_hash: thread_purchase.purchase_tx_hash.clone(),
                buyer: thread_purchase
                    .buyer
                    .clone()
                    .or_else(|| existing.as_ref().and_then(|context| context.buyer.clone())),
                secret_sharing_key: existing
                    .as_ref()
                    .and_then(|context| context.secret_sharing_key.clone()),
                status: existing
                    .as_ref()
                    .map(|context| context.status.clone())
                    .unwrap_or_else(|| thread_purchase.status.clone()),
                fulfill_tx_hash: existing
                    .as_ref()
                    .and_then(|context| context.fulfill_tx_hash.clone()),
                settle_tx_hash: existing
                    .as_ref()
                    .and_then(|context| context.settle_tx_hash.clone()),
            },
        );
    }

    let mut batch_shares = Vec::new();
    for thread_purchase in thread
        .purchases
        .iter()
        .filter(|purchase| purchase.needs_vss)
    {
        let buyer = thread_purchase
            .buyer
            .as_deref()
            .or_else(|| {
                state
                    .purchases
                    .iter()
                    .find(|context| {
                        context
                            .purchase_tx_hash
                            .eq_ignore_ascii_case(&thread_purchase.purchase_tx_hash)
                    })
                    .and_then(|context| context.buyer.as_deref())
            })
            .ok_or_else(|| {
                anyhow!(
                    "purchase {} missing buyer; run drop-cli purchase show or phase respond again",
                    thread_purchase.purchase_tx_hash
                )
            })?;
        batch_shares.push(drop_script::BatchVssShare {
            buyer: parse_address(buyer)?,
            purchase_tx_hash: thread_purchase.purchase_tx_hash.parse()?,
            secret_sharing_key: [0u8; 32],
        });
    }

    if !batch_shares.is_empty() {
        let share_tx =
            drop_script::stage_3_share_data_key_batch(&listing, &batch_shares, &seller_ctx).await?;
        let share_tx_string = format!("{share_tx:?}");
        state.transactions.push(tx_record(
            "batch_vss_share",
            Some(share_tx_string.clone()),
            TxStatus::Confirmed,
            None,
        ));
        for share in &batch_shares {
            if let Some(context) = state.purchases.iter_mut().find(|context| {
                context
                    .purchase_tx_hash
                    .eq_ignore_ascii_case(&format!("{:?}", share.purchase_tx_hash))
            }) {
                context.buyer = Some(format!("{:?}", share.buyer));
                context.fulfill_tx_hash = Some(share_tx_string.clone());
                context.status = "fulfilled".to_string();
            }
        }
    }

    for thread_purchase in thread
        .purchases
        .iter()
        .filter(|purchase| !purchase.needs_vss)
    {
        if let Some(context) = state.purchases.iter_mut().find(|context| {
            context
                .purchase_tx_hash
                .eq_ignore_ascii_case(&thread_purchase.purchase_tx_hash)
        }) {
            context.status = "fulfilled".to_string();
        }
    }

    state.next_actions = vec![format!("drop-cli phase settle {}", thread.thread_id)];
    save_sale_state(&state_dir, &state)?;
    Ok(())
}

async fn fulfill_first_purchase_for_sale(sale_id: &str) -> Result<()> {
    ensure_sale_data_available_for_fulfill(sale_id).await?;

    let config = load_config()?;
    let state_dir = state_dir(&config)?;
    let mut state = load_sale_state(&state_dir, sale_id)?;
    let listing = drop_script_listing_from_state(&state)?;
    let seller_ctx = drop_script_seller_context(&config).await?;
    let walrus = drop_script_walrus_client(&config);
    let purchase_context = first_purchase_context(&state)?.clone();
    let purchase = purchase_state_from_context(&purchase_context)?;

    let fulfill_tx =
        drop_script::stage_3_fulfill(&walrus, &listing, &purchase, &seller_ctx).await?;
    state.transactions.push(tx_record(
        "fulfill",
        Some(format!("{fulfill_tx:?}")),
        TxStatus::Confirmed,
        None,
    ));
    if let Some(context) = state.purchases.iter_mut().find(|context| {
        context
            .purchase_tx_hash
            .eq_ignore_ascii_case(&purchase_context.purchase_tx_hash)
    }) {
        context.fulfill_tx_hash = Some(format!("{fulfill_tx:?}"));
        context.status = "fulfilled".to_string();
    }
    state.next_actions = vec![format!("drop-cli settle {} --yes", state.sale_id)];
    save_sale_state(&state_dir, &state)?;
    println!("fulfillTx: {fulfill_tx:?}");
    println!("next: drop-cli settle {} --yes", state.sale_id);
    Ok(())
}

async fn settle_thread_purchases(thread: &ThreadState) -> Result<()> {
    let config = load_config()?;
    let state_dir = state_dir(&config)?;
    let purchase_txs: Vec<String> = thread
        .purchases
        .iter()
        .filter(|purchase| {
            purchase.status != "settled" && purchase.settle_tx_hash.as_deref().is_none()
        })
        .map(|purchase| purchase.purchase_tx_hash.clone())
        .collect();
    for purchase_tx in purchase_txs {
        settle_purchase_for_sale(&thread.sale_id, &purchase_tx).await?;
    }
    let mut state = load_sale_state(&state_dir, &thread.sale_id)?;
    state.next_actions = vec![format!("drop-cli phase verify {}", thread.sale_id)];
    save_sale_state(&state_dir, &state)?;
    Ok(())
}

async fn settle_first_purchase_for_sale(sale_id: &str) -> Result<()> {
    let config = load_config()?;
    let state = load_sale_state(state_dir(&config)?, sale_id)?;
    let purchase_context = first_purchase_context(&state)?.clone();
    settle_purchase_for_sale(sale_id, &purchase_context.purchase_tx_hash).await
}

async fn settle_purchase_for_sale(sale_id: &str, purchase_tx_hash: &str) -> Result<()> {
    let config = load_config()?;
    let state_dir = state_dir(&config)?;
    let mut state = load_sale_state(&state_dir, sale_id)?;
    let listing = drop_script_listing_from_state(&state)?;
    let seller_ctx = drop_script_seller_context(&config).await?;
    let purchase_context = purchase_context_by_tx(&state, purchase_tx_hash)?.clone();
    let purchase_tx: H256 = purchase_context.purchase_tx_hash.parse()?;

    drop_script::wait_for_oracle_signal(
        listing.channel_address,
        listing.encrypted_blob_id,
        seller_ctx.signer.clone(),
    )
    .await?;

    let (buyer_address, exchange_info) = drop_script::get_purchase_info_from_event(
        seller_ctx.signer.provider(),
        purchase_tx,
        listing.channel_address,
        listing.unique_sale_id,
    )
    .await?;
    let settle_tx = drop_script::stage_5_settle(
        &seller_ctx,
        listing.channel_address,
        buyer_address,
        exchange_info,
        listing.onchain_data_version,
        listing.encrypted_blob_id,
    )
    .await?;
    state.transactions.push(tx_record(
        "settle",
        Some(format!("{settle_tx:?}")),
        TxStatus::Confirmed,
        None,
    ));
    if let Some(context) = state.purchases.iter_mut().find(|context| {
        context
            .purchase_tx_hash
            .eq_ignore_ascii_case(&purchase_context.purchase_tx_hash)
    }) {
        context.buyer = Some(format!("{buyer_address:?}"));
        context.settle_tx_hash = Some(format!("{settle_tx:?}"));
        context.status = "settled".to_string();
    }
    state.next_actions = vec![format!("drop-cli phase verify {}", state.sale_id)];
    save_sale_state(&state_dir, &state)?;
    println!("settleTx: {settle_tx:?}");
    println!("next: drop-cli phase verify {}", state.sale_id);
    Ok(())
}

async fn recover_first_purchase_for_sale(sale_id: &str) -> Result<()> {
    let config = load_config()?;
    let state = load_sale_state(state_dir(&config)?, sale_id)?;
    let listing = drop_script_listing_from_state(&state)?;
    let buyer_ctx = drop_script_buyer_context(&config).await?;
    let walrus = drop_script_walrus_client(&config);
    let purchase_context = first_purchase_context(&state)?;
    let secret_sharing_key = parse_optional_hex32(
        purchase_context.secret_sharing_key.as_deref(),
        "purchase secret_sharing_key",
    )?;
    let fulfill_tx_string = purchase_context
        .fulfill_tx_hash
        .clone()
        .or_else(|| confirmed_tx_hash(&state, "fulfill"))
        .ok_or_else(|| anyhow!("state missing fulfill tx hash"))?;
    let fulfill_tx: H256 = fulfill_tx_string.parse()?;
    drop_script::stage_4_recovery(
        &walrus,
        &buyer_ctx,
        listing.channel_address,
        fulfill_tx,
        listing.walrus_blob_id,
        secret_sharing_key,
        listing.original_len,
    )
    .await?;
    println!("recoverTest: complete");
    Ok(())
}

fn first_purchase_context(state: &SaleState) -> Result<&PurchaseContextRecord> {
    state.purchases.first().ok_or_else(|| {
        anyhow!(
            "state has no purchase context; run complete-test-flow or import buyer purchase context before VSS fulfill/settle"
        )
    })
}

fn purchase_context_by_tx<'a>(
    state: &'a SaleState,
    purchase_tx_hash: &str,
) -> Result<&'a PurchaseContextRecord> {
    state
        .purchases
        .iter()
        .find(|context| context.purchase_tx_hash.eq_ignore_ascii_case(purchase_tx_hash))
        .ok_or_else(|| {
            anyhow!(
                "state has no purchase context for {}; import buyer purchase context before VSS fulfill/settle",
                purchase_tx_hash
            )
        })
}

fn purchase_state_from_context(
    context: &PurchaseContextRecord,
) -> Result<drop_script::PurchaseState> {
    let secret_sharing_key =
        parse_optional_hex32(context.secret_sharing_key.as_deref(), "secret_sharing_key")?;
    Ok(drop_script::PurchaseState {
        secret_sharing_key,
        transaction_hash: context.purchase_tx_hash.parse()?,
    })
}

fn parse_optional_hex32(value: Option<&str>, name: &str) -> Result<[u8; 32]> {
    let value = value.ok_or_else(|| anyhow!("{name} is missing"))?;
    parse_hex32_state(value)
}

fn print_thread(thread: &ThreadState) {
    println!("thread: {}", thread.thread_id);
    println!("saleId: {}", thread.sale_id);
    println!(
        "channel: {}",
        thread.channel_address.as_deref().unwrap_or("-")
    );
    println!("status: {:?}", thread.status);
    println!("purchases:");
    for purchase in &thread.purchases {
        println!("  - tx: {}", purchase.purchase_tx_hash);
        println!("    buyer: {}", purchase.buyer.as_deref().unwrap_or("-"));
        println!("    needsVss: {}", purchase.needs_vss);
        println!("    status: {}", purchase.status);
        println!(
            "    settleTx: {}",
            purchase.settle_tx_hash.as_deref().unwrap_or("-")
        );
    }
    println!(
        "vssProof: {}",
        thread.vss_proof_id.as_deref().unwrap_or("-")
    );
    println!(
        "fulfillTx: {}",
        thread.fulfill_tx_hash.as_deref().unwrap_or("-")
    );
    if !thread.oracle_request_ids.is_empty() {
        println!("oracleRequests:");
        for request in &thread.oracle_request_ids {
            println!("  - {request}");
        }
    }
    if !thread.settle_tx_hashes.is_empty() {
        println!("settleTxs:");
        for tx_hash in &thread.settle_tx_hashes {
            println!("  - {tx_hash}");
        }
    }
    if let Some(error) = &thread.last_error {
        println!("lastError: {error}");
    }
    if thread.next_actions.is_empty() {
        println!("next: drop-cli thread show {}", thread.thread_id);
    } else {
        for action in &thread.next_actions {
            println!("next: {action}");
        }
    }
}

async fn complete_test_flow(sale_id: &str) -> Result<()> {
    let config = load_config()?;
    let state_dir = state_dir(&config)?;
    let mut state = load_sale_state(&state_dir, sale_id)?;
    if state.channel_address.is_none()
        || state.walrus_blob_id.is_none()
        || state.data_version.is_none()
        || state.original_asset_id.is_none()
        || state.encrypted_blob_id.is_none()
    {
        bail!("sale state is not ready; run drop-cli phase publish <sale-id> --yes first");
    }

    println!("running prototype complete test flow through drop-script library...");
    println!("This sends buyer purchase, requests sale-bound VSS/VDD proofs, fulfills, triggers oracle, waits, and settles.");

    ensure_walrus_asset_available(&config, &state_dir, &mut state).await?;

    if let Some(input_asset_path) = state.input_asset_path.as_deref() {
        env::set_var("DROP_SCRIPT_INPUT_ASSET", input_asset_path);
    }

    let listing = drop_script_listing_from_state(&state)?;
    let seller_ctx = drop_script_seller_context(&config).await?;
    let buyer_ctx = drop_script_buyer_context(&config).await?;
    let walrus = drop_script_walrus_client(&config);

    let vdd_tx = drop_script::stage_1_6_submit_vdd_proof(&walrus, &listing, &seller_ctx).await?;
    if vdd_tx != H256::zero() {
        state.transactions.push(tx_record(
            "submit_vdd_proof",
            Some(format!("{vdd_tx:?}")),
            TxStatus::Confirmed,
            None,
        ));
        save_sale_state(&state_dir, &state)?;
        drop_script::trigger_centralized_oracle_worker_if_enabled(vdd_tx, listing.walrus_end_epoch)
            .await?;
    }
    drop_script::wait_for_oracle_signal(
        listing.channel_address,
        listing.encrypted_blob_id,
        seller_ctx.signer.clone(),
    )
    .await?;

    let seller_vss_pubkey = drop_script::seller_public_key_bytes(&seller_ctx.owner_sk_bytes)?;
    let purchase = drop_script::stage_2_purchase(
        &buyer_ctx,
        listing.unique_sale_id,
        listing.onchain_data_version,
        listing.channel_address,
        listing.original_asset_id,
        &seller_vss_pubkey,
    )
    .await?;
    let purchase_tx = format!("{:?}", purchase.transaction_hash);
    state.transactions.push(tx_record(
        "purchase",
        Some(purchase_tx.clone()),
        TxStatus::Confirmed,
        None,
    ));
    upsert_purchase_context(
        &mut state,
        PurchaseContextRecord {
            purchase_tx_hash: purchase_tx.clone(),
            buyer: None,
            secret_sharing_key: Some(format!("0x{}", hex::encode(purchase.secret_sharing_key))),
            status: "paid".to_string(),
            fulfill_tx_hash: None,
            settle_tx_hash: None,
        },
    );
    save_sale_state(&state_dir, &state)?;

    let fulfill_tx =
        drop_script::stage_3_fulfill(&walrus, &listing, &purchase, &seller_ctx).await?;
    state.transactions.push(tx_record(
        "fulfill",
        Some(format!("{fulfill_tx:?}")),
        TxStatus::Confirmed,
        None,
    ));
    if let Some(context) = state
        .purchases
        .iter_mut()
        .find(|context| context.purchase_tx_hash.eq_ignore_ascii_case(&purchase_tx))
    {
        context.fulfill_tx_hash = Some(format!("{fulfill_tx:?}"));
        context.status = "fulfilled".to_string();
    }
    save_sale_state(&state_dir, &state)?;

    drop_script::wait_for_oracle_signal(
        listing.channel_address,
        listing.encrypted_blob_id,
        seller_ctx.signer.clone(),
    )
    .await?;

    let provider = seller_ctx.signer.provider();
    let (buyer_address, exchange_info) = drop_script::get_purchase_info_from_event(
        provider,
        purchase.transaction_hash,
        listing.channel_address,
        listing.unique_sale_id,
    )
    .await?;
    let settle_tx = drop_script::stage_5_settle(
        &seller_ctx,
        listing.channel_address,
        buyer_address,
        exchange_info,
        listing.onchain_data_version,
        listing.encrypted_blob_id,
    )
    .await?;
    state.transactions.push(tx_record(
        "settle",
        Some(format!("{settle_tx:?}")),
        TxStatus::Confirmed,
        None,
    ));
    if let Some(context) = state
        .purchases
        .iter_mut()
        .find(|context| context.purchase_tx_hash.eq_ignore_ascii_case(&purchase_tx))
    {
        context.buyer = Some(format!("{buyer_address:?}"));
        context.settle_tx_hash = Some(format!("{settle_tx:?}"));
        context.status = "settled".to_string();
    }
    save_sale_state(&state_dir, &state)?;

    drop_script::stage_4_recovery(
        &walrus,
        &buyer_ctx,
        listing.channel_address,
        fulfill_tx,
        listing.walrus_blob_id.clone(),
        purchase.secret_sharing_key,
        listing.original_len,
    )
    .await?;
    state.next_actions = vec![format!("drop-cli phase verify {}", state.sale_id)];
    save_sale_state(&state_dir, &state)?;
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
    let asset_encryption_key = config.require_asset_encryption_key()?;
    let asset_nonce = derive_rslh_nonce(&asset_encryption_key, b"trustdrop_asset_v1");
    let encrypted_payload = chacha8_encrypt(&payload, &asset_encryption_key, &asset_nonce, 0)?;
    let encrypted_blob_id = compute_rs_id(&encrypted_payload)?;

    let sale_id = sale_id_from_asset_id(&original_asset_id);
    let encrypted_asset_path = encrypted_asset_path(&config, &sale_id)?;
    if let Some(parent) = encrypted_asset_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&encrypted_asset_path, encrypted_payload)?;

    let mut state = SaleState::new(&sale_id);
    state.input_asset_path = Some(file.to_string());
    state.original_len = Some(original_len);
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

async fn drop_script_seller_context(config: &DropCliConfig) -> Result<drop_script::SellerContext> {
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
    Ok(drop_script::SellerContext {
        signer: Arc::new(SignerMiddleware::new(provider, wallet)),
        owner_sk_bytes: config.require_owner_secret_key()?,
        asset_encryption_key: config.require_asset_encryption_key()?,
    })
}

async fn drop_script_buyer_context(config: &DropCliConfig) -> Result<drop_script::BuyerContext> {
    let rpc_url = config
        .rpc_url
        .as_deref()
        .ok_or_else(|| anyhow!("ARBITRUM_SEPOLIA_RPC_URL is missing"))?;
    let key = config
        .buyer_private_key
        .as_deref()
        .ok_or_else(|| anyhow!("BUYER_KEY is missing"))?;
    let provider = Provider::<Http>::try_from(rpc_url)?;
    let wallet = key.parse::<LocalWallet>()?.with_chain_id(config.chain_id);
    Ok(drop_script::BuyerContext {
        signer: Arc::new(SignerMiddleware::new(provider, wallet)),
    })
}

fn drop_script_walrus_client(config: &DropCliConfig) -> WalrusClient {
    let publisher_url = config
        .walrus_publisher_url
        .clone()
        .unwrap_or_else(|| "http://localhost:31415".to_string());
    let aggregator_url = config
        .walrus_aggregator_url
        .clone()
        .unwrap_or_else(|| publisher_url.clone());
    WalrusClient::new(WalrusConfig {
        aggregator_url,
        publisher_url,
        api_key: String::new(),
        blockberry_base: String::new(),
        send_object_to: None,
    })
}

fn drop_script_listing_from_state(state: &SaleState) -> Result<drop_script::ListingState> {
    Ok(drop_script::ListingState {
        unique_sale_id: parse_hex32_state(&state.sale_id)?,
        onchain_data_version: parse_hex32_state(
            state
                .data_version
                .as_deref()
                .ok_or_else(|| anyhow!("state missing data_version"))?,
        )?,
        walrus_blob_id: state
            .walrus_blob_id
            .clone()
            .ok_or_else(|| anyhow!("state missing walrus_blob_id"))?,
        walrus_end_epoch: state.walrus_end_epoch,
        channel_address: parse_address(
            state
                .channel_address
                .as_deref()
                .ok_or_else(|| anyhow!("state missing channel_address"))?,
        )?,
        original_asset_id: parse_hex32_state(
            state
                .original_asset_id
                .as_deref()
                .ok_or_else(|| anyhow!("state missing original_asset_id"))?,
        )?,
        encrypted_blob_id: parse_hex32_state(
            state
                .encrypted_blob_id
                .as_deref()
                .ok_or_else(|| anyhow!("state missing encrypted_blob_id"))?,
        )?,
        original_len: state
            .original_len
            .ok_or_else(|| anyhow!("state missing original_len"))?,
    })
}

fn upsert_purchase_context(state: &mut SaleState, context: PurchaseContextRecord) {
    if let Some(existing) = state.purchases.iter_mut().find(|existing| {
        existing
            .purchase_tx_hash
            .eq_ignore_ascii_case(&context.purchase_tx_hash)
    }) {
        *existing = context;
    } else {
        state.purchases.push(context);
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

fn flag_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|window| window[0] == flag)
        .map(|window| window[1].as_str())
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
        "originalLength: {}",
        state
            .original_len
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string())
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
    if has_confirmed_tx(state, "settle") {
        return format!("next: drop-cli phase verify {}", state.sale_id);
    }
    if has_confirmed_tx(state, "fulfill") {
        return format!("next: drop-cli phase settle {}", state.sale_id);
    }
    if has_confirmed_tx(state, "submit_vdd_proof") {
        return format!("next: drop-cli phase settle {}", state.sale_id);
    }
    if state.input_asset_path.is_none() {
        return "next: drop-cli phase prepare <file>".to_string();
    }
    if state.walrus_blob_id.is_none() {
        return format!("next: drop-cli phase publish {}", state.sale_id);
    }
    if state.encrypted_blob_id.is_none() {
        return format!("next: drop-cli asset upload {}", state.sale_id);
    }
    format!("next: drop-cli phase prove {} --yes", state.sale_id)
}

fn has_confirmed_tx(state: &SaleState, kind: &str) -> bool {
    state
        .transactions
        .iter()
        .any(|tx| tx.kind == kind && tx.status == TxStatus::Confirmed)
}

fn confirmed_tx_hash(state: &SaleState, kind: &str) -> Option<String> {
    state
        .transactions
        .iter()
        .find(|tx| tx.kind == kind && tx.status == TxStatus::Confirmed)
        .and_then(|tx| tx.tx_hash.clone())
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
