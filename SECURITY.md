# Security notes

- Never share member private keys or recovery phrases. This tool only needs their public addresses.
- The creator keypair stays local but signs and pays for the one-time creation transaction.
- Files under `results/` may contain wallet secrets or the ephemeral Squads `createKey` signer. They are ignored by Git and created with mode `0600`. Do not commit or share them.
- Verify that the displayed vault address has the requested suffix before funding it. Do not send assets to the multisig configuration address.
- Mainnet is guarded by an explicit confirmation phrase and the tool verifies the RPC cluster's genesis hash before submission.

## Dependency audit

The current official `@sqds/multisig@2.1.4` dependency tree reports npm advisories through its Solana v1 SDK dependencies (`bigint-buffer` and `uuid`). npm currently offers no non-breaking automatic resolution for this SDK combination. The tool pins its full dependency tree in `package-lock.json`, accepts only locally supplied addresses/configuration, and should first be exercised on Devnet. Re-run `npm audit` and review upstream Squads releases before any Mainnet use.
