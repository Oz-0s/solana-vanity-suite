import { parentPort, workerData } from "node:worker_threads";
import { Keypair } from "@solana/web3.js";
import { deriveAddresses, matchesSuffix } from "./shared.js";

if (!parentPort) throw new Error("grind-worker must run as a worker thread");

const { suffix, caseInsensitive, vaultIndex } = workerData as {
  suffix: string;
  caseInsensitive: boolean;
  vaultIndex: number;
};

let attempts = 0;
while (true) {
  const createKey = Keypair.generate();
  const { multisigPda, vaultPda } = deriveAddresses(createKey.publicKey, vaultIndex);
  attempts += 1;

  if (matchesSuffix(vaultPda.toBase58(), suffix, caseInsensitive)) {
    parentPort.postMessage({
      type: "found",
      attempts,
      createKey: createKey.publicKey.toBase58(),
      createKeySecret: Array.from(createKey.secretKey),
      multisigPda: multisigPda.toBase58(),
      vaultPda: vaultPda.toBase58(),
    });
    break;
  }

  if (attempts % 25_000 === 0) parentPort.postMessage({ type: "progress", attempts: 25_000 });
}
