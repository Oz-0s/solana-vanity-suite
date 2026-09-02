import { readFileSync } from "node:fs";
import { Keypair, PublicKey } from "@solana/web3.js";
import * as multisig from "@sqds/multisig";

export const SQUADS_PROGRAM_ID = multisig.PROGRAM_ID;
export const DEFAULT_VAULT_INDEX = 0;

export type VanityResult = {
  version: 1;
  prefix?: string;
  suffix: string;
  caseInsensitive: boolean;
  programId: string;
  vaultIndex: number;
  createKey: string;
  createKeySecret: number[];
  multisigPda: string;
  vaultPda: string;
  attempts: number;
  elapsedMs: number;
};

export function keypairFromBytes(bytes: unknown, label: string): Keypair {
  if (!Array.isArray(bytes) || bytes.length !== 64 || bytes.some((n) => !Number.isInteger(n) || n < 0 || n > 255)) {
    throw new Error(`${label} must contain exactly 64 byte values`);
  }
  return Keypair.fromSecretKey(Uint8Array.from(bytes as number[]));
}

export function deriveAddresses(createKey: PublicKey, vaultIndex = DEFAULT_VAULT_INDEX) {
  const [multisigPda] = multisig.getMultisigPda({ createKey });
  const [vaultPda] = multisig.getVaultPda({ multisigPda, index: vaultIndex });
  return { multisigPda, vaultPda };
}

export function matchesSuffix(address: string, suffix: string, caseInsensitive: boolean) {
  return caseInsensitive
    ? address.toLowerCase().endsWith(suffix.toLowerCase())
    : address.endsWith(suffix);
}

export function readKeypair(path: string): Keypair {
  const bytes: unknown = JSON.parse(readFileSync(path, "utf8"));
  return keypairFromBytes(bytes, "Creator keypair");
}

export function arg(name: string): string | undefined {
  const prefix = `--${name}=`;
  const inline = process.argv.find((value) => value.startsWith(prefix));
  if (inline) return inline.slice(prefix.length);
  const index = process.argv.indexOf(`--${name}`);
  return index >= 0 ? process.argv[index + 1] : undefined;
}

export function hasFlag(name: string) {
  return process.argv.includes(`--${name}`);
}
