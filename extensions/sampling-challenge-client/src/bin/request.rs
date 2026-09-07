use anyhow::{bail, Context, Result};
use sampling_challenge_client::{
    challenge_key, proof_domain, request_and_read_seed, CLUSTER_SAMPLING_DOMAIN,
    FLOW_GRAPH_SAMPLING_DOMAIN, VIDEO_SAMPLING_DOMAIN,
};
use std::{env, fs};

#[tokio::main]
async fn main() -> Result<()> {
    let usage = "usage: sampling-challenge-request <video|flow|cluster> <sale-contract> <sale-id>";
    let kind = env::args().nth(1).context(usage)?;
    let sale_contract: [u8; 20] = decode_hex(&env::args().nth(2).context(usage)?)?;
    let sale_id: [u8; 32] = decode_hex(&env::args().nth(3).context(usage)?)?;
    let domain = match kind.as_str() {
        "video" => VIDEO_SAMPLING_DOMAIN,
        "flow" => FLOW_GRAPH_SAMPLING_DOMAIN,
        "cluster" => CLUSTER_SAMPLING_DOMAIN,
        _ => bail!(usage),
    };
    let chain_id = env::var("CHAIN_ID")
        .unwrap_or_else(|_| "421614".to_owned())
        .parse::<u64>()
        .context("invalid CHAIN_ID")?;
    let rpc = env::var("SAMPLING_VRF_RPC_URL")
        .or_else(|_| env::var("ARBITRUM_SEPOLIA_RPC_URL"))
        .or_else(|_| env::var("ARBITRUM_SEPOLIA_RPC"))
        .context("missing sampling RPC URL")?;
    let contract = env::var("SAMPLING_VRF_ADDRESS").context("missing SAMPLING_VRF_ADDRESS")?;
    let private_key = private_key()?;
    let key = challenge_key(chain_id, sale_contract, sale_id, proof_domain(domain));
    let receipt = request_and_read_seed(&rpc, &private_key, &contract, chain_id, key).await?;

    println!("kind: {kind}");
    println!("challengeKey: 0x{}", hex::encode(receipt.challenge_key));
    println!("requestId: 0x{}", hex::encode(receipt.request_id));
    println!("seed: 0x{}", hex::encode(receipt.seed));
    println!("requestCount: {}", receipt.request_count);
    println!("blockNumber: {}", receipt.block_number);
    println!("txHash: {:#x}", receipt.transaction_hash);
    Ok(())
}

fn private_key() -> Result<String> {
    if let Ok(value) = env::var("SAMPLING_VRF_PRIVATE_KEY").or_else(|_| env::var("SELLER_KEY")) {
        if !value.is_empty() {
            return Ok(value);
        }
    }
    let path = env::var("SAMPLING_VRF_PRIVATE_KEY_FILE").context("missing seller key source")?;
    fs::read_to_string(path)?
        .lines()
        .find_map(|line| {
            line.strip_prefix("SAMPLING_VRF_PRIVATE_KEY=")
                .or_else(|| line.strip_prefix("SELLER_KEY="))
        })
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .context("seller key missing from key file")
}

fn decode_hex<const N: usize>(value: &str) -> Result<[u8; N]> {
    let bytes = hex::decode(value.strip_prefix("0x").unwrap_or(value)).context("invalid hex")?;
    bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("expected {N} bytes"))
}
