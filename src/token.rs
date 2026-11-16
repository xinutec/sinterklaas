//! Secrets handed to a browser — sign-in links and session cookies — and the
//! hashes the database keeps instead of them.

use rand::Rng;
use sha2::{Digest, Sha256};

/// 32 random bytes, hex: what goes in a link or a cookie.
pub fn new_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// What the database stores for `token`: a stolen row cannot be replayed.
pub fn hash(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}
