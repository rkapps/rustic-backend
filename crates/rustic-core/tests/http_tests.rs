#[cfg(test)]
mod tests {
    use rustic_core::HttpClient;
    use serde::{Deserialize, Serialize};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // ── Client Creation ───────────────────────────────────────────────────────

    #[test]
    fn test_client_creates_successfully() {
        let client = HttpClient::new();
        assert!(client.is_ok());
    }

    // generic response type for tests
    #[derive(Debug, Deserialize, Serialize)]
    struct TestResponse {
        id: String,
        name: String,
    }

    #[tokio::test]
    async fn test_post_deserializes_response() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/resource"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id":   "123",
                "name": "test"
            })))
            .mount(&mock_server)
            .await;

        let client = HttpClient::new().unwrap();
        let response: TestResponse = client
            .post_request(
                format!("{}/resource", mock_server.uri()),
                None,
                serde_json::json!({"key": "value"}),
            )
            .await
            .unwrap();

        assert_eq!(response.id, "123");
        assert_eq!(response.name, "test");
    }
}
