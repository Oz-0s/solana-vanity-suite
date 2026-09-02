# End-to-End Workflow

This document describes the complete process for generating and deploying a Solana vanity wallet or Squads v4 vanity multisig vault. It intentionally uses placeholders and contains no private keys, recovery phrases, local filesystem paths, or generated result values.

## Security rules

1. Never share a recovery phrase or member-wallet private key.
2. Treat any key printed in a chat, terminal log, or screenshot as compromised and development-only.
3. Share only public member addresses.
4. Keep generated JSON files under `results/`; the directory is excluded from Git.
5. Result files containing wallet secrets or the Squads one-time `createKey` are created with owner-only permissions (`0600`).
6. Never fund a Squads multisig configuration address. Fund only the derived vault address.
7. Test the full lifecycle on Devnet before using Mainnet.
8. Require a separate, explicit confirmation immediately before every on-chain mutation.

## 1. Install and verify

```sh
npm install
npm test
npm run rust:test
```

The tests cross-check PDA derivation against the official Squads SDK. The Rust grinder also runs a known-vector Squads PDA self-test whenever it starts.

## 2. Generate a normal vanity wallet

The Rust grinder supports a prefix, suffix, or both:

```sh
npm run rust:grind -- \
  --kind wallet \
  --prefix PREFIX \
  --suffix SUFFIX \
  --output results/wallet-result.json
```

The output includes:

- The public Solana address.
- A Base58-encoded standard 64-byte secret key for Phantom, Solflare, and compatible wallets.
- The same 64 bytes as a JSON array for Solana CLI-compatible tooling.

The private-key output must never be committed or shared.

## 3. Generate a Squads vanity vault

```sh
npm run rust:grind -- \
  --kind squads-vault \
  --suffix SUFFIX \
  --output results/vanity-result.json
```

The grinder works offline. For every candidate, it derives:

1. A Squads v4 multisig PDA from a one-time `createKey`.
2. The vault PDA from that multisig PDA and vault index.
3. The requested prefix/suffix match against the vault address—not the configuration address.

The winning result stores the one-time `createKey` because Squads requires it to co-sign multisig creation. It is not a multisig member and cannot control the vault after creation.

## 4. Choose members and threshold

Collect only public Solana addresses:

```text
MEMBER_A_PUBLIC_ADDRESS
MEMBER_B_PUBLIC_ADDRESS
```

For a 2-of-2 multisig, both members receive propose, vote, and execute permissions, and both approvals are required before a vault transaction becomes executable.

The multisig should be created with no unilateral config authority. Configuration changes then follow the multisig approval threshold.

## 5. Test on Devnet

Fund a one-time creator/payer with valueless Devnet SOL. The creator pays account rent and transaction fees; it does not need to be a multisig member.

Before broadcasting, perform read-only checks:

- Confirm the RPC genesis hash is Devnet.
- Re-derive the saved multisig and vault addresses.
- Confirm both member addresses are valid and unique.
- Confirm the threshold is valid.
- Confirm the multisig account does not already exist.
- Confirm the creator has enough Devnet SOL.

After explicit approval, create the multisig:

```sh
npm run create -- \
  --result results/vanity-result.json \
  --creator-keypair results/devnet-creator.keypair.json \
  --members MEMBER_A_PUBLIC_ADDRESS,MEMBER_B_PUBLIC_ADDRESS \
  --threshold 2 \
  --network devnet \
  --rpc https://api.devnet.solana.com
```

The creator uses the current Squads `multisigCreateV2` instruction and reads the required treasury from the on-chain Squads program configuration.

After creation, independently fetch and verify:

- Account ownership by the official Squads v4 program.
- Threshold.
- Member list and permission masks.
- No unilateral config authority.
- Vault address.

Fund the Devnet vault with a small amount and test the complete propose → approve → execute flow.

## 6. Prepare Mainnet safely

Member wallets should be newly created, secure Phantom/Solflare/hardware-wallet accounts whose secrets have never been exported or disclosed.

Because the command-line creator cannot sign through Phantom, generate a separate one-time payer locally. It is not added as a member. Fund it with only enough SOL for creation plus a small buffer.

Before broadcasting, repeat all read-only checks against Mainnet and display the exact final configuration:

- Member addresses.
- Threshold.
- Multisig configuration address.
- Vanity vault address.
- Config-authority setting.
- Expected cost.

Require explicit user confirmation after displaying these values.

## 7. Create on Mainnet

Only after explicit confirmation:

```sh
npm run create -- \
  --result results/vanity-result.json \
  --creator-keypair results/mainnet-creator.keypair.json \
  --members MEMBER_A_PUBLIC_ADDRESS,MEMBER_B_PUBLIC_ADDRESS \
  --threshold 2 \
  --network mainnet \
  --rpc https://api.mainnet-beta.solana.com \
  --confirm-mainnet CREATE_REAL_MULTISIG
```

The tool verifies the Mainnet genesis hash, refuses duplicate creation, submits the signed transaction, waits for confirmation, and verifies ownership of the resulting multisig account.

Afterward, independently read the on-chain configuration and verify every field again. Do not fund the vault until this verification passes.

## 8. Handle temporary keys and remaining SOL

- Keep the vanity result file until the intended Devnet and Mainnet multisigs have both been created.
- A one-time creator may retain a small SOL remainder after creation.
- Do not delete its key while funds remain. First obtain explicit approval to return the remainder to a designated address.
- After confirming the return transaction and a zero/uneconomic balance, securely remove the one-time creator key.
- Never commit any result or keypair file.

## 9. Operate the multisig

Funds belong at the vault address. Normal operation follows three separate on-chain stages:

1. A member proposes a transaction.
2. Members approve asynchronously until the threshold is reached.
3. An authorized member executes the approved transaction.

Each member signs locally with their own wallet. The vault has no private key; the Squads program signs for its PDA only after the configured approval conditions are satisfied.

Without a fee relayer, members need a small SOL balance for proposal, approval, and execution fees. Transaction-account rent can generally be reclaimed after completion.
