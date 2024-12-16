# Stylus sale_contract

  This project contains two contract which implements erc20 token and trading token at fixed price using stylus.

## Quick Start

Install [Rust](https://www.rust-lang.org/tools/install), and then install the Stylus CLI tool with Cargo

```bash
cargo install --force cargo-stylus cargo-stylus-check
```

Add the `wasm32-unknown-unknown` build target to your Rust compiler:

```
rustup target add wasm32-unknown-unknown
```

You should now have it available as a Cargo subcommand:

```bash
cargo stylus --help
```

Then, clone the template:

```
git clone https://github.com/vlasfama/sale_contract.git
```

## Deploying

You can use the `cargo stylus` command to also deploy your program to the Stylus testnet.

```bash
cargo stylus check
```

Next, we can estimate the gas costs to deploy and activate our program before we send our transaction.

```bash
cargo stylus deploy \
  --private-key-path=<PRIVKEY_FILE_PATH> \
  --estimate-gas
```

Here's how to deploy:

```bash
cargo stylus deploy \
  --private-key-path=<PRIVKEY_FILE_PATH>
```

The CLI will send 2 transactions to deploy and activate your program onchain.

## Calling Your Program

Before running, set the following env vars or place them in a `.env` file (see: [.env.example](./.env.example)) in this project:

```
RPC_URL=https://sepolia-rollup.arbitrum.io/rpc
STYLUS_CONTRACT_ADDRESS=<the onchain address of your deployed program>
PRIV_KEY_PATH=<the file path for your priv key to transact with>
```
