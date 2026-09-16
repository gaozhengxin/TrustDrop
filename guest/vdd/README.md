# SP1 Project Template

This is a template for creating an end-to-end [SP1](https://github.com/succinctlabs/sp1) project
that can generate a proof of any RISC-V program.

## Requirements

- [Rust](https://rustup.rs/)
- [SP1](https://docs.succinct.xyz/docs/sp1/getting-started/install)

## Running the Project

There are 3 main ways to run this project: execute a program, generate a core proof, and
generate an EVM-compatible proof.

### Build the Program

The program is automatically built through `script/build.rs` when the script is built.

### Execute the Program

To run the program without generating a proof:

```sh
cd script
cargo run --release -- --execute
```

This will execute the program and display the output.

### Generate an SP1 Core Proof

To generate an SP1 [core proof](https://docs.succinct.xyz/docs/sp1/generating-proofs/proof-types#core-default) for your program:

```sh
cd script
cargo run --release -- --prove
```

### Generate an EVM-Compatible Proof

> [!WARNING]
> You will need at least 16GB RAM to generate a Groth16 or PLONK proof. View the [SP1 docs](https://docs.succinct.xyz/docs/sp1/getting-started/hardware-requirements#local-proving) for more information.

Generating a proof that is cheap to verify on the EVM (e.g. Groth16 or PLONK) is more intensive than generating a core proof.

To generate a Groth16 proof:

```sh
cd script
cargo run --release --bin evm -- --system groth16
```

To generate a PLONK proof:

```sh
cargo run --release --bin evm -- --system plonk
```

These commands will also generate fixtures that can be used to test the verification of SP1 proofs
inside Solidity.

### Retrieve the Verification Key

To retrieve your `programVKey` for your on-chain contract, run the following command in `script`:

```sh
cargo run --release --bin vkey
```

## Using the Prover Network

We highly recommend using the [Succinct Prover Network](https://docs.succinct.xyz/docs/network/introduction) for any non-trivial programs or benchmarking purposes. For more information, see the [key setup guide](https://docs.succinct.xyz/docs/network/developers/key-setup) to get started.

To get started, copy the example environment file:

```sh
cp .env.example .env
```

Then, set the `SP1_PROVER` environment variable to `network` and set the `NETWORK_PRIVATE_KEY`
environment variable to your whitelisted private key.

For example, to generate an EVM-compatible proof using the prover network, run the following
command:

```sh
SP1_PROVER=network NETWORK_PRIVATE_KEY=... cargo run --release --bin evm
```

## Observed Prover Cost

The current RSLH/VE guest has been measured on two assets with the same proof path:

| Asset size | Prover work |
| --- | ---: |
| about 31 MB | 648,131,771 PGUs |
| 100,843,520 bytes | 1,318,495,910 PGUs |

These measurements support treating prover work as roughly linear in asset size for operational
capacity planning, with a material fixed cost and implementation-dependent step effects. They do
not establish an asymptotic bound, and costs should not be described as sublinear based on these
two observations.

Succinct quotes the variable price per billion PGUs (bPGU). For the 1.318495910 bPGU run, using
the observed Groth16 base fee of 0.374064 PROVE:

- Promotional price of 0.000000001 PROVE/bPGU: approximately 0.374064001 PROVE total.
- Reference non-promotional price of 0.59 PROVE/bPGU: approximately 1.151976587 PROVE total.

At the 2026-09-08 reference exchange rate of 1 PROVE = 0.195893 USD, these totals are approximately
0.0733 USD and 0.2257 USD respectively. Both the network auction price and the token/USD exchange
rate are time-dependent; recompute them when quoting a current cost.
