## Walrus

### Setup walrus daemon

[Setup and configure walrus]("https://docs.wal.app/docs/usage/setup")
[Start local daemon]("https://docs.wal.app/docs/operator-guide/aggregator")

```shell
walrus daemon --sub-wallets-dir ~/.sui/sui_config --n-clients 1
```

### Upload

```shell
cargo run --bin walrus -- upload --input ./testdata.txt --epoch 6
```

### Download

```shell
cargo run --bin walrus -- download --blob TGBXYr0km3mLvyNHt6bQXbLHslBR5HAwV8mAQ-HFJIo --output ./out
```

### View status

Only available on mainnet

```shell
cargo run --bin walrus -- status --blob tba9dVjvTALBy_fVVBIfuY8PBCiJ5nWg15umBKYk8q4
```

## Filecoin (Lighthouse)

```shell
cargo run --bin filecoin -- upload --input ./testdata.txt
```

### Download

```shell
cargo run --bin filecoin -- download --cid bafkreidn5zqnpzy4kfdv3xilsucbzvube726wly5g2zmi2do2kvlag3fr4 --output ./out
```

### View status

```shell
cargo run --bin filecoin -- status --cid bafkreidn5zqnpzy4kfdv3xilsucbzvube726wly5g2zmi2do2kvlag3fr4
```
