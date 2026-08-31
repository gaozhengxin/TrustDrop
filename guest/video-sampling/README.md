# Authenticated video sampling proof

This extension proves that three deterministic MP4 previews were assembled from
video samples contained in one Walrus blob. It does not prove a property of every
frame in the source video; it proves the origin and selection of the published
preview evidence.

All build and runtime commands below run in the Ubuntu amd64 container. The
seller-facing binary is `video-sampling-client`.

## Binding chain

```text
sale context + external randomness + origin blob ID + sampling spec
                              |
                              v
                    deterministic sample plans
                              |
                              v
Walrus blob ID <- metadata root <- sliver-pair proofs
                                      |
                                      v
                            primary multiproofs
                                      |
                                      v
authenticated source symbols <- MP4 sample byte_offset/byte_size
                                      |
                                      v
preview skeleton + authenticated source sample payloads
                                      |
                                      v
                         complete preview MP4 CID
```

## Guest inputs

The private witness contains:

- `origin`: the origin blob ID, encoding type, unencoded length, shard count,
  metadata root, and only the primary symbols needed by authenticated MP4
  metadata and the selected samples;
- a Merkle multiproof for those symbols and a sliver-pair path for each involved
  primary sliver;
- the authenticated top-level MP4 box directory used to locate the one `moov`;
- three preview templates containing the file length and all non-video-sample
  byte ranges, with the duplicated video sample payloads removed.

The sale context contains `chain_id`, `sale_contract`, `sale_id`, and
`external_randomness`. The host never supplies final sample positions directly.

## Guest verification

1. Recompute the Walrus blob ID from the authenticated metadata root, encoding
   type, and original length.
2. Verify each involved sliver pair against the metadata root.
3. Verify each requested primary symbol with the per-sliver multiproof.
4. Read the authenticated `moov`, build the source video sample index, and reject
   fragmented MP4 input.
5. Derive the sampling seed and the three time-bucket plans inside the guest.
6. Parse each zero-filled preview template and require its supplied segments to
   be exactly the complement of its video sample ranges.
7. Check codec, sample count, sample size, duration, and sync-frame flags against
   the selected source samples.
8. Read every selected source sample from authenticated Walrus symbols, insert
   it into its preview position, and compute the CID of the reconstructed MP4.

The public values are six ABI-encoded `bytes32` fields:

```text
originBlobId
specHash
samplingSeed
previewCidDigest0
previewCidDigest1
previewCidDigest2
```

`previewCidDigest*` is the final 32-byte multihash digest used by the CID, not a
host assertion.

## Container flow

Assume the repository is mounted at `/workspace`, the public-domain source asset
is already in the repository, and the artifact directory is mounted at `/output`.

Build the seller client and guest ELF:

```bash
cargo build --release -p video-sampling-script --bin video-sampling-client
```

After the sale is listed, write `/output/sale-context.json`. The external
randomness is the listing block hash; `randomSource` records the chain and block
number so anyone can retrieve and verify that hash.

```json
{
  "chainId": 421614,
  "saleContract": "0x...20 bytes...",
  "saleId": "0x...32 bytes...",
  "externalRandomness": "0x...listing block hash...",
  "randomSource": "arbitrum-sepolia:blockHash:<listing-block-number>"
}
```

Generate previews and the witness. Placeholder sale values are rejected:

```bash
cargo run --release -p video-sampling-script --bin video-sampling-client -- \
  /workspace/drop-lib/tests/fixtures/how-a-mosquito-operates-1912.mp4 \
  /output/execute \
  /output/sale-context.json
```

Request a Groth16 proof from the Succinct Prover Network without a local
simulation:

```bash
SP1_PRIVATE_KEY_FILE=/run/secrets/seller.env \
cargo run --release -p video-sampling-script --bin video-sampling-client \
  --features network -- \
  /output/execute/video-sampling-witness.bin \
  /output/execute/video-sampling-proof.json
```

The key file is read at runtime and is never copied into the repository or proof
fixture. The JSON fixture contains `originBlobId`, `programVKey`, `publicValues`,
and `proof`.

Verify the returned proof through the official Groth16 gateway using `eth_call`:

```bash
cargo run --release -p video-sampling-script --bin video-sampling-client \
  --features chain-verify -- \
  /output/execute/video-sampling-proof.json
```

Publish the three previews and certificate through Pinata without requesting a
second proof:

```bash
PINATA_CONFIG_FILE=/run/secrets/pinata.env \
cargo run --release -p video-sampling-script --bin video-sampling-client \
  --features publish -- \
  /output/execute/video-sampling-witness.bin \
  /output/execute/video-sampling-proof.json \
  /output/execute/video-sampling-certificate.json
```

The publisher checks each local preview CID against the digest committed by the
guest, checks each Pinata response against the locally computed CID, uploads the
certificate last, and prints one tag:

```text
trustdrop:video-sampling:v1:<certificate-cid>
```

Attach that tag while preserving the sale's current price, data commitment,
metadata, and unrelated tags:

```bash
drop-cli sale attach-video-proof <sale-id> \
  --certificate-cid <certificate-cid> --yes
```

The client ABI-encodes `verifyProof(bytes32,bytes,bytes)` and sends an `eth_call`
to the official Groth16 gateway on Arbitrum Sepolia. `SP1_VERIFIER_GATEWAY` and
`ARBITRUM_SEPOLIA_RPC_URL` can override the documented defaults.

The wrapper contract additionally checks
`keccak256(abi.encode(PublicValues))` against the certificate hash supplied by
the caller. A sale can store the certificate/proof location in its existing
updatable additional field.

## Current scope

- one non-fragmented MP4 with exactly one supported video track;
- H.264 stream-copy previews with audio removed;
- three deterministic time buckets and a five-second presentation target;
- sampled evidence only, not a claim about every frame or the semantic truth of
  the video.
