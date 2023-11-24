use sha2::{Digest, Sha512};

const CHAIN_PREFIX: &str = "CNI-";
const MAX_CHAIN_LENGTH: usize = 28;

pub fn format_chain_name(name: &str, id: &str) -> String {
    let to_hash = format!("{}{}", name, id);
    let hash = {
        let mut sha = Sha512::default();
        sha.update(to_hash.as_bytes());
        sha.finalize().to_vec()
    };
    let result = format!("{}{}", CHAIN_PREFIX, to_hex_string(&hash));
    result[..MAX_CHAIN_LENGTH.min(result.len())].to_string()
}

fn to_hex_string(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut output, b| {
            let _ = write!(output, "{b:02x}");
            output
        })
}
