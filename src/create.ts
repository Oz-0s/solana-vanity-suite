import { readFileSync } from "node:fs";
import { Connection, PublicKey } from "@solana/web3.js";
import * as multisig from "@sqds/multisig";
import { arg, deriveAddresses, keypairFromBytes, readKeypair, SQUADS_PROGRAM_ID, type VanityResult } from "./shared.js";

const resultPath = arg("result") ?? "results/vanity-result.json";
const creatorPath = arg("creator-keypair");
const membersValue = arg("members");
const threshold = Number(arg("threshold") ?? "2");
const rpcUrl = arg("rpc") ?? "https://api.devnet.solana.com";
const network = arg("network") ?? "devnet";
const genesisHashes = {
  devnet: "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG",
  mainnet: "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdp",
} as const;

if (!creatorPath || !membersValue) {
  throw new Error("Required: --creator-keypair /path/id.json --members ADDRESS1,ADDRESS2");
}
if (!['devnet', 'mainnet'].includes(network)) throw new Error("network must be devnet or mainnet");
if (network === "mainnet" && arg("confirm-mainnet") !== "CREATE_REAL_MULTISIG") {
  throw new Error("Mainnet requires --confirm-mainnet CREATE_REAL_MULTISIG");
}

const saved = JSON.parse(readFileSync(resultPath, "utf8")) as VanityResult;
if (saved.programId !== SQUADS_PROGRAM_ID.toBase58()) throw new Error("Result uses an unexpected Squads program ID");
const createKey = keypairFromBytes(saved.createKeySecret, "Saved createKey");
if (createKey.publicKey.toBase58() !== saved.createKey) throw new Error("Saved createKey secret does not match its public key");
const expected = deriveAddresses(createKey.publicKey, saved.vaultIndex);
if (expected.multisigPda.toBase58() !== saved.multisigPda || expected.vaultPda.toBase58() !== saved.vaultPda) {
  throw new Error("Vanity result failed local PDA verification");
}
const members = membersValue.split(",").map((value) => new PublicKey(value.trim()));
if (new Set(members.map(String)).size !== members.length) throw new Error("Member addresses must be unique");
if (!Number.isInteger(threshold) || threshold < 1 || threshold > members.length) throw new Error("Invalid threshold");

const creator = readKeypair(creatorPath);
if (!members.some((member) => member.equals(creator.publicKey))) {
  throw new Error("The creator must be included in --members");
}

console.log(`Network: ${network}`);
console.log(`Vault (send funds here): ${saved.vaultPda}`);
console.log(`Multisig config (do not fund): ${saved.multisigPda}`);
console.log(`Rule: ${threshold}-of-${members.length}`);

const connection = new Connection(rpcUrl, "confirmed");
const genesisHash = await connection.getGenesisHash();
if (genesisHash !== genesisHashes[network as keyof typeof genesisHashes]) {
  throw new Error(`RPC endpoint is not the requested ${network} cluster (unexpected genesis hash)`);
}
if (await connection.getAccountInfo(expected.multisigPda, "confirmed")) {
  throw new Error("The derived multisig account already exists; refusing to submit a duplicate creation");
}
const [programConfigPda] = multisig.getProgramConfigPda({});
const programConfig = await multisig.accounts.ProgramConfig.fromAccountAddress(connection, programConfigPda, "confirmed");
const signature = await multisig.rpc.multisigCreateV2({
  connection,
  treasury: programConfig.treasury,
  creator,
  createKey,
  multisigPda: expected.multisigPda,
  configAuthority: null,
  timeLock: 0,
  members: members.map((key) => ({ key, permissions: multisig.types.Permissions.all() })),
  threshold,
  rentCollector: null,
  sendOptions: { skipPreflight: false, preflightCommitment: "confirmed" },
});

await connection.confirmTransaction(signature, "confirmed");
const account = await connection.getAccountInfo(expected.multisigPda, "confirmed");
if (!account?.owner.equals(SQUADS_PROGRAM_ID)) throw new Error("Created account could not be verified as a Squads multisig");

console.log(`Created and verified. Transaction: ${signature}`);
