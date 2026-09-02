import { availableParallelism } from "node:os";
import { resolve } from "node:path";
import { writeFileSync } from "node:fs";
import { Worker } from "node:worker_threads";
import { arg, hasFlag, SQUADS_PROGRAM_ID, type VanityResult } from "./shared.js";

const suffix = arg("suffix") ?? "toads";
const caseInsensitive = hasFlag("ignore-case");
const vaultIndex = Number(arg("vault-index") ?? "0");
const threads = Number(arg("threads") ?? availableParallelism());
const output = resolve(arg("output") ?? "results/vanity-result.json");
const base58 = /^[1-9A-HJ-NP-Za-km-z]+$/;

if (!suffix || !base58.test(suffix)) throw new Error("Suffix must contain only Base58 characters");
if (!Number.isInteger(vaultIndex) || vaultIndex < 0 || vaultIndex > 255) throw new Error("vault-index must be 0-255");
if (!Number.isInteger(threads) || threads < 1 || threads > 256) throw new Error("threads must be 1-256");

console.log(`Searching for a Squads v4 vault ending in ${caseInsensitive ? "(any case) " : ""}${suffix}`);
console.log(`Vault index: ${vaultIndex}; threads: ${threads}; program: ${SQUADS_PROGRAM_ID.toBase58()}`);

const started = Date.now();
let totalAttempts = 0;
let finished = false;
const workers: Worker[] = [];
const timer = setInterval(() => {
  const elapsedSeconds = (Date.now() - started) / 1000;
  const rate = elapsedSeconds > 0 ? Math.round(totalAttempts / elapsedSeconds) : 0;
  console.log(`${totalAttempts.toLocaleString()} attempts (${rate.toLocaleString()}/s)`);
}, 10_000);

for (let i = 0; i < threads; i += 1) {
  const worker = new Worker(new URL("./grind-worker.js", import.meta.url), {
    workerData: { suffix, caseInsensitive, vaultIndex },
  });
  workers.push(worker);
  worker.on("message", (message) => {
    if (message.type === "progress") totalAttempts += message.attempts;
    if (message.type !== "found" || finished) return;
    finished = true;
    totalAttempts += message.attempts;
    clearInterval(timer);
    const result: VanityResult = {
      version: 1,
      suffix,
      caseInsensitive,
      programId: SQUADS_PROGRAM_ID.toBase58(),
      vaultIndex,
      createKey: message.createKey,
      createKeySecret: message.createKeySecret,
      multisigPda: message.multisigPda,
      vaultPda: message.vaultPda,
      attempts: totalAttempts,
      elapsedMs: Date.now() - started,
    };
    writeFileSync(output, `${JSON.stringify(result, null, 2)}\n`, { flag: "wx", mode: 0o600 });
    console.log(`Found vault: ${result.vaultPda}`);
    console.log(`Multisig config: ${result.multisigPda}`);
    console.log(`Saved with owner-only permissions: ${output}`);
    for (const other of workers) void other.terminate();
  });
  worker.on("error", (error) => {
    if (!finished) {
      clearInterval(timer);
      console.error(error);
      process.exitCode = 1;
      for (const other of workers) void other.terminate();
    }
  });
}
