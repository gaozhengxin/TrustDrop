use serde::{Deserialize, Serialize};

pub const CERTIFICATE_TYPE: &str = "trustdrop.video-sampling";
pub const CERTIFICATE_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VideoSamplingCertificate {
    #[serde(rename = "type")]
    pub certificate_type: String,
    pub version: u32,
    pub sale: CertificateSale,
    pub origin: CertificateOrigin,
    pub sampling: CertificateSampling,
    pub previews: [CertificatePreview; 3],
    pub proof: CertificateProof,
    pub verifier: CertificateVerifier,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CertificateSale {
    pub chain_id: u64,
    pub contract: String,
    pub sale_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CertificateOrigin {
    pub walrus_blob_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CertificateSampling {
    pub spec_hash: String,
    pub seed: String,
    pub external_randomness: String,
    pub random_source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CertificatePreview {
    pub bucket: u8,
    pub cid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CertificateProof {
    pub system: String,
    #[serde(rename = "programVKey")]
    pub program_vkey: String,
    pub public_values: String,
    pub proof_bytes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CertificateVerifier {
    pub chain_id: u64,
    pub address: String,
    pub version: String,
}

impl VideoSamplingCertificate {
    pub fn new(
        sale: CertificateSale,
        origin: CertificateOrigin,
        sampling: CertificateSampling,
        previews: [CertificatePreview; 3],
        proof: CertificateProof,
        verifier: CertificateVerifier,
    ) -> Self {
        Self {
            certificate_type: CERTIFICATE_TYPE.to_owned(),
            version: CERTIFICATE_VERSION,
            sale,
            origin,
            sampling,
            previews,
            proof,
            verifier,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn certificate_uses_stable_public_field_names() {
        let certificate = VideoSamplingCertificate::new(
            CertificateSale {
                chain_id: 1,
                contract: "0x01".into(),
                sale_id: "0x02".into(),
            },
            CertificateOrigin {
                walrus_blob_id: "blob".into(),
            },
            CertificateSampling {
                spec_hash: "0x03".into(),
                seed: "0x04".into(),
                external_randomness: "0x05".into(),
                random_source: "sale-block-hash".into(),
            },
            std::array::from_fn(|bucket| CertificatePreview {
                bucket: bucket as u8,
                cid: format!("cid-{bucket}"),
            }),
            CertificateProof {
                system: "sp1-groth16".into(),
                program_vkey: "0x06".into(),
                public_values: "0x07".into(),
                proof_bytes: "0x08".into(),
            },
            CertificateVerifier {
                chain_id: 1,
                address: "0x09".into(),
                version: "v1".into(),
            },
        );
        let value = serde_json::to_value(certificate).unwrap();
        assert_eq!(value["type"], CERTIFICATE_TYPE);
        assert_eq!(value["version"], CERTIFICATE_VERSION);
        assert_eq!(value["sale"]["chainId"], 1);
        assert_eq!(value["origin"]["walrusBlobId"], "blob");
        assert_eq!(value["sampling"]["randomSource"], "sale-block-hash");
        assert_eq!(value["proof"]["programVKey"], "0x06");
        assert_eq!(value["proof"]["proofBytes"], "0x08");
        assert_eq!(value["previews"].as_array().unwrap().len(), 3);
    }
}
