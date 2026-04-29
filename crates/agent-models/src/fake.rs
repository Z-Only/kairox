use crate::{ModelClient, ModelEvent, ModelRequest};
use async_trait::async_trait;
use futures::{stream, stream::BoxStream};

#[derive(Debug, Clone)]
pub struct FakeModelClient {
    tokens: Vec<String>,
}

impl FakeModelClient {
    pub fn new(tokens: Vec<String>) -> Self {
        Self { tokens }
    }
}

#[async_trait]
impl ModelClient for FakeModelClient {
    async fn stream(
        &self,
        request: ModelRequest,
    ) -> crate::Result<BoxStream<'static, crate::Result<ModelEvent>>> {
        let _ = request;
        let mut events: Vec<crate::Result<ModelEvent>> = self
            .tokens
            .iter()
            .cloned()
            .map(ModelEvent::TokenDelta)
            .map(Ok)
            .collect();
        events.push(Ok(ModelEvent::Completed { usage: None }));
        Ok(Box::pin(stream::iter(events)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ModelEvent, ModelRequest};
    use futures::StreamExt;

    #[tokio::test]
    async fn streams_configured_tokens_then_completion() {
        let client = FakeModelClient::new(vec!["hello".into(), " ".into(), "world".into()]);
        let mut stream = client
            .stream(ModelRequest::user_text("test", "hi"))
            .await
            .unwrap();

        let mut seen = Vec::new();
        while let Some(event) = stream.next().await {
            seen.push(event.unwrap());
        }

        assert_eq!(
            seen,
            vec![
                ModelEvent::TokenDelta("hello".into()),
                ModelEvent::TokenDelta(" ".into()),
                ModelEvent::TokenDelta("world".into()),
                ModelEvent::Completed { usage: None },
            ]
        );
    }
}
