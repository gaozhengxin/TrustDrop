use serde::{ Deserialize, Serialize };

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClientConfig {
    pub publisher_url: String,
    pub aggregator_url: String,
    pub blockberry_base: String,
    pub api_key: String,
    pub send_object_to: Option<String>,
}

impl ClientConfig {
    pub fn new(
        publisher: impl Into<String>,
        aggregator: impl Into<String>,
        blockberry: impl Into<String>,
        api_key: impl Into<String>
    ) -> Self {
        Self {
            publisher_url: publisher.into(),
            aggregator_url: aggregator.into(),
            blockberry_base: blockberry.into(),
            api_key: api_key.into(),
            send_object_to: None,
        }
    }
}
