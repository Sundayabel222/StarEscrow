/// XDR base64 encoding/decoding utilities for Soroban RPC.
///
/// Soroban RPC transmits XDR payloads as standard base64 (with padding).
/// All encoding/decoding in the CLI must go through these helpers to ensure
/// consistency and avoid ad-hoc implementations.
use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};

/// Encode raw XDR bytes to a base64 string (standard alphabet, with padding).
pub fn encode(xdr: &[u8]) -> String {
    STANDARD.encode(xdr)
}

/// Decode a base64 string back to raw XDR bytes.
pub fn decode(b64: &str) -> Result<Vec<u8>> {
    STANDARD
        .decode(b64.trim())
        .context("invalid base64-encoded XDR")
}

/// Round-trip helper: encode then immediately decode, returning the bytes.
/// Useful for verifying that a payload survives the encoding cycle.
pub fn roundtrip(xdr: &[u8]) -> Result<Vec<u8>> {
    decode(&encode(xdr))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_empty() {
        assert_eq!(encode(b""), "");
    }

    #[test]
    fn test_decode_empty() {
        assert_eq!(decode("").expect("decode empty"), b"");
    }

    #[test]
    fn test_encode_known_value() {
        // base64("Man") == "TWFu"
        assert_eq!(encode(b"Man"), "TWFu");
    }

    #[test]
    fn test_decode_known_value() {
        assert_eq!(decode("TWFu").expect("decode"), b"Man");
    }

    #[test]
    fn test_roundtrip_arbitrary_bytes() {
        let xdr = b"\x00\x00\x00\x05hello\x00\x00\x00"; // XDR-style string
        let result = roundtrip(xdr).expect("roundtrip");
        assert_eq!(result, xdr);
    }

    #[test]
    fn test_roundtrip_all_byte_values() {
        let xdr: Vec<u8> = (0u8..=255).collect();
        let result = roundtrip(&xdr).expect("roundtrip all bytes");
        assert_eq!(result, xdr);
    }

    #[test]
    fn test_encode_uses_padding() {
        // "a" encodes to "YQ==" — padding must be present
        let encoded = encode(b"a");
        assert_eq!(encoded, "YQ==");
        assert!(encoded.ends_with('='), "standard base64 must pad");
    }

    #[test]
    fn test_decode_with_padding() {
        assert_eq!(decode("YQ==").expect("decode padded"), b"a");
    }

    #[test]
    fn test_decode_trims_whitespace() {
        // RPC responses sometimes include trailing newlines
        assert_eq!(decode("TWFu\n").expect("decode with newline"), b"Man");
    }

    #[test]
    fn test_decode_invalid_returns_error() {
        assert!(decode("not!!valid@@base64").is_err());
    }

    #[test]
    fn test_encode_decode_xdr_like_payload() {
        // Simulate a minimal Soroban XDR envelope (arbitrary bytes)
        let xdr = b"\x00\x00\x00\x06\x00\x00\x00\x01stellar";
        let encoded = encode(xdr);
        // Must be valid base64 string
        assert!(encoded.chars().all(|c| c.is_alphanumeric() || c == '+' || c == '/' || c == '='));
        let decoded = decode(&encoded).expect("decode xdr payload");
        assert_eq!(decoded, xdr);
    }
}
