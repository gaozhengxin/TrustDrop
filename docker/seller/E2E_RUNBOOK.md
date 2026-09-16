# Seller E2E runbook (Mac mini)

This runbook covers the seller side of one manual buyer-to-seller TrustDrop transaction. Run every
command on the Mac mini. The listed asset must remain in the directory mounted at `/host-input` so
the daemon can read it after the buyer submits a purchase.

## 1. Select the repository and asset

```sh
export PATH=/Applications/Docker.app/Contents/Resources/bin:$PATH
export TRUSTDROP_REPO=/Users/niuniu/TrustDrop/TrustDrop
export TRUSTDROP_ASSET=/Users/niuniu/TrustDrop/TrustDrop/app/gui/demo-assets/e2e-asset.tar

cd "$TRUSTDROP_REPO"
test -f "$TRUSTDROP_ASSET"
```

Replace only `TRUSTDROP_ASSET`. Keep the file under `app/gui/demo-assets` unless
`TRUSTDROP_HOST_INPUT_DIR` in `docker/seller/seller.env` deliberately points somewhere else.

## 2. Start and check the seller stack

```sh
cd "$TRUSTDROP_REPO"
./docker/seller/check-elf-hashes.sh

cd "$TRUSTDROP_REPO/docker/seller"
./run-seller-daemon.sh

cd "$TRUSTDROP_REPO"
docker ps --filter name=trustdrop --format '{{.Names}} {{.Status}}'
./docker/seller/drop-cli-host.sh daemon check
./docker/seller/drop-cli-host.sh daemon status
./docker/seller/drop-cli-host.sh keys check
./docker/seller/drop-cli-host.sh doctor
```

Do not continue unless `trustdrop-walrus-publisher` is healthy, the daemon is running, and the CLI
checks succeed.

## 3. Prepare and publish the asset

```sh
cd "$TRUSTDROP_REPO"
TRUSTDROP_PREPARE_OUTPUT="$(./docker/seller/drop-cli-host.sh phase prepare "$TRUSTDROP_ASSET")"
printf '%s\n' "$TRUSTDROP_PREPARE_OUTPUT"
export TRUSTDROP_SALE_ID="$(printf '%s\n' "$TRUSTDROP_PREPARE_OUTPUT" | awk '/^saleId:/ { print $2; exit }')"
test -n "$TRUSTDROP_SALE_ID"

./docker/seller/drop-cli-host.sh phase publish "$TRUSTDROP_SALE_ID" --yes
./docker/seller/drop-cli-host.sh sale show "$TRUSTDROP_SALE_ID"
./docker/seller/drop-cli-host.sh status "$TRUSTDROP_SALE_ID"
```

`phase publish` performs the Walrus upload, creates the sale channel if needed, lists the sale, and
submits the data-key commitment. Use the `phasePublishSaleId` printed by the command if it differs
from the local prepared ID:

```sh
export TRUSTDROP_SALE_ID=<phasePublishSaleId>
```

At this point the buyer can refresh FFM, open the listing, and submit the purchase.

## 4. Watch automatic fulfillment

Use one terminal for logs:

```sh
export PATH=/Applications/Docker.app/Contents/Resources/bin:$PATH
docker logs --since 5m -f trustdrop-seller-daemon
```

Use another terminal for state checks:

```sh
cd /Users/niuniu/TrustDrop/TrustDrop
./docker/seller/drop-cli-host.sh purchase list --sale "$TRUSTDROP_SALE_ID"
./docker/seller/drop-cli-host.sh thread list --sale "$TRUSTDROP_SALE_ID"
./docker/seller/drop-cli-host.sh status "$TRUSTDROP_SALE_ID"
```

The daemon discovers the paid purchase and performs VDD proof generation, oracle confirmation,
VSS proof generation, and fulfillment. The buyer can then refresh Records and download the asset.

## 5. Inspect or resume a failed thread

Find the thread ID and inspect it before resuming:

```sh
cd /Users/niuniu/TrustDrop/TrustDrop
./docker/seller/drop-cli-host.sh thread list --sale "$TRUSTDROP_SALE_ID"
./docker/seller/drop-cli-host.sh thread show <thread-id>
./docker/seller/drop-cli-host.sh thread resume <thread-id>
```

Resume only after correcting the reported dependency or configuration error. A `Planned` thread is
not resumable; the running daemon handles it on its next scan.
