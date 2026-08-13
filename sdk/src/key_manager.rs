use anyhow::{Result, anyhow};

const SELLER_ASSET_KEY_DOMAIN: &[u8] = b"trustdrop:seller-asset-encryption-key:v1";

pub fn derive_asset_encryption_key_from_seller_key(
    seller_private_key: &str,
    chain_id: u64,
) -> Result<[u8; 32]> {
    let seller_key = parse_private_key_bytes(seller_private_key)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(SELLER_ASSET_KEY_DOMAIN);
    hasher.update(&chain_id.to_be_bytes());
    hasher.update(&seller_key);
    Ok(*hasher.finalize().as_bytes())
}

pub fn asset_key_commitment(asset_encryption_key: &[u8; 32]) -> [u8; 32] {
    *blake3::hash(asset_encryption_key).as_bytes()
}

fn parse_private_key_bytes(value: &str) -> Result<[u8; 32]> {
    let clean = value.trim().strip_prefix("0x").unwrap_or(value.trim());
    let bytes =
        hex::decode(clean).map_err(|error| anyhow!("invalid seller private key: {error}"))?;
    bytes
        .try_into()
        .map_err(|_| anyhow!("seller private key must be exactly 32 bytes"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seller_key_derivation_is_stable_and_domain_separated() {
        let key = "0x1111111111111111111111111111111111111111111111111111111111111111";
        let a = derive_asset_encryption_key_from_seller_key(key, 421614).unwrap();
        let b = derive_asset_encryption_key_from_seller_key(key, 421614).unwrap();
        let c = derive_asset_encryption_key_from_seller_key(key, 1).unwrap();
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(asset_key_commitment(&a).len(), 32);
    }
}
