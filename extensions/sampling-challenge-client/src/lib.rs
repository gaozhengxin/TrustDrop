use anyhow::{ensure, Context, Result};
use ethers::{
    abi::{encode, Token},
    contract::abigen,
    middleware::SignerMiddleware,
    providers::{Http, Provider},
    signers::{LocalWallet, Signer},
    types::{Address, H256, U256},
    utils::keccak256,
};
use std::{str::FromStr, sync::Arc};

abigen!(
    SamplingChallengeVrfMock,
    r#"[
        function requestSeed(bytes32 challengeKey) external returns (bytes32 requestId, bytes32 seed)
        function latestChallenges(address requester, bytes32 challengeKey) external view returns (address storedRequester, bytes32 seed, bytes32 requestId, uint64 requestCount, uint64 blockNumber)
    ]"#
);

pub const VIDEO_SAMPLING_DOMAIN: &str = "trustdrop.video-sampling.v1";
pub const FLOW_GRAPH_SAMPLING_DOMAIN: &str = "trustdrop.flow-graph-sampling.v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChallengeReceipt {
    pub contract: Address,
    pub challenge_key: [u8; 32],
    pub request_id: [u8; 32],
    pub seed: [u8; 32],
    pub requester: Address,
    pub request_count: u64,
    pub block_number: u64,
    pub transaction_hash: H256,
}

pub fn proof_domain(name: &str) -> [u8; 32] {
    keccak256(name.as_bytes())
}

pub fn challenge_key(
    chain_id: u64,
    sale_contract: [u8; 20],
    sale_id: [u8; 32],
    domain: [u8; 32],
) -> [u8; 32] {
    keccak256(encode(&[
        Token::Uint(U256::from(chain_id)),
        Token::Address(Address::from(sale_contract)),
        Token::FixedBytes(sale_id.to_vec()),
        Token::FixedBytes(domain.to_vec()),
    ]))
}

/// Requests a synchronous mock-VRF challenge and independently reads the
/// latest mapping entry back before returning it to a sampling proof pipeline.
pub async fn request_and_read_seed(
    rpc_url: &str,
    private_key: &str,
    vrf_address: &str,
    chain_id: u64,
    challenge_key: [u8; 32],
) -> Result<ChallengeReceipt> {
    let vrf_address = Address::from_str(vrf_address).context("invalid sampling VRF address")?;
    let provider = Provider::<Http>::try_from(rpc_url).context("invalid sampling VRF RPC URL")?;
    let wallet = LocalWallet::from_str(private_key)
        .context("invalid sampling VRF seller key")?
        .with_chain_id(chain_id);
    let requester = wallet.address();
    let client = Arc::new(SignerMiddleware::new(provider, wallet));
    let contract = SamplingChallengeVrfMock::new(vrf_address, client);
    let request = contract.request_seed(challenge_key);
    let pending = request.send().await.context("send sampling seed request")?;
    let transaction_hash = pending.tx_hash();
    let receipt = pending
        .await
        .context("wait for sampling seed transaction")?
        .context("sampling seed transaction dropped")?;
    ensure!(
        receipt.status == Some(1u64.into()),
        "sampling seed transaction reverted"
    );

    let (stored_requester, seed, request_id, request_count, block_number) = contract
        .latest_challenges(requester, challenge_key)
        .call()
        .await
        .context("read latest sampling challenge")?;
    ensure!(
        stored_requester == requester,
        "latest challenge requester changed"
    );
    ensure!(seed != [0u8; 32], "sampling VRF returned a zero seed");
    ensure!(
        request_id != [0u8; 32],
        "sampling VRF returned a zero request ID"
    );

    Ok(ChallengeReceipt {
        contract: vrf_address,
        challenge_key,
        request_id,
        seed,
        requester,
        request_count,
        block_number,
        transaction_hash,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_is_stable_and_domain_separated() {
        let sale_contract: [u8; 20] = Address::from_low_u64_be(7).into();
        let sale_id = [9u8; 32];
        let video = challenge_key(
            421614,
            sale_contract,
            sale_id,
            proof_domain(VIDEO_SAMPLING_DOMAIN),
        );
        let graph = challenge_key(
            421614,
            sale_contract,
            sale_id,
            proof_domain(FLOW_GRAPH_SAMPLING_DOMAIN),
        );
        assert_eq!(
            video,
            challenge_key(
                421614,
                sale_contract,
                sale_id,
                proof_domain(VIDEO_SAMPLING_DOMAIN)
            )
        );
        assert_ne!(video, graph);
    }
}
