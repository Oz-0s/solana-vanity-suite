import assert from "node:assert/strict";
import test from "node:test";
import { PublicKey } from "@solana/web3.js";
import * as multisig from "@sqds/multisig";
import { deriveAddresses, matchesSuffix } from "./shared.js";

test("derives the same Squads v4 multisig and vault PDAs as the SDK", () => {
  const createKey = new PublicKey("11111111111111111111111111111111");
  const actual = deriveAddresses(createKey, 0);
  const [multisigPda] = multisig.getMultisigPda({ createKey });
  const [vaultPda] = multisig.getVaultPda({ multisigPda, index: 0 });
  assert.equal(actual.multisigPda.toBase58(), multisigPda.toBase58());
  assert.equal(actual.vaultPda.toBase58(), vaultPda.toBase58());
});

test("suffix matching is exact unless ignore-case is enabled", () => {
  assert.equal(matchesSuffix("abcTOADS", "toads", false), false);
  assert.equal(matchesSuffix("abcTOADS", "toads", true), true);
});
