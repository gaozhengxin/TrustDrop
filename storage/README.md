### Setup walrus daemon
[Setup and configure walrus]("https://docs.wal.app/docs/usage/setup")
[Start local daemon]("https://docs.wal.app/docs/operator-guide/aggregator")
```shell
walrus daemon --sub-wallets-dir ~/.sui/sui_config --n-clients 1
```

### Upload
```shell
cargo run -- upload --input ./testdata.txt --epoch 6
```

### Download
```shell
cargo run -- download --blob TGBXYr0km3mLvyNHt6bQXbLHslBR5HAwV8mAQ-HFJIo --output ./out
```

### View status
Only available on mainnet
```shell
cargo run status --blob tba9dVjvTALBy_fVVBIfuY8PBCiJ5nWg15umBKYk8q4
```