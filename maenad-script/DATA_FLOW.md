# Maenad Protocol: Detailed Parameter Flow

This document outlines the precise end-to-end data flow of the Maenad protocol, detailing the calculation of every critical parameter.

## Core Concepts

*   **Actors**:
    *   **Seller**: The user selling access to digital data.
    *   **Buyer**: The user purchasing access to the data.
*   **Execution Environments**:
    *   **Local**: The user's own machine. All private keys and raw data originate here.
    *   **On-Chain**: Smart contracts on the blockchain, the public source of truth.
    *   **SP1 ZKVM**: Decentralized network for ZK proof generation.
    *   **Storage Network**: A decentralized storage provider (e.g., Walrus).

---

## Stage 1: Data Preparation & Commitment (Seller)

The Seller prepares the data, encrypts it, and generates all necessary commitments before listing.

| Parameter | Calculation / Derivation | Visibility | On-Chain Contract & Function |
| :--- | :--- | :--- | :--- |
| `raw_data` | The original file content. | **Private** | N/A |
| `asset_encryption_key` | A cryptographically random `[u8; 32]` array. | **Private** | N/A |
| `data_key_commitment` | `Sha256::digest(&asset_encryption_key)` | **Public** | `VSS.sol` -> `submitDataKeyCommitment()` |
| `data_commitment` | `compute_blob_id_default(&raw_data)` (Merkle root) | **Public** | `VDD.sol` -> `listDataInfo()` |
| `aux_data` | A fixed byte string, e.g., `b"maenad_v1"`. | **Public** | N/A (Used off-chain) |
| `encryption_nonce` | `derive_rslh_nonce(&asset_encryption_key, aux_data)` | **Public** | N/A (Used off-chain) |
| `cipher_data` | `ChaCha8::encrypt(raw_data, &asset_encryption_key, &encryption_nonce)` | **Publicly Stored** | Uploaded to Storage Network |
| `c_cipher` | `compute_blob_id_default(&cipher_data)` (Merkle root) | **Public** | Used in VDD proof |

---

## Stage 2: Listing & Purchase

The Seller lists the item, and the Buyer pays to begin the exchange.

| Action | Actor | Data Involved | Calculation / Derivation | Visibility |
| :--- | :--- | :--- | :--- | :--- |
| **1. List for Sale** | Seller | `data_commitment`, `price` | Calls `listFile(data_commitment, price)` | **Public** |
| **2. Buyer Prepares** | Buyer | `vss_key` | A cryptographically random `[u8; 32]` key. | **Private** (Buyer's secret) |
| **3. Buyer Commits** | Buyer | `vss_key_commitment` | `blake3::hash(&vss_key)` | **Public** |
| **4. Buyer Encrypts Key** | Buyer | `encrypted_vss_key` | ECIES encryption of `vss_key` for the Seller. | **Public** |
| **5. Buyer Purchases**| Buyer | `saleId`, `price`, `vss_key_commitment`, `encrypted_vss_key` | Calls `purchase()` with payment. | **Public** |

---

## Stage 3: Fulfillment (ZK Proofs)

The Seller generates and submits proofs to release the key securely.

### VSS Proof (Key Sharing)
| Role | Parameter | Calculation | Visibility |
| :--- | :--- | :--- | :--- |
| **ZKVM Private Input** | `msg` | The `asset_encryption_key`. | **ZKVM Only** |
| **ZKVM Private Input** | `keys` | A list containing the Buyer's `vss_key`. | **ZKVM Only** |
| **ZKVM Public Output** | `h_orig_block` | `blake3::hash(asset_encryption_key)` (Commitment to asset key). | **Public** |
| **ZKVM Public Output** | `ciphertext` | ChaCha8 encryption of `asset_encryption_key` with `vss_key`. This is the `wrapped_asset_key`. | **Public** |
| **ZKVM Public Output** | `h_k` | `blake3::hash(vss_key)`. Matches Buyer's `vss_key_commitment`. | **Public** |
| **On-Chain Check** | `bindingHash` | `keccak256(abi.encode(dataKeyCommitment, ...))` | **Public** (in `shareDataKey`) |

### VDD Proof (Data Availability)
| Role | Parameter | Calculation | Visibility |
| :--- | :--- | :--- | :--- |
| **ZKVM Private Input**| `key` | The `asset_encryption_key`. | **ZKVM Only** |
| **ZKVM Private Input**| `c_origin_bytes`| The `data_commitment` from Stage 1. | **ZKVM Only** |
| **ZKVM Private Input**| `c_cipher_bytes`| The `c_cipher` from Stage 1. | **ZKVM Only** |
| **ZKVM Private Input**| `proofs` | Data shards sampled from `raw_data` and `cipher_data`. | **ZKVM Only** |
| **ZKVM Public Output**| `public_values` | Contains `c_origin_bytes`, `c_key` (`Sha256(key)`), `c_cipher_bytes`. | **Public** |
| **On-Chain Check** | `bindHash` | `keccak256(abi.encode(cOrigin, dataKeyCommitment, cCipher))` | **Public** (in `submitVDDProof`) |

### `fulfill` Transaction
The Seller calls `fulfill` on-chain with the `vss_proof`, `vdd_proof`, and the public outputs (`wrapped_asset_key`, etc.). The contract verifies both proofs against the commitments it has stored.

---

## Stage 4: Settlement & Data Recovery

| Action | Actor | Data Involved | Calculation / Derivation | Visibility |
| :--- | :--- | :--- | :--- | :--- |
| **1. Oracle Verifies** | Oracle | `c_cipher` | Oracle service confirms `c_cipher` is available on the Storage Network. | **Public** |
| **2. Settle Funds** | Seller | - | Calls `settle()`. Contract transfers funds. | **Public** |
| **3. Get Wrapped Key**| Buyer | `wrapped_asset_key` | From `DataKeyShared` event log. | **Public** |
| **4. Unwrap Key** | Buyer | `asset_encryption_key` | Decrypt `wrapped_asset_key` with private `vss_key`. | **Private** |
| **5. Get Data** | Buyer | `raw_data` | Download `cipher_data` and decrypt with `asset_encryption_key`.| **Private** |