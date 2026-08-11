# Container-outside `drop-cli` operation guide

This guide is for users operating TrustDrop from the Mac mini host while the seller stack runs in Docker.

Do not put user assets into the repository just to make Docker paths work. Use the host wrapper:

```sh
cd /Users/niuniu/TrustDrop/TrustDrop
./docker/seller/drop-cli-host.sh <drop-cli args...>
```

For commands that take a file, pass the host path directly. The wrapper mounts the file's parent directory read-only into a one-off seller CLI container and rewrites the file path for `drop-cli`.

Example:

```sh
./docker/seller/drop-cli-host.sh phase prepare ~/Desktop/demo-assets/apollo.mp4
```

Internally this runs `drop-cli phase prepare /host-input/apollo.mp4`; the source file stays outside the repository.

## Prerequisites

Start the seller stack first:

```sh
cd /Users/niuniu/TrustDrop/TrustDrop/docker/seller
./run-seller-daemon.sh
```

Expected containers:

```text
trustdrop-walrus-publisher   healthy
trustdrop-seller-daemon      running
```

Check from the host:

```sh
docker ps --filter name=trustdrop
./docker/seller/drop-cli-host.sh doctor
./docker/seller/drop-cli-host.sh daemon status
```

The seller CLI container uses:

```text
WALRUS_PUBLISHER_URL=http://walrus-publisher:31415
WALRUS_LOCAL_ENDPOINT=http://walrus-publisher:31415
DROP_CLI_ENV=drop-script/.env
```

The Walrus publisher itself is a separate sidecar container with the Walrus binary baked into the image. Host-specific config is mounted read-only:

```text
TRUSTDROP_WALRUS_CONFIG_DIR=~/walrus
~/walrus/client_config.yaml -> /home/justin/walrus/client_config.yaml
```

If your Walrus config folder lives somewhere else, set `TRUSTDROP_WALRUS_CONFIG_DIR` in `docker/seller/seller.env` before running `./docker/seller/run-seller-daemon.sh`.


## Persistent state

All seller CLI containers share the same host state directory:

```text
TRUSTDROP_STATE_DIR=~/.trustdrop
```

Compose mounts it as:

```text
~/.trustdrop -> /root/.trustdrop
```

This is important for host-file commands. `phase prepare ~/Desktop/file.dat` runs in a one-off container, but the resulting sale state is persisted on the host and can be used by the long-running daemon or later commands such as `phase publish`, `status`, and `next`.

## Daemon access to listed files

`phase prepare <host-file>` stores the container path for the source file, normally `/host-input/<filename>`. The seller daemon must be started with a matching host directory mounted at `/host-input`; otherwise it can discover a buyer purchase but fail during fulfillment with `No such file or directory`.

For the Mac mini demo assets, the default is:

```text
TRUSTDROP_HOST_INPUT_DIR=~/TrustDrop/TrustDrop/app/gui/demo-assets
```

If you list files from another host folder, set this in `docker/seller/seller.env` before starting the daemon:

```text
TRUSTDROP_HOST_INPUT_DIR=/absolute/path/to/the/listed/files
```

Only sales whose stored `/host-input/<filename>` exists in that mounted directory can be fulfilled automatically. If assets live in several host folders, either list the current demo batch from one directory or restart the daemon with `TRUSTDROP_HOST_INPUT_DIR` pointing at the directory needed for the batch under test.

## Command mapping

All commands below are run from the host as:

```sh
./docker/seller/drop-cli-host.sh ...
```

### Setup and diagnostics

| Goal | Host command | Notes |
| --- | --- | --- |
| Show CLI help | `./docker/seller/drop-cli-host.sh --help` | Wrapper help. For raw `drop-cli` help use `./docker/seller/drop-cli-host.sh help`. |
| Initialize local state | `./docker/seller/drop-cli-host.sh init` | Writes state inside the mounted repo/state configuration used by `drop-cli`. |
| Initialize DB | `./docker/seller/drop-cli-host.sh db init` | No host file path needed. |
| Migrate DB | `./docker/seller/drop-cli-host.sh db migrate` | No host file path needed. |
| Inspect DB | `./docker/seller/drop-cli-host.sh db inspect` | Read-only inspection. |
| Check keys | `./docker/seller/drop-cli-host.sh keys check` | Does not print private keys. |
| Full doctor check | `./docker/seller/drop-cli-host.sh doctor` | Verifies RPC/contracts/oracle/Walrus-related config. |

### Asset commands

| Goal | Host command | Notes |
| --- | --- | --- |
| Prepare external host file | `./docker/seller/drop-cli-host.sh asset prepare ~/Desktop/file.dat` | The wrapper mounts `~/Desktop` read-only and passes `/host-input/file.dat` to `drop-cli`. |
| Upload prepared sale asset | `./docker/seller/drop-cli-host.sh asset upload <sale-id>` | Uses container-network Walrus publisher. May upload to Walrus. |
| Ensure asset uploaded | `./docker/seller/drop-cli-host.sh asset ensure <sale-id>` | Idempotent upload/check path. |

### Phase commands

| Goal | Host command | Notes |
| --- | --- | --- |
| Prepare external host file | `./docker/seller/drop-cli-host.sh phase prepare ~/Desktop/file.dat` | Preferred user entry for selecting a host file. |
| Publish prepared sale | `./docker/seller/drop-cli-host.sh phase publish <sale-id>` | Uses `walrus-publisher:31415`; may upload and/or write state. |
| Complete test flow | `./docker/seller/drop-cli-host.sh phase complete-test-flow <sale-id> --yes` | High-impact demo helper; check current state before running. |
| Respond to purchase tx | `./docker/seller/drop-cli-host.sh phase respond <purchase-tx>` | Can submit chain/oracle-related actions depending state. |
| Fulfill thread | `./docker/seller/drop-cli-host.sh phase fulfill <thread-id>` | Seller fulfillment step. |
| Settle thread or sale | `./docker/seller/drop-cli-host.sh phase settle <thread-id-or-sale-id>` | Settlement path. |
| Prove sale | `./docker/seller/drop-cli-host.sh phase prove <sale-id> --yes` | May request SP1 Prove Network proof. |
| Verify sale | `./docker/seller/drop-cli-host.sh phase verify <sale-id>` | Verification/check command. |

### Channel and sale commands

| Goal | Host command | Notes |
| --- | --- | --- |
| List channels | `./docker/seller/drop-cli-host.sh channel list` | Read-only. |
| Show channel | `./docker/seller/drop-cli-host.sh channel show <channel>` | Read-only. |
| Create channel | `./docker/seller/drop-cli-host.sh channel create` | May submit a transaction. |
| List sales | `./docker/seller/drop-cli-host.sh sale list` | Read-only list. |
| List sales by channel | `./docker/seller/drop-cli-host.sh sale list --channel <channel>` | Read-only list. |
| Show sale | `./docker/seller/drop-cli-host.sh sale show <sale-id>` | Read-only sale detail. |
| List/publish sale with yes gate | `./docker/seller/drop-cli-host.sh sale list <sale-id> --yes` | Keep the `--yes` gate explicit. |
| Submit key commitment | `./docker/seller/drop-cli-host.sh sale submit-key-commitment <sale-id>` | May submit transaction. |

### Purchase and thread commands

| Goal | Host command | Notes |
| --- | --- | --- |
| List purchases | `./docker/seller/drop-cli-host.sh purchase list` | Read-only. |
| Filter purchases by channel | `./docker/seller/drop-cli-host.sh purchase list --channel <channel>` | Read-only. |
| Filter purchases by sale | `./docker/seller/drop-cli-host.sh purchase list --sale <sale-id>` | Read-only. |
| Filter purchases by status | `./docker/seller/drop-cli-host.sh purchase list --status <status>` | Read-only. |
| Show purchase | `./docker/seller/drop-cli-host.sh purchase show <purchase-tx>` | Read-only. |
| List threads | `./docker/seller/drop-cli-host.sh thread list` | Read-only. |
| Filter threads by channel | `./docker/seller/drop-cli-host.sh thread list --channel <channel>` | Read-only. |
| Filter threads by sale | `./docker/seller/drop-cli-host.sh thread list --sale <sale-id>` | Read-only. |
| Show thread | `./docker/seller/drop-cli-host.sh thread show <thread-id>` | Read-only. |
| Cancel thread | `./docker/seller/drop-cli-host.sh thread cancel <thread-id>` | Local workflow action. |
| Resume thread | `./docker/seller/drop-cli-host.sh thread resume <thread-id>` | Local workflow action. |

### Status, oracle, proof, settlement, transactions

| Goal | Host command | Notes |
| --- | --- | --- |
| Sale status | `./docker/seller/drop-cli-host.sh status <sale-id>` | Read-only state refresh/display. |
| Next action | `./docker/seller/drop-cli-host.sh next <sale-id>` | Read-only workflow guidance. |
| Oracle check by sale | `./docker/seller/drop-cli-host.sh oracle check <sale-id>` | Read-only check. |
| Oracle check by blob id | `./docker/seller/drop-cli-host.sh oracle check --blob-id <blob-id>` | Read-only check. |
| Oracle check by c-cipher | `./docker/seller/drop-cli-host.sh oracle check --c-cipher <0x...>` | Read-only check. |
| VSS proof | `./docker/seller/drop-cli-host.sh proof vss <sale-id> --yes` | May request/submit proof; keep `--yes` explicit. |
| VDD proof | `./docker/seller/drop-cli-host.sh proof vdd <sale-id> --yes` | May request/submit proof; keep `--yes` explicit. |
| Settle sale | `./docker/seller/drop-cli-host.sh settle <sale-id> --yes` | May submit transaction. |
| Transaction status | `./docker/seller/drop-cli-host.sh tx status <tx-hash>` | Read-only. |
| Resume sale transaction flow | `./docker/seller/drop-cli-host.sh tx resume <sale-id>` | Refreshes and resumes from safe checkpoint. |

### Daemon commands

| Goal | Host command | Notes |
| --- | --- | --- |
| Check daemon dependencies | `./docker/seller/drop-cli-host.sh daemon check` | Read-only dependency check. |
| Show daemon status | `./docker/seller/drop-cli-host.sh daemon status` | Shows daemon status/state. |
| Request daemon stop | `./docker/seller/drop-cli-host.sh daemon stop` | Writes stop request for daemon logic. |
| Run one scan manually | `./docker/seller/drop-cli-host.sh daemon run --once` | Usually use the already-running container instead. |

The long-running daemon itself is started by Docker Compose, not by `drop-cli-host.sh`:

```sh
cd /Users/niuniu/TrustDrop/TrustDrop/docker/seller
./run-seller-daemon.sh
```

### Recovery, TUI, and debug commands

| Goal | Host command | Notes |
| --- | --- | --- |
| Recovery test | `./docker/seller/drop-cli-host.sh recover-test <sale-id>` | Demo/recovery helper. |
| TUI | `./docker/seller/drop-cli-host.sh tui` | Requires an interactive terminal. |
| Debug buyer purchase | `./docker/seller/drop-cli-host.sh debug buyer-purchase <sale-id> --yes` | Test-only helper; can send a real buyer transaction. |
| Debug thread resume | `./docker/seller/drop-cli-host.sh debug thread resume <thread-id>` | Test/debug helper. |

## Host file selection rules

Only these `drop-cli` commands currently take a direct file path:

```text
asset prepare <file>
phase prepare <file>
```

For those, pass a normal host path:

```sh
./docker/seller/drop-cli-host.sh phase prepare /Users/niuniu/Desktop/demo-files/a.mp4
./docker/seller/drop-cli-host.sh phase prepare ~/Movies/demo.mp4
./docker/seller/drop-cli-host.sh asset prepare ./local-test-file.bin
```

The file parent directory is mounted read-only as `/host-input`. If two files have the same name in different host directories, run them as separate commands.

## When to use raw Docker commands

Use raw Docker only for container operations, not normal `drop-cli` work:

```sh
# logs
docker logs -f trustdrop-seller-daemon
docker logs -f trustdrop-walrus-publisher

# restart stack
cd /Users/niuniu/TrustDrop/TrustDrop/docker/seller
docker compose --env-file /dev/null restart

# stop stack
cd /Users/niuniu/TrustDrop/TrustDrop/docker/seller
docker compose --env-file /dev/null down
```
