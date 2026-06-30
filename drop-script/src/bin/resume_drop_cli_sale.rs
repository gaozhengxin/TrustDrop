use anyhow::{anyhow, Result};
use dotenv::dotenv;
use drop_sdk::state::{load_sale_state, save_sale_state, SaleState, TxRecord, TxStatus};
use ethers::prelude::*;
use std::{
    env,
    path::PathBuf,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use storage::{WalrusClient, WalrusConfig};

#[path = "../main.rs"]
mod drop_script;

pub use drop_script::{
    configured_hub_address, configured_rpc_url, configured_vdd_verifier_address,
    configured_vss_verifier_address, configured_walrus_endpoint,
};

const CHAIN_ID: u64 = 421614;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();

    let sale_id = env::args()
        .nth(1)
        .ok_or_else(|| anyhow!("usage: resume_drop_cli_sale <sale-id>"))?;
    let state_dir = env::var("DROP_CLI_STATE_DIR")
        .map(PathBuf::from)
        .map_err(|_| anyhow!("DROP_CLI_STATE_DIR is required"))?;
    let mut state = load_sale_state(&state_dir, &sale_id)?;

    let provider = Provider::<Http>::try_from(drop_script::configured_rpc_url())?;
    let seller_wallet = env::var("SELLER_KEY")?
        .parse::<LocalWallet>()?
        .with_chain_id(CHAIN_ID);
    let buyer_wallet = env::var("BUYER_KEY")?
        .parse::<LocalWallet>()?
        .with_chain_id(CHAIN_ID);

    let seller_ctx = drop_script::SellerContext {
        signer: Arc::new(SignerMiddleware::new(provider.clone(), seller_wallet)),
        owner_sk_bytes: [0x11; 32],
        asset_encryption_key: [0x22; 32],
    };
    let buyer_ctx = drop_script::BuyerContext {
        signer: Arc::new(SignerMiddleware::new(provider.clone(), buyer_wallet)),
    };
    let walrus_endpoint = drop_script::configured_walrus_endpoint();
    let walrus_client = WalrusClient::new(WalrusConfig {
        aggregator_url: walrus_endpoint.clone(),
        publisher_url: walrus_endpoint,
        api_key: String::new(),
        blockberry_base: String::new(),
        send_object_to: None,
    });

    let listing = listing_from_state(&state)?;
    println!(">>> [DROP-CLI RESUME] saleId: {}", state.sale_id);
    println!(
        ">>> [DROP-CLI RESUME] channel: {:?}",
        listing.channel_address
    );

    let vdd_tx =
        drop_script::stage_1_6_submit_vdd_proof(&walrus_client, &listing, &seller_ctx).await?;
    if vdd_tx != H256::zero() {
        record_confirmed_tx(&mut state, "submit_vdd_proof", vdd_tx);
        save_sale_state(&state_dir, &state)?;
        drop_script::trigger_centralized_oracle_worker_if_enabled(vdd_tx).await?;
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
    record_confirmed_tx(&mut state, "purchase", purchase.transaction_hash);
    save_sale_state(&state_dir, &state)?;

    let fulfill_tx =
        drop_script::stage_3_fulfill(&walrus_client, &listing, &purchase, &seller_ctx).await?;
    println!(">>> fulfill txHash: {fulfill_tx:#x}");
    record_confirmed_tx(&mut state, "fulfill", fulfill_tx);
    save_sale_state(&state_dir, &state)?;

    drop_script::wait_for_oracle_signal(
        listing.channel_address,
        listing.encrypted_blob_id,
        seller_ctx.signer.clone(),
    )
    .await?;

    let (buyer_address, exchange_info) = drop_script::get_purchase_info_from_event(
        provider.provider(),
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
    println!(">>> settle txHash: {settle_tx:#x}");
    record_confirmed_tx(&mut state, "settle", settle_tx);
    save_sale_state(&state_dir, &state)?;

    drop_script::stage_4_recovery(
        &walrus_client,
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

    println!(">>> drop-cli sale flow completed");
    Ok(())
}

fn listing_from_state(state: &SaleState) -> Result<drop_script::ListingState> {
    Ok(drop_script::ListingState {
        unique_sale_id: parse_hex32(&state.sale_id)?,
        onchain_data_version: parse_hex32(
            state
                .data_version
                .as_deref()
                .ok_or_else(|| anyhow!("state missing data_version"))?,
        )?,
        walrus_blob_id: state
            .walrus_blob_id
            .clone()
            .ok_or_else(|| anyhow!("state missing walrus_blob_id"))?,
        channel_address: state
            .channel_address
            .as_deref()
            .ok_or_else(|| anyhow!("state missing channel_address"))?
            .parse::<Address>()?,
        original_asset_id: parse_hex32(
            state
                .original_asset_id
                .as_deref()
                .ok_or_else(|| anyhow!("state missing original_asset_id"))?,
        )?,
        encrypted_blob_id: parse_hex32(
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

fn parse_hex32(value: &str) -> Result<[u8; 32]> {
    let clean = value.strip_prefix("0x").unwrap_or(value);
    let bytes = hex::decode(clean)?;
    bytes
        .try_into()
        .map_err(|_| anyhow!("expected 32-byte hex value"))
}

fn record_confirmed_tx(state: &mut SaleState, kind: &str, tx_hash: H256) {
    let now = unix_timestamp_string();
    state.transactions.push(TxRecord {
        id: format!("{}_{}", kind, now),
        kind: kind.to_string(),
        chain_id: CHAIN_ID,
        tx_hash: Some(format!("{tx_hash:#x}")),
        status: TxStatus::Confirmed,
        required_confirmations: 1,
        block_number: None,
        receipt_status: Some("success".to_string()),
        created_at: now.clone(),
        updated_at: now,
        next_command: None,
    });
}

fn unix_timestamp_string() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}
