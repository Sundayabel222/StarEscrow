/// WASM hash verification utilities.
///
/// Computes SHA-256 hashes of local WASM files and compares them against
/// on-chain hashes fetched via the Stellar CLI, using lowercase hex encoding
/// to match Stellar's format.
use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

/// Compute the SHA-256 hash of a local WASM file and return it as a
/// lowercase hex string (64 characters), matching Stellar's on-chain format.
pub fn hash_wasm_file(path: &std::path::Path) -> Result<String> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("failed to read WASM file: {}", path.display()))?;
    Ok(hash_wasm_bytes(&bytes))
}

/// Compute the SHA-256 hash of raw WASM bytes and return lowercase hex.
pub fn hash_wasm_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// Compare a local WASM file's hash against an expected on-chain hex hash.
/// Returns `Ok(true)` if they match, `Ok(false)` if they differ.
pub fn verify_wasm_hash(path: &std::path::Path, expected_hex: &str) -> Result<bool> {
    let local = hash_wasm_file(path)?;
    Ok(local.eq_ignore_ascii_case(expected_hex.trim()))
}

/// Fetch the on-chain WASM hash for a deployed contract via the Stellar CLI
/// and return it as a lowercase hex string.
pub fn fetch_onchain_hash(
    rpc_url: &str,
    network_passphrase: &str,
    contract_id: &str,
) -> Result<String> {
    let out = std::process::Command::new("stellar")
        .args([
            "contract",
            "fetch",
            "--id",
            contract_id,
            "--rpc-url",
            rpc_url,
            "--network-passphrase",
            network_passphrase,
            "--output",
            "wasm",
        ])
        .output()
        .context("stellar CLI not found — install from https://developers.stellar.org/docs/tools/developer-tools/cli/install-cli")?;

    if !out.status.success() {
        anyhow::bail!(
            "stellar contract fetch failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    Ok(hash_wasm_bytes(&out.stdout))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    fn tmp_wasm(contents: &[u8]) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().expect("tempfile");
        f.write_all(contents).expect("write");
        f
    }

    #[test]
    fn test_hash_known_value() {
        // SHA-256("") = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        let hash = hash_wasm_bytes(b"");
        assert_eq!(hash, "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    }

    #[test]
    fn test_hash_wasm_bytes_is_lowercase_hex() {
        let hash = hash_wasm_bytes(b"some wasm content");
        assert_eq!(hash.len(), 64, "SHA-256 hex must be 64 chars");
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit() && !c.is_uppercase()),
            "hash must be lowercase hex");
    }

    #[test]
    fn test_hash_wasm_file_matches_bytes() {
        let data = b"\x00asm\x01\x00\x00\x00"; // minimal WASM magic + version
        let f = tmp_wasm(data);
        let from_file = hash_wasm_file(f.path()).expect("hash file");
        let from_bytes = hash_wasm_bytes(data);
        assert_eq!(from_file, from_bytes);
    }

    #[test]
    fn test_verify_wasm_hash_match() {
        let data = b"wasm binary data here";
        let f = tmp_wasm(data);
        let expected = hash_wasm_bytes(data);
        assert!(verify_wasm_hash(f.path(), &expected).expect("verify"));
    }

    #[test]
    fn test_verify_wasm_hash_mismatch() {
        let data = b"wasm binary data here";
        let f = tmp_wasm(data);
        let wrong = "0".repeat(64);
        assert!(!verify_wasm_hash(f.path(), &wrong).expect("verify"));
    }

    #[test]
    fn test_verify_wasm_hash_case_insensitive() {
        let data = b"case test";
        let f = tmp_wasm(data);
        let lower = hash_wasm_bytes(data);
        let upper = lower.to_uppercase();
        assert!(verify_wasm_hash(f.path(), &upper).expect("case insensitive verify"));
    }

    #[test]
    fn test_hash_wasm_file_missing_returns_error() {
        let result = hash_wasm_file(std::path::Path::new("/nonexistent/path/contract.wasm"));
        assert!(result.is_err());
    }

    #[test]
    fn test_different_contents_produce_different_hashes() {
        let h1 = hash_wasm_bytes(b"contract v1");
        let h2 = hash_wasm_bytes(b"contract v2");
        assert_ne!(h1, h2);
    }
}
