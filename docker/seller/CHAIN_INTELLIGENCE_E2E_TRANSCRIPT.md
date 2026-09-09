# Chain-intelligence E2E recording transcript

This transcript lists and buys one real Arbitrum chain-intelligence dataset. Run seller commands on
the Mac mini. Commands explicitly marked **local Mac** run on the buyer/development Mac.

The prepared asset is an uncompressed 67 MiB tar containing real Arbitrum One USDC/WETH transfer
facts and derived chain-intelligence sections. Its covered source interval is
`2026-08-28 00:00:00Z` through `2026-08-28 01:23:34Z`.

## 0. Fixed paths and contracts

On the Mac mini:

```sh
export PATH=/Applications/Docker.app/Contents/Resources/bin:$PATH
export TRUSTDROP_REPO=/Users/niuniu/TrustDrop/TrustDrop
export DATASET_DIR="$TRUSTDROP_REPO/app/gui/demo-assets/arbitrum-e2e/intelligence"
export TRUSTDROP_ASSET="$TRUSTDROP_REPO/app/gui/demo-assets/arbitrum-usdc-weth-chain-intelligence-2026-08-28.tar"
export PROOF_DIR=/root/.trustdrop/dataset-proofs/arbitrum-e2e
export SAMPLING_VRF_ADDRESS=0xF761821Ecae34E34AD8670F07E2aAf412AD80Faf
export FLOW_VERIFIER=0xf8D06350C5b261e79ccA1A1061A6bd7922a3b09d
export CLUSTER_VERIFIER=0xe5Ba837f440AC5460C4cc02E5f48fe04994E68e6

cd "$TRUSTDROP_REPO"
test -f "$TRUSTDROP_ASSET"
test -f "$DATASET_DIR/manifest.json"
test -f "$DATASET_DIR/entity-flows.ndjson"
test -f "$DATASET_DIR/cluster-subgraphs.ndjson"
ls -lh "$TRUSTDROP_ASSET"
```

Do not continue if any `test` fails.

## 1. Show what is being sold

Display the manifest, section sizes, record counts, and the first and last records. These commands
show that the asset is Arbitrum data before it is listed.

```sh
cd "$DATASET_DIR"
jq . manifest.json
du -h ./*
wc -l ./*.ndjson

head -n 1 entity-flows.ndjson | jq .
tail -n 1 entity-flows.ndjson | jq .

head -n 1 cluster-subgraphs.ndjson | jq '{wallet_cluster_id, member_count: (.member_addresses | length), node_count: (.nodes | length), edge_count: (.edges | length)}'
tail -n 1 cluster-subgraphs.ndjson | jq '{wallet_cluster_id, member_count: (.member_addresses | length), node_count: (.nodes | length), edge_count: (.edges | length)}'

tar -tf "$TRUSTDROP_ASSET"
```

Expected headline values are 81,112 source transfers, 16,857 addresses, 54 behavioral wallet
clusters, 53,194 fund relationships, and 54 cluster subgraphs. The tar must remain below 100 MB and
above TrustDrop's 1 MiB VDD minimum.

## 2. Manually inspect the complete dataset in the viewer

Run this on the **local Mac**:

```sh
cd /Users/xiexie/playDM/trustdrop-flow-graph
npm run dev -- --host 127.0.0.1
```

Open `http://127.0.0.1:5173/`, choose local-file import, and select all six files in:

```text
/Users/xiexie/playDM/trustdrop-flow-graph/data/arbitrum-e2e/intelligence/
```

The viewer should identify `Arbitrum One (chain ID 42161)`. Show the flow time ruler, then switch to
Clusters and use the cluster selector. Stop here if the viewer reports a schema/import error.

## 3. Start and verify the seller stack

```sh
cd "$TRUSTDROP_REPO"
./docker/seller/check-elf-hashes.sh

cd "$TRUSTDROP_REPO/docker/seller"
./run-seller-daemon.sh
```

Leave that terminal attached. In a second Mac mini terminal:

```sh
export PATH=/Applications/Docker.app/Contents/Resources/bin:$PATH
export TRUSTDROP_REPO=/Users/niuniu/TrustDrop/TrustDrop
cd "$TRUSTDROP_REPO"

docker ps --filter name=trustdrop --format '{{.Names}} {{.Status}}'
./docker/seller/drop-cli-host.sh daemon check
./docker/seller/drop-cli-host.sh daemon status
./docker/seller/drop-cli-host.sh keys check
./docker/seller/drop-cli-host.sh doctor
```

Normal means:

- `trustdrop-walrus-publisher` says `healthy` and `trustdrop-seller-daemon` says `Up`;
- `daemon check` prints `daemonCheck: ok`;
- `daemon status` prints `running: true`, a live PID, and a recent `status: ... running` line;
- key checks and `doctor` complete without a missing-key, missing-RPC, or ELF-hash error.

Warnings caused by a temporary Graph/RPC retry are not by themselves a daemon failure. Do not
restart a healthy daemon merely because the repository contains unrelated untracked files.

## 4. Prepare, upload to Walrus, and list

```sh
export TRUSTDROP_ASSET=/Users/niuniu/TrustDrop/TrustDrop/app/gui/demo-assets/arbitrum-usdc-weth-chain-intelligence-2026-08-28.tar
cd /Users/niuniu/TrustDrop/TrustDrop

TRUSTDROP_PREPARE_OUTPUT="$(./docker/seller/drop-cli-host.sh phase prepare "$TRUSTDROP_ASSET")"
printf '%s\n' "$TRUSTDROP_PREPARE_OUTPUT"
export TRUSTDROP_SALE_ID="$(printf '%s\n' "$TRUSTDROP_PREPARE_OUTPUT" | awk '/^saleId:/ { print $2; exit }')"
test -n "$TRUSTDROP_SALE_ID"

./docker/seller/drop-cli-host.sh phase publish "$TRUSTDROP_SALE_ID" --yes
TRUSTDROP_SALE_OUTPUT="$(./docker/seller/drop-cli-host.sh sale show "$TRUSTDROP_SALE_ID")"
printf '%s\n' "$TRUSTDROP_SALE_OUTPUT"
export CHANNEL_ADDRESS="$(printf '%s\n' "$TRUSTDROP_SALE_OUTPUT" | awk '/^channel:/ { print $2; exit }')"
test -n "$CHANNEL_ADDRESS"
./docker/seller/drop-cli-host.sh status "$TRUSTDROP_SALE_ID"
```

If `phase publish` prints a different `phasePublishSaleId`, copy that value into
`TRUSTDROP_SALE_ID`. Successful output includes a Walrus blob ID and confirmed list/key-commitment
transactions. Now refresh [Fair File Marketplace](https://trustdrop.pages.dev), show the new listing,
but do not buy it yet.

## 5. Request two challenges and build the authenticated samples

Enter the running Ubuntu seller container so compilation and proof preparation happen in the same
Linux environment as the daemon:

```sh
docker exec -it \
  -e TRUSTDROP_SALE_ID="$TRUSTDROP_SALE_ID" \
  -e CHANNEL_ADDRESS="$CHANNEL_ADDRESS" \
  trustdrop-seller-daemon bash
```

Inside the container:

```sh
cd /home/justin/TrustDrop/TrustDrop
set -a
source drop-script/.env
source /root/.trustdrop/secrets/pinata-write.env
set +a

export SAMPLING_VRF_ADDRESS=0xF761821Ecae34E34AD8670F07E2aAf412AD80Faf
export PROOF_DIR=/root/.trustdrop/dataset-proofs/arbitrum-e2e
export DATASET_DIR=/host-input/arbitrum-e2e/intelligence
mkdir -p "$PROOF_DIR/flow" "$PROOF_DIR/clusters"

cargo run --release -p sampling-challenge-client --bin request -- \
  flow "$CHANNEL_ADDRESS" "$TRUSTDROP_SALE_ID" | tee "$PROOF_DIR/flow/challenge.txt"
export FLOW_SEED="$(awk '/^seed:/ { print $2; exit }' "$PROOF_DIR/flow/challenge.txt")"
test -n "$FLOW_SEED"

cargo run --release -p sampling-challenge-client --bin request -- \
  cluster "$CHANNEL_ADDRESS" "$TRUSTDROP_SALE_ID" | tee "$PROOF_DIR/clusters/challenge.txt"
export CLUSTER_SEED="$(awk '/^seed:/ { print $2; exit }' "$PROOF_DIR/clusters/challenge.txt")"
test -n "$CLUSTER_SEED"

cargo run --release -p flow-graph-sampling-script --bin flow-graph-sampling-client -- \
  "$DATASET_DIR/entity-flows.ndjson" "$PROOF_DIR/flow" "$FLOW_SEED"

cargo run --release -p cluster-sampling-script --bin cluster-sampling-client -- \
  "$DATASET_DIR/cluster-subgraphs.ndjson" "$PROOF_DIR/clusters" "$CLUSTER_SEED"

wc -l "$PROOF_DIR/flow/flow-sample.ndjson"
jq 'length' "$PROOF_DIR/clusters/cluster-sample.json"
head -n 1 "$PROOF_DIR/flow/flow-sample.ndjson" | jq .
jq '.[].wallet_cluster_id' "$PROOF_DIR/clusters/cluster-sample.json"
```

Before running this section, set `CHANNEL_ADDRESS` in the shell from `sale show` output. The flow
sample is the complete seed-selected one-minute bucket; the cluster sample is always a JSON array of
three complete seed-selected cluster subgraphs. Neither path takes a first-N truncation.

A deterministic pre-recording dry run against this exact dataset selected 560 flow rows (592 KiB)
and three cluster subgraphs (84 KiB). The real VRF seed will select different records, so these are
capacity checks rather than promised sample counts.

## 6. Submit both proofs to the Succinct proving network

Still inside the Ubuntu container:

```sh
cargo run --release -p flow-graph-sampling-script --features network \
  --bin flow-graph-sampling-client -- \
  "$PROOF_DIR/flow/flow-graph-sampling-witness.bin" "$PROOF_DIR/flow/proof.json"

cargo run --release -p cluster-sampling-script --features network \
  --bin cluster-sampling-client -- \
  "$PROOF_DIR/clusters/cluster-sampling-witness.bin" "$PROOF_DIR/clusters/proof.json"

jq '{originBlobId, samplingSeed, bucketStart, bucketEnd, sampleCidDigest, programVKey}' \
  "$PROOF_DIR/flow/proof.json"
jq '{originBlobId, samplingSeed, clusterIds, sampleCidDigest, programVKey}' \
  "$PROOF_DIR/clusters/proof.json"
```

These commands wait for real Groth16 proofs. Continue only after both `proof.json` files exist.

## 7. Upload samples, build and upload the certificate

Still inside the Ubuntu container:

```sh
export FLOW_SAMPLE_CID="$(curl -fsS https://api.pinata.cloud/pinning/pinFileToIPFS \
  -H "Authorization: Bearer $PINATA_JWT" \
  -F 'pinataOptions={"cidVersion":1}' \
  -F "file=@$PROOF_DIR/flow/flow-sample.ndjson;type=application/x-ndjson" | jq -r .IpfsHash)"
export CLUSTER_SAMPLE_CID="$(curl -fsS https://api.pinata.cloud/pinning/pinFileToIPFS \
  -H "Authorization: Bearer $PINATA_JWT" \
  -F 'pinataOptions={"cidVersion":1}' \
  -F "file=@$PROOF_DIR/clusters/cluster-sample.json;type=application/json" | jq -r .IpfsHash)"
case "$FLOW_SAMPLE_CID" in b*) ;; *) echo "invalid flow sample CID: $FLOW_SAMPLE_CID" >&2; exit 1 ;; esac
case "$CLUSTER_SAMPLE_CID" in b*) ;; *) echo "invalid cluster sample CID: $CLUSTER_SAMPLE_CID" >&2; exit 1 ;; esac
printf 'flow sample CID: %s\ncluster sample CID: %s\n' "$FLOW_SAMPLE_CID" "$CLUSTER_SAMPLE_CID"

jq -n \
  --arg saleId "$TRUSTDROP_SALE_ID" \
  --arg contract "$CHANNEL_ADDRESS" \
  --arg flowCid "$FLOW_SAMPLE_CID" \
  --arg clusterCid "$CLUSTER_SAMPLE_CID" \
  --slurpfile manifest "$DATASET_DIR/manifest.json" \
  --slurpfile flow "$PROOF_DIR/flow/proof.json" \
  --slurpfile clusters "$PROOF_DIR/clusters/proof.json" \
  '{
    type: "trustdrop.chain-intelligence-v2-sampling", version: 1,
    sale: {chainId: 421614, contract: $contract, saleId: $saleId},
    dataset: {
      schema: $manifest[0].schema,
      chain: "arbitrum", chainId: 42161,
      generatedAt: $manifest[0].generated_at,
      firstTimestamp: $manifest[0].source.first_timestamp,
      lastTimestamp: $manifest[0].source.last_timestamp,
      transferCount: $manifest[0].source.transfers,
      clusteringMethod: $manifest[0].model.clustering_method
    },
    samples: {
      flow: ($flow[0] + {
        cid: $flowCid,
        verifier: {chainId: 421614, address: "0xf8D06350C5b261e79ccA1A1061A6bd7922a3b09d", version: "v1"}
      }),
      clusters: ($clusters[0] + {
        cid: $clusterCid,
        verifier: {chainId: 421614, address: "0xe5Ba837f440AC5460C4cc02E5f48fe04994E68e6", version: "v1"}
      })
    }
  }' > "$PROOF_DIR/certificate.json"

jq . "$PROOF_DIR/certificate.json"
export CERTIFICATE_CID="$(curl -fsS https://api.pinata.cloud/pinning/pinFileToIPFS \
  -H "Authorization: Bearer $PINATA_JWT" \
  -F 'pinataOptions={"cidVersion":1}' \
  -F "file=@$PROOF_DIR/certificate.json;type=application/json" | jq -r .IpfsHash)"
case "$CERTIFICATE_CID" in b*) ;; *) echo "invalid certificate CID: $CERTIFICATE_CID" >&2; exit 1 ;; esac
printf 'certificate CID: %s\n' "$CERTIFICATE_CID"
printf '%s\n' "$CERTIFICATE_CID" > "$PROOF_DIR/certificate.cid"
exit
```

Each network client decodes its returned public values and checks the origin blob ID and sampling
seed before writing `proof.json`. The product's actual on-chain verifier calls happen in FFM after
the certificate is attached in the next section.

## 8. Attach the certificate and inspect it in FFM

Back in the Mac mini host shell:

```sh
cd /Users/niuniu/TrustDrop/TrustDrop
export CERTIFICATE_CID="$(docker exec trustdrop-seller-daemon cat /root/.trustdrop/dataset-proofs/arbitrum-e2e/certificate.cid)"
test -n "$CERTIFICATE_CID"
./docker/seller/drop-cli-host.sh sale attach-dataset-proof "$TRUSTDROP_SALE_ID" \
  --certificate-cid "$CERTIFICATE_CID" --yes
```

Refresh FFM after The Graph indexes the metadata update. Open the certificate page. It must show two
successful verifier checks. Expand each verifier call, copy its displayed `curl` command, and run it
once to show the same successful `eth_call` result outside the page. The page also provides two
viewer links:

- Flow graph sample: opens the viewer with the one-minute flow CID already imported.
- Cluster sample: opens the viewer with the three-cluster array already imported and a working
  cluster selector.

## 9. Check readiness, then buy from the buyer account

Before the buyer transaction, run on the Mac mini:

```sh
cd /Users/niuniu/TrustDrop/TrustDrop
./docker/seller/drop-cli-host.sh daemon check
./docker/seller/drop-cli-host.sh daemon status
./docker/seller/drop-cli-host.sh purchase list --sale "$TRUSTDROP_SALE_ID"
```

`daemonCheck: ok` plus `running: true` means the seller is ready. An empty purchase list is expected
before purchase.

On the buyer Mac:

1. Open FFM and connect the buyer wallet.
2. Open the Arbitrum chain-intelligence listing and its sampling certificate.
3. Open both preloaded sample viewers and show the verification results.
4. Purchase the asset.
5. Return to the Mac mini log terminal and watch the daemon discover the paid purchase, run VDD,
   obtain oracle confirmation, run VSS, and submit fulfillment.
6. In FFM Records, press Refresh until the record is fulfilled, then download and decrypt the tar.
7. Extract it and compare the downloaded manifest/head/tail with section 1.

During fulfillment, inspect from a second seller terminal with:

```sh
./docker/seller/drop-cli-host.sh purchase list --sale "$TRUSTDROP_SALE_ID"
./docker/seller/drop-cli-host.sh thread list --sale "$TRUSTDROP_SALE_ID"
./docker/seller/drop-cli-host.sh status "$TRUSTDROP_SALE_ID"
```

Do not manually resume a `Planned` or actively running thread. Resume only a failed thread after its
reported dependency or configuration error has been corrected.
