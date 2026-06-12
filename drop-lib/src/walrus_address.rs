#![no_std]
use hex;
use core::num::NonZeroU16;
use walrus_core::{ BlobId, encoding::{ EncodingConfig, EncodingFactory as _ } };

/// Computes the Walrus blob ID for the given data using specified encoding configuration.
///
/// This function encodes the data according to Walrus standards and returns the computed blob ID.
///
/// # Arguments
/// * `data` - The raw data to encode and compute blob ID for
/// * `n_shards` - The number of shards to use for encoding (should match publisher's configuration)
///
/// # Returns
/// The `BlobId` computed from the encoded data
///
/// # Errors
/// Returns `DataTooLargeError` if the data is too large to be encoded
pub fn compute_blob_id(
    data: &[u8],
    n_shards: u16
) -> Result<BlobId, walrus_core::encoding::DataTooLargeError> {
    let n_shards = NonZeroU16::new(n_shards).expect("n_shards must be > 0");
    let config = EncodingConfig::new(n_shards);

    // Get the encoding config for the default encoding type (RS2)
    let encoding_config = config.get_for_type(walrus_core::EncodingType::RS2);

    // Compute metadata which includes the blob ID
    let metadata_with_id = encoding_config.compute_metadata(data)?;

    Ok(*metadata_with_id.blob_id())
}

/// Computes the Walrus blob ID using the default 1000 shards configuration.
/// This matches the typical Walrus network configuration.
pub fn compute_blob_id_default(
    data: &[u8]
) -> Result<BlobId, walrus_core::encoding::DataTooLargeError> {
    compute_blob_id(data, 1000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_blob_id_basic() {
        let data =
            "[\"朝辞白帝彩云间\",\"千里江陵一日还\",\"两岸猿声啼不住\",\"轻舟已过万重山\"]\n".as_bytes();

        // Data: [91, 34, 230, 156, 157, 232, 190, 158, 231, 153, 189, 229, 184, 157, 229, 189, 169, 228, 186, 145, 233, 151, 180, 34, 44, 34, 229, 141, 131, 233, 135, 140, 230, 177, 159, 233, 153, 181, 228, 184, 128, 230, 151, 165, 232, 191, 152, 34, 44, 34, 228, 184, 164, 229, 178, 184, 231, 140, 191, 229, 163, 176, 229, 149, 188, 228, 184, 141, 228, 189, 143, 34, 44, 34, 232, 189, 187, 232, 136, 159, 229, 183, 178, 232, 191, 135, 228, 184, 135, 233, 135, 141, 229, 177, 177, 34, 93, 10]
        let blob_id = compute_blob_id_default(data).expect("Should compute blob ID");

        // Verify the blob ID is 32 bytes
        assert_eq!(blob_id.as_ref().len(), 32);
        eprintln!("Computed Blob ID: {}", blob_id.to_string());
        // TGBXYr0km3mLvyNHt6bQXbLHslBR5HAwV8mAQ-HFJIo
        eprintln!("Computed Blob ID (Hex): {}", hex::encode(blob_id));
        // 4c605762bd249b798bbf2347b7a6d05db2c7b25051e4703057c98043e1c5248a

        assert_eq!(blob_id.to_string(), "TGBXYr0km3mLvyNHt6bQXbLHslBR5HAwV8mAQ-HFJIo");
    }

    #[test]
    fn test_compute_blob_id_different_data() {
        let data1 = b"Hello, Walrus!";
        let data2 = b"Hello, World!";

        let blob_id1 = compute_blob_id_default(data1).expect("Should compute blob ID");
        eprintln!("Blob ID for data1: {}", blob_id1.to_string());
        let blob_id2 = compute_blob_id_default(data2).expect("Should compute blob ID");
        eprintln!("Blob ID for data2: {}", blob_id2.to_string());

        // Different data should produce different blob IDs
        assert_ne!(blob_id1, blob_id2);
    }

    #[test]
    fn test_compute_blob_id_empty_data() {
        let data = b"";
        let blob_id = compute_blob_id_default(data).expect("Should compute blob ID for empty data");
        eprintln!("Blob ID for empty data: {}", blob_id.to_string());

        // Even empty data should produce a valid blob ID
        assert_eq!(blob_id.as_ref().len(), 32);
    }

    #[test]
    fn test_compute_blob_id_large_data() {
        // Test with larger data (1MB)
        let data = vec![42u8; 1024 * 1024];
        let blob_id = compute_blob_id_default(&data).expect(
            "Should compute blob ID for larger data"
        );
        eprintln!("Blob ID for large data: {}", blob_id.to_string());

        assert_eq!(blob_id.as_ref().len(), 32);
    }
}
