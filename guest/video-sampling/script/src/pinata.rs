use drop_lib::cid::compute_ipfs_cid;
use reqwest::multipart::{Form, Part};
use std::{collections::HashMap, env, fs};

const PINATA_UPLOAD_URL: &str = "https://api.pinata.cloud/pinning/pinFileToIPFS";

#[derive(Debug)]
pub enum PinataAuth {
    Jwt(String),
    ApiKey { key: String, secret: String },
}

impl PinataAuth {
    pub fn load() -> Result<Self, String> {
        let values = match env::var("PINATA_CONFIG_FILE") {
            Ok(path) => {
                parse_config(&fs::read_to_string(path).map_err(|error| error.to_string())?)?
            }
            Err(_) => HashMap::new(),
        };
        let get = |name: &str| env::var(name).ok().or_else(|| values.get(name).cloned());
        if let Some(jwt) = get("PINATA_JWT").filter(|value| !value.is_empty()) {
            return Ok(Self::Jwt(jwt));
        }
        match (get("PINATA_API_KEY"), get("PINATA_SECRET_API_KEY")) {
            (Some(key), Some(secret)) if !key.is_empty() && !secret.is_empty() => {
                Ok(Self::ApiKey { key, secret })
            }
            (Some(_), None) | (None, Some(_)) => {
                Err("Pinata API key and secret must be supplied together".to_owned())
            }
            _ => Err("set PINATA_JWT or PINATA_API_KEY plus PINATA_SECRET_API_KEY".to_owned()),
        }
    }
}

pub async fn upload_bytes(
    auth: &PinataAuth,
    name: &str,
    mime: &str,
    bytes: Vec<u8>,
) -> Result<String, String> {
    let local_cid = compute_ipfs_cid(&bytes);
    let file = Part::bytes(bytes)
        .file_name(name.to_owned())
        .mime_str(mime)
        .map_err(|error| error.to_string())?;
    let form = Form::new()
        .part("file", file)
        .text(
            "pinataMetadata",
            serde_json::json!({ "name": name }).to_string(),
        )
        .text(
            "pinataOptions",
            serde_json::json!({ "cidVersion": 1 }).to_string(),
        );
    let request = reqwest::Client::new()
        .post(PINATA_UPLOAD_URL)
        .multipart(form);
    let request = match auth {
        PinataAuth::Jwt(jwt) => request.bearer_auth(jwt),
        PinataAuth::ApiKey { key, secret } => request
            .header("pinata_api_key", key)
            .header("pinata_secret_api_key", secret),
    };
    let body: serde_json::Value = request
        .send()
        .await
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?
        .json()
        .await
        .map_err(|error| error.to_string())?;
    let returned_cid = body["IpfsHash"]
        .as_str()
        .ok_or_else(|| "Pinata response is missing IpfsHash".to_owned())?;
    if returned_cid != local_cid {
        return Err(format!(
            "Pinata CID mismatch for {name}: local={local_cid}, returned={returned_cid}"
        ));
    }
    Ok(local_cid)
}

fn parse_config(contents: &str) -> Result<HashMap<String, String>, String> {
    let mut values = HashMap::new();
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| "invalid Pinata config line".to_owned())?;
        values.insert(key.trim().to_owned(), value.trim().to_owned());
    }
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_parser_keeps_values_after_first_equals_sign() {
        let values = parse_config("PINATA_JWT=header.payload=signature\n").unwrap();
        assert_eq!(values["PINATA_JWT"], "header.payload=signature");
    }

    #[test]
    fn config_parser_ignores_comments_and_blank_lines() {
        let values = parse_config("# external secret\n\nPINATA_API_KEY=key\n").unwrap();
        assert_eq!(values.len(), 1);
        assert_eq!(values["PINATA_API_KEY"], "key");
    }
}
