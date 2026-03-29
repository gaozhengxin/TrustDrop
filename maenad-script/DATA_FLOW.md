# Maenad Script: Data Flow Documentation

This document details the data flow, meaning, source, and specification of key variables as they are passed between the local Rust script, the on-chain smart contracts, and the ZKVM.

---

## `main` Function & Context Setup

-   **`seller_key`, `buyer_key`, `sp1_private_key`**
    -   **Meaning**: Private keys for the seller, buyer, and SP1 prover identity.
    -   **Source**: Loaded from the `.env` file.
    -   **Specification**: Hex-encoded string.
    -   **Transmission**: Used to create `LocalWallet` signers for on-chain transactions and configure the `ProverClient`.

-   **`seller_ctx`, `buyer_ctx`**
    -   **Meaning**: Context objects holding the signer middleware and any relevant off-chain keys for the seller and buyer.
    -   **Source**: Created in the `main` function. `SignerMiddleware` is initialized with the correct `chain_id` (`421614`).
    -   **Specification**: `SellerContext`, `BuyerContext` structs.
    -   **Transmission**: Passed to stage functions (`stage_1_listing`, etc.).

-   **`owner_sk_bytes`, `asset_encryption_key`**
    -   **Meaning**: `owner_sk_bytes` is the seller's secret key for ECIES decryption. `asset_encryption_key` is the key for encrypting the actual asset data.
    -   **Source**: Hardcoded in `main` function for demonstration.
    -   **Specification**: `[u8; 32]`.
    -   **Transmission**: Held within `SellerContext`.

---

## Stage 1: Listing

-   **`file_payload`**
    -   **Meaning**: The raw byte content of the asset being sold.
    -   **Source**: Read from disk (`fs::read(INPUT_ASSET_NAME)`).
    -   **Specification**: `Vec<u8>`.
    -   **Transmission**: Used to calculate `original_asset_id`.

-   **`original_asset_id`**
    -   **Meaning**: A unique identifier for the raw asset data.
    -   **Source**: Computed by `compute_rs_id(&file_payload)`.
    -   **Specification**: `[u8; 32]`.
    -   **Transmission**:
        -   The raw `[u8; 32]` value is sent to the `listFile` contract function inside a `DataCommitment` struct.
        -   Passed to `stage_2_purchase` to be used in deriving the `secret_sharing_key`.

-   **`encrypted_asset_data` & `encrypted_blob_id`**
    -   **Meaning**: The asset data encrypted with `asset_encryption_key`, and its unique ID.
    -   **Source**: Computed using `chacha8_encrypt` and `compute_rs_id`.
    -   **Specification**: `Vec<u8>` and `[u8; 32]`.
    -   **Transmission**: `encrypted_asset_data` is uploaded to Walrus. `encrypted_blob_id` is passed to `stage_3_fulfill` for the VDD proof.

-   **`channel_addr`**
    -   **Meaning**: The on-chain address of the seller's `ExchangeChannel` contract.
    -   **Source**: Returned from `get_or_create_channel`.
    -   **Specification**: `ethers::types::Address`.
    -   **Transmission**: Used throughout the script to interact with the correct contract.

-   **`sale_nonce`**
    -   **Meaning**: The current nonce from the `ExchangeChannel` contract, used to ensure a unique `saleId`.
    -   **Source**: Read from the contract via `.nonce().call()`.
    -   **Specification**: `U256`.
    -   **Transmission**: Used as an input to `compute_sale_id`.

-   **`unique_sale_id`**
    -   **Meaning**: The unique identifier for this specific sale listing.
    -   **Source**: Computed by `compute_sale_id(channel_addr, chain_id, sale_nonce)`. The calculation `keccak256(abi.encodePacked(address, uint256, uint256))` must match the on-chain `getNextSaleId()` function.
    -   **Specification**: `[u8; 32]`.
    -   **Transmission**: Passed to `stage_2_purchase`.

-   **`onchain_data_version`**
    -   **Meaning**: The official "version" of the data for this sale, as stored and checked on-chain.
    -   **Source**: Computed as `ethers::utils::keccak256(original_asset_id)`.
    -   **Specification**: `H256` (`[u8; 32]`).
    -   **Transmission**: Passed to `stage_2_purchase` to be used as the `dataVersion` parameter in the `purchase` call.

---

## Stage 2: Purchase

-   **`secret_sharing_key`**
    -   **Meaning**: The buyer's temporary secret for this transaction, used to receive the seller's main `dataKey`.
    -   **Source**: Derived via `key_derive` using a fixed key and the `original_asset_id`.
    -   **Specification**: `[u8; 32]`.
    -   **Transmission**:
        -   Hashed to create `arg_vss_commit`.
        -   Encrypted to create `encrypted_vss_key`.
        -   Returned from `stage_2_purchase` to be used in `stage_4_recovery`.

-   **`arg_vss_commit`**
    -   **Meaning**: A public commitment to the buyer's secret key.
    -   **Source**: Computed as `blake3::hash(&secret_sharing_key)`.
    -   **Specification**: `[u8; 32]`.
    -   **Transmission**: Passed to the `purchase` contract function as `vssKeyCommitment`.

-   **`encrypted_vss_key`**
    -   **Meaning**: The buyer's `secret_sharing_key`, encrypted for the seller.
    -   **Source**: Computed via a custom `ecies::encrypt` function.
    -   **Specification**: `[u8; 32]` (per the custom ECIES implementation).
    -   **Transmission**: Passed to the `purchase` contract function as `encryptedVssKey` (`bytes32`).

-   **`eph_pk` (ephemeral public key)**
    -   **Meaning**: The ephemeral public key generated during the ECIES encryption of the `secret_sharing_key`.
    -   **Source**: Returned from `ecies::encrypt`.
    -   **Specification**: `Vec<u8>` (a compressed public key, 33 bytes).
    -   **Transmission**: Passed to the `purchase` contract function as `dataCommitment` (`bytes`). The seller will retrieve this from the `PurchaseEvent` to use in decryption.

-   **`purchase_transaction_hash`**
    -   **Meaning**: The transaction hash of the successful `purchase` call.
    -   **Source**: Returned from the `ethers` call `tx.unwrap().transaction_hash`.
    -   **Specification**: `H256`.
    -   **Transmission**: Passed to `stage_3_fulfill` to retrieve event data.

---

## Stage 3: Fulfill

-   **`exchange_info`**
    -   **Meaning**: A struct containing all the details of the purchase, retrieved from the on-chain event.
    -   **Source**: Decoded from the `PurchaseEvent` log emitted by the `Hub` contract in the `get_purchase_info_from_event` function.
    -   **Specification**: `channel_abi::ExchangeInfo`.
    -   **Transmission**: Used throughout `stage_3` and passed to `stage_5_settle`.

-   **`wrapped_asset_key_vec`**
    -   **Meaning**: The seller's `asset_encryption_key` (which is related to the master `dataKey`) after being encrypted with the buyer's decrypted `secret_sharing_key`.
    -   **Source**: Computed via `chacha8_encrypt`.
    -   **Specification**: `Vec<u8>`.
    -   **Transmission**: The first 32 bytes are passed into the `VSSArgs` struct for the `fulfill` call as `encryptedDataKey`.

-   **`v_proof`, `v_pv` (VSS Proof)**
    -   **Meaning**: The ZK proof and public values proving the correct wrapping of the asset key.
    -   **Source**: Generated by the ZKVM in `generate_vss_proof`. The `groth16` proof system is used.
    -   **Specification**: `Bytes`.
    -   **Transmission**: Passed inside `VSSArgs` to the `fulfill` contract function.

-   **`d_proof`, `d_pv` (VDD Proof)**
    -   **Meaning**: The ZK proof and public values proving the relationship between the original and encrypted assets.
    -   **Source**: Generated by the ZKVM in `generate_vdd_proof`. The `groth16` proof system is used.
    -   **Specification**: `Bytes`.
    -   **Transmission**: Passed inside `VDDArgs` to the `fulfill` contract function.
