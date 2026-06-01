use super::config::OpenAiCompatibleConfig;
use super::streaming::stream_openai_response;
use crate::{ModelError, ModelEvent, ModelRequest, Result};
use async_trait::async_trait;
use futures::stream::BoxStream;

#[derive(Debug, Clone)]
pub struct OpenAiCompatibleClient {
    pub(super) config: OpenAiCompatibleConfig,
    http: reqwest::Client,
}

impl OpenAiCompatibleClient {
    pub fn new(config: OpenAiCompatibleConfig) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .expect("failed to build reqwest client");
        Self { config, http }
    }

    pub fn from_config(config: OpenAiCompatibleConfig, http: reqwest::Client) -> Self {
        Self { config, http }
    }

    async fn send_streaming(
        &self,
        request: ModelRequest,
    ) -> Result<BoxStream<'static, Result<ModelEvent>>> {
        let body = self.build_chat_request(&request)?;
        let url = format!(
            "{}/chat/completions",
            self.config.base_url.trim_end_matches('/')
        );

        let mut builder = self.http.post(&url).json(&body);
        if let Some(api_key) = self.config.api_key() {
            builder = builder.bearer_auth(&api_key);
        }
        for (key, value) in &self.config.headers {
            builder = builder.header(key.as_str(), value.as_str());
        }

        let response = builder.send().await.map_err(|e| {
            if e.is_connect() || e.is_timeout() {
                ModelError::Connection(e.to_string())
            } else {
                ModelError::Http {
                    status: e.status().map_or(0, |s| s.as_u16()),
                    message: e.to_string(),
                }
            }
        })?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(ModelError::Api {
                status: status.as_u16(),
                message: body,
            });
        }

        Ok(stream_openai_response(response))
    }
}

#[async_trait]
impl crate::ModelClient for OpenAiCompatibleClient {
    async fn stream(
        &self,
        request: ModelRequest,
    ) -> Result<BoxStream<'static, Result<ModelEvent>>> {
        self.send_streaming(request).await
    }
}
