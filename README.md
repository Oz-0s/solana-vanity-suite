# Solana Vanity Wallet and Squads Vault Suite

Generates either standard Solana wallets or Squads Protocol v4 multisig vaults with a chosen Base58 prefix, suffix, or both. The optimized Rust grinder runs entirely offline. The separate creation command registers a winning Squads configuration on Devnet or Mainnet.

## Safety model

- Vanity grinding is completely offline and never needs an existing wallet key. Standard-wallet mode generates a new key locally; Squads mode generates only the one-time creation key required by the protocol.
- The result contains a one-time `createKey`, multisig PDA, and vault PDA. Squads requires this ephemeral key to sign creation, so the result file is owner-readable only (`0600`). It does not control the vault after creation, but keep the file private until creation succeeds.
- Only `create` contacts a Solana RPC endpoint and submits an on-chain transaction.
- `create` never prints private-key material.
- The multisig is created with no unilateral config authority. Membership changes therefore follow the multisig approval process.
- Send assets only to the printed **vault** address. Never fund the multisig configuration address.

## Install

```sh
npm install
npm test
```

## Build and test

```sh
npm run rust:test
npm test
```

## Generate a standard wallet

The result contains a standard 64-byte Solana secret key encoded in Base58 (`privateKeyBase58`) for Phantom, Solflare, and compatible wallets, plus the same 64 bytes as a JSON array for Solana CLI tooling.

```sh
npm run rust:grind -- \
  --kind wallet \
  --suffix toads \
  --output results/wallet-result.json
```

Prefix only:

```sh
npm run rust:grind -- --kind wallet --prefix TOAD --output results/wallet-result.json
```

Prefix and suffix:

```sh
npm run rust:grind -- --kind wallet --prefix TOAD --suffix toads --output results/wallet-result.json
```

The output file contains the wallet private key and is created owner-readable only (`0600`). Never upload, commit, or share it.

## 1. Grind a Squads vault address offline

```sh
npm run rust:grind -- \
  --kind squads-vault \
  --suffix toads \
  --output results/vanity-result.json
```

The TypeScript reference grinder is still available with `npm run grind -- --suffix toads`, but the Rust implementation is preferred for performance.

Rust grinder options:

```text
--kind wallet|squads-vault
--prefix TEXT        optional Base58 prefix
--suffix TEXT        optional Base58 suffix
--threads 10          number of worker threads
--ignore-case         case-insensitive prefix/suffix matching
--vault-index 0       Squads vault index (default: 0)
--output result.json  output file (default: results/vanity-result.json)
```

At least one of `--prefix` or `--suffix` is required. Exact matching is probabilistic. Prefix matching and combined prefix/suffix matching require full Base58 encoding for each candidate and are slower than the optimized exact-suffix-only path. Output files are written once and never overwritten.

## 2. Create and verify on Devnet

The creator must be one of the members and needs a small amount of Devnet SOL. Supply a standard 64-byte Solana CLI JSON keypair file. Never commit that file.

```sh
npm run create -- \
  --result results/vanity-result.json \
  --creator-keypair /absolute/path/to/id.json \
  --members YOUR_ADDRESS,BUDDY_ADDRESS \
  --threshold 2 \
  --network devnet \
  --rpc https://api.devnet.solana.com
```

The command re-derives and verifies both addresses before submission, confirms the transaction, and verifies that the new multisig account is owned by the official Squads v4 program.

## Mainnet

Test the complete workflow on Devnet first. Mainnet creation is deliberately guarded:

```sh
npm run create -- \
  --result results/vanity-result.json \
  --creator-keypair /absolute/path/to/id.json \
  --members YOUR_ADDRESS,BUDDY_ADDRESS \
  --threshold 2 \
  --network mainnet \
  --rpc YOUR_MAINNET_RPC_URL \
  --confirm-mainnet CREATE_REAL_MULTISIG
```

This tool uses the official Squads v4 program ID `SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf`.

## Phantom

Version 0.1 uses a local Solana CLI keypair for the creator transaction. Member addresses can be normal Phantom Solana addresses, and members can subsequently manage the multisig through a compatible Squads interface. A browser-based Phantom signing flow can be added without changing the offline vanity result format.
