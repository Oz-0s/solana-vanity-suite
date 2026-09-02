use clap::{Parser, ValueEnum};
use curve25519_dalek::edwards::CompressedEdwardsY;
use ed25519_dalek::{PublicKey, SecretKey};
use rand_chacha::ChaCha8Rng;
use rand_core::{RngCore, SeedableRng};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs::OpenOptions,
    io::Write,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

const SQUADS_PROGRAM: &str = "SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf";
const PDA_MARKER: &[u8] = b"ProgramDerivedAddress";
const PREFIX: &[u8] = b"multisig";
const MULTISIG_SEED: &[u8] = b"multisig";
const VAULT_SEED: &[u8] = b"vault";
const BASE58: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

#[derive(Parser)]
#[command(about = "Optimized offline Squads v4 vanity vault grinder")]
struct Args {
    #[arg(long, value_enum, default_value_t = Kind::SquadsVault)]
    kind: Kind,
    #[arg(long)]
    prefix: Option<String>,
    #[arg(long)]
    suffix: Option<String>,
    #[arg(long)]
    ignore_case: bool,
    #[arg(long, default_value_t = 0)]
    vault_index: u8,
    #[arg(long)]
    threads: Option<usize>,
    #[arg(long, default_value = "results/vanity-result.json")]
    output: PathBuf,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Kind {
    Wallet,
    SquadsVault,
}

#[derive(Clone)]
struct Found {
    create_key: [u8; 32],
    create_key_secret: [u8; 64],
    multisig: [u8; 32],
    vault: [u8; 32],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Output {
    version: u8,
    prefix: Option<String>,
    suffix: String,
    case_insensitive: bool,
    program_id: String,
    vault_index: u8,
    create_key: String,
    create_key_secret: Vec<u8>,
    multisig_pda: String,
    vault_pda: String,
    attempts: u64,
    elapsed_ms: u128,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WalletOutput {
    version: u8,
    kind: &'static str,
    prefix: Option<String>,
    suffix: Option<String>,
    case_insensitive: bool,
    address: String,
    private_key_base58: String,
    secret_key_bytes: Vec<u8>,
    attempts: u64,
    elapsed_ms: u128,
}

fn create_program_address(seeds: &[&[u8]], program: &[u8; 32]) -> Option<[u8; 32]> {
    let mut hasher = Sha256::new();
    for seed in seeds {
        hasher.update(seed);
    }
    hasher.update(program);
    hasher.update(PDA_MARKER);
    let hash: [u8; 32] = hasher.finalize().into();
    if CompressedEdwardsY(hash).decompress().is_some() {
        None
    } else {
        Some(hash)
    }
}

fn find_program_address(seeds: &[&[u8]], program: &[u8; 32]) -> ([u8; 32], u8) {
    for bump in (0..=255u8).rev() {
        let bump_seed = [bump];
        let mut with_bump = seeds.to_vec();
        with_bump.push(&bump_seed);
        if let Some(address) = create_program_address(&with_bump, program) {
            return (address, bump);
        }
    }
    panic!("unable to find a viable PDA bump");
}

fn derive(create_key: &[u8; 32], vault_index: u8, program: &[u8; 32]) -> ([u8; 32], [u8; 32]) {
    let (multisig, _) = find_program_address(&[PREFIX, MULTISIG_SEED, create_key], program);
    let index = [vault_index];
    let (vault, _) = find_program_address(&[PREFIX, &multisig, VAULT_SEED, &index], program);
    (multisig, vault)
}

fn suffix_residue(suffix: &str) -> Result<(u64, u64), String> {
    let mut modulus = 1u64;
    let mut target = 0u64;
    for byte in suffix.bytes() {
        let digit = BASE58
            .iter()
            .position(|candidate| *candidate == byte)
            .ok_or_else(|| format!("invalid Base58 character: {}", byte as char))? as u64;
        modulus = modulus.checked_mul(58).ok_or("suffix is too long")?;
        target = target * 58 + digit;
    }
    if suffix.is_empty() {
        return Err("suffix cannot be empty".into());
    }
    Ok((modulus, target))
}

fn validate_pattern(pattern: &str) -> Result<(), String> {
    if pattern.is_empty() {
        return Err("vanity pattern cannot be empty".into());
    }
    for byte in pattern.bytes() {
        if !BASE58.contains(&byte) {
            return Err(format!("invalid Base58 character: {}", byte as char));
        }
    }
    Ok(())
}

fn has_suffix(bytes: &[u8; 32], modulus: u64, target: u64) -> bool {
    let mut remainder = 0u64;
    for byte in bytes {
        remainder = (remainder * 256 + *byte as u64) % modulus;
    }
    remainder == target
}

fn address_matches(address: &str, prefix: Option<&str>, suffix: Option<&str>, ignore_case: bool) -> bool {
    if ignore_case {
        let address = address.to_ascii_lowercase();
        prefix.map(|value| address.starts_with(&value.to_ascii_lowercase())).unwrap_or(true)
            && suffix.map(|value| address.ends_with(&value.to_ascii_lowercase())).unwrap_or(true)
    } else {
        prefix.map(|value| address.starts_with(value)).unwrap_or(true)
            && suffix.map(|value| address.ends_with(value)).unwrap_or(true)
    }
}

fn create_private_file(path: &PathBuf) -> std::io::Result<std::fs::File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if args.prefix.is_none() && args.suffix.is_none() {
        return Err("provide --prefix, --suffix, or both".into());
    }
    for pattern in args.prefix.iter().chain(args.suffix.iter()) {
        validate_pattern(pattern)?;
    }
    let residue = if args.prefix.is_none() && !args.ignore_case {
        args.suffix.as_deref().and_then(|value| suffix_residue(value).ok())
    } else {
        None
    };
    let program_vec = bs58::decode(SQUADS_PROGRAM).into_vec()?;
    let program: [u8; 32] = program_vec.try_into().map_err(|_| "invalid program ID")?;
    let threads = args
        .threads
        .unwrap_or_else(|| thread::available_parallelism().map(usize::from).unwrap_or(1));
    if threads == 0 {
        return Err("threads must be at least 1".into());
    }

    // Gate the custom PDA implementation against an official-SDK test vector.
    let zero_key = [0u8; 32];
    let (test_multisig, test_vault) = derive(&zero_key, 0, &program);
    assert_eq!(bs58::encode(test_multisig).into_string(), "EEPqJbpYrwqisgoPt3Vu74YBqRji8mFrRxQdARVfDuNG");
    assert_eq!(bs58::encode(test_vault).into_string(), "6soQChwEoXXbAo17wNPdfLFaxzrAjiAxPif9nbJkDXCm");

    println!("Squads PDA self-test OK");
    println!(
        "Searching {:?} addresses with prefix {:?}, suffix {:?}, ignore-case={} using {} threads",
        args.kind, args.prefix, args.suffix, args.ignore_case, threads
    );
    let started = Instant::now();
    let stop = Arc::new(AtomicBool::new(false));
    let attempts = Arc::new(AtomicU64::new(0));
    let found = Arc::new(Mutex::new(None::<Found>));
    let mut handles = Vec::with_capacity(threads);

    for _ in 0..threads {
        let stop = Arc::clone(&stop);
        let attempts = Arc::clone(&attempts);
        let found = Arc::clone(&found);
        let program = program;
        let vault_index = args.vault_index;
        let kind = args.kind;
        let prefix = args.prefix.clone();
        let suffix = args.suffix.clone();
        let ignore_case = args.ignore_case;
        handles.push(thread::spawn(move || {
            let mut rng_seed = [0u8; 32];
            getrandom::getrandom(&mut rng_seed).expect("OS randomness unavailable");
            let mut rng = ChaCha8Rng::from_seed(rng_seed);
            let mut local_attempts = 0u64;
            while !stop.load(Ordering::Relaxed) {
                let mut seed = [0u8; 32];
                rng.fill_bytes(&mut seed);
                let secret = SecretKey::from_bytes(&seed).expect("32-byte secret");
                let public = PublicKey::from(&secret);
                let create_key = *public.as_bytes();
                let (multisig, vault) = match kind {
                    Kind::Wallet => ([0u8; 32], create_key),
                    Kind::SquadsVault => derive(&create_key, vault_index, &program),
                };
                local_attempts += 1;
                let matches = if let Some((modulus, target)) = residue {
                    has_suffix(&vault, modulus, target)
                } else {
                    let encoded = bs58::encode(vault).into_string();
                    address_matches(&encoded, prefix.as_deref(), suffix.as_deref(), ignore_case)
                };
                if matches {
                    let mut create_key_secret = [0u8; 64];
                    create_key_secret[..32].copy_from_slice(&seed);
                    create_key_secret[32..].copy_from_slice(&create_key);
                    *found.lock().expect("result mutex poisoned") = Some(Found {
                        create_key,
                        create_key_secret,
                        multisig,
                        vault,
                    });
                    attempts.fetch_add(local_attempts, Ordering::Relaxed);
                    stop.store(true, Ordering::Release);
                    return;
                }
                if local_attempts == 10_000 {
                    attempts.fetch_add(local_attempts, Ordering::Relaxed);
                    local_attempts = 0;
                }
            }
            attempts.fetch_add(local_attempts, Ordering::Relaxed);
        }));
    }

    while !stop.load(Ordering::Acquire) {
        thread::sleep(Duration::from_secs(10));
        let count = attempts.load(Ordering::Relaxed);
        let rate = count as f64 / started.elapsed().as_secs_f64();
        println!("{} attempts ({:.0}/s)", count, rate);
    }
    for handle in handles {
        handle.join().expect("worker panicked");
    }

    let result = found.lock().expect("result mutex poisoned").clone().expect("missing result");
    let address = bs58::encode(result.vault).into_string();
    let json = match args.kind {
        Kind::SquadsVault => serde_json::to_vec_pretty(&Output {
            version: 1,
            prefix: args.prefix.clone(),
            suffix: args.suffix.clone().unwrap_or_default(),
            case_insensitive: args.ignore_case,
            program_id: SQUADS_PROGRAM.into(),
            vault_index: args.vault_index,
            create_key: bs58::encode(result.create_key).into_string(),
            create_key_secret: result.create_key_secret.to_vec(),
            multisig_pda: bs58::encode(result.multisig).into_string(),
            vault_pda: address.clone(),
            attempts: attempts.load(Ordering::Relaxed),
            elapsed_ms: started.elapsed().as_millis(),
        })?,
        Kind::Wallet => serde_json::to_vec_pretty(&WalletOutput {
            version: 1,
            kind: "wallet",
            prefix: args.prefix.clone(),
            suffix: args.suffix.clone(),
            case_insensitive: args.ignore_case,
            address: address.clone(),
            private_key_base58: bs58::encode(result.create_key_secret).into_string(),
            secret_key_bytes: result.create_key_secret.to_vec(),
            attempts: attempts.load(Ordering::Relaxed),
            elapsed_ms: started.elapsed().as_millis(),
        })?,
    };
    let mut file = create_private_file(&args.output)?;
    file.write_all(&json)?;
    file.write_all(b"\n")?;
    match args.kind {
        Kind::Wallet => println!("Found wallet: {}", address),
        Kind::SquadsVault => {
            println!("Found vault: {}", address);
            println!("Multisig config: {}", bs58::encode(result.multisig).into_string());
        }
    }
    println!("Saved: {}", args.output.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn residue_matches_base58_suffix() {
        let bytes = bs58::decode("6soQChwEoXXbAo17wNPdfLFaxzrAjiAxPif9nbJkDXCm").into_vec().unwrap();
        let address: [u8; 32] = bytes.try_into().unwrap();
        let (modulus, target) = suffix_residue("DXCm").unwrap();
        assert!(has_suffix(&address, modulus, target));
        let (modulus, target) = suffix_residue("toads").unwrap();
        assert!(!has_suffix(&address, modulus, target));
    }


    #[test]
    fn supports_prefix_suffix_and_both() {
        let address = "ADxP4Z1ARaeAqhUoEDzVQbSNiSztqMWqsMENwVXx1fF1";
        assert!(address_matches(address, Some("ADx"), None, false));
        assert!(address_matches(address, None, Some("fF1"), false));
        assert!(address_matches(address, Some("adx"), Some("FF1"), true));
        assert!(!address_matches(address, Some("adx"), Some("FF1"), false));
    }
}
