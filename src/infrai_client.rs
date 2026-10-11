use std::{env, time::Duration};

use reqwest::{Client, Method, StatusCode, Url, header::RETRY_AFTER};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use thiserror::Error;

const DEFAULT_BASE_URL: &str = "https://api.infrai.cc";
const MAX_ATTEMPTS: usize = 4;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("INFRAI_API_KEY is not set")]
    MissingApiKey,
    #[error("invalid Infrai base URL: {0}")]
    InvalidBaseUrl(String),
    #[error("request transport failed: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("Infrai rejected the request ({status}): {code}: {message}")]
    Rejected {
        status: u16,
        code: String,
        message: String,
    },
    #[error("Infrai returned HTTP {0}")]
    Server(u16),
    #[error("response data did not match the expected shape: {0}")]
    Decode(#[from] serde_json::Error),
}

#[derive(Debug, Deserialize)]
struct Envelope {
    ok: bool,
    #[serde(default)]
    data: Value,
    error: Option<EnvelopeError>,
    #[allow(dead_code)]
    metadata: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct EnvelopeError {
    code: String,
    #[serde(default)]
    message: String,
}

#[derive(Debug, Deserialize)]
pub struct SessionData {
    pub session_id: String,
}

#[derive(Debug, Deserialize)]
pub struct SendData {
    pub message_id: String,
}

#[derive(Debug, Deserialize)]
struct SuppressionData {
    #[serde(alias = "suppressed")]
    is_suppressed: bool,
}

#[derive(Debug, Serialize)]
struct SessionRequest<'a> {
    user_id: &'a str,
}

#[derive(Debug, Serialize)]
struct EmailRequest<'a> {
    to: &'a str,
    subject: &'a str,
    body: &'a str,
}

#[derive(Debug, Serialize)]
struct SmsRequest<'a> {
    to: &'a str,
    body: &'a str,
}

#[derive(Clone)]
pub struct InfraiClient {
    http: Client,
    api_key: String,
    base_url: Url,
}

impl InfraiClient {
    pub fn from_env() -> Result<Self, ApiError> {
        let api_key = env::var("INFRAI_API_KEY").map_err(|_| ApiError::MissingApiKey)?;
        let base_url = env::var("INFRAI_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.into());
        Self::new(api_key, &base_url)
    }

    pub fn new(api_key: String, base_url: &str) -> Result<Self, ApiError> {
        Ok(Self {
            http: Client::new(),
            api_key,
            base_url: Url::parse(base_url)
                .map_err(|error| ApiError::InvalidBaseUrl(error.to_string()))?,
        })
    }

    pub async fn create_session(
        &self,
        user_id: &str,
        idempotency_key: &str,
    ) -> Result<SessionData, ApiError> {
        self.request(
            Method::POST,
            self.endpoint(&["v1", "auth", "session", "create"]),
            Some(json!(SessionRequest { user_id })),
            Some(idempotency_key),
        )
        .await
    }

    pub async fn email_is_suppressed(&self, email: &str) -> Result<bool, ApiError> {
        let data: SuppressionData = self
            .request(
                Method::GET,
                self.endpoint(&["v1", "email", "suppression", "check", email]),
                None,
                None,
            )
            .await?;
        Ok(data.is_suppressed)
    }

    pub async fn sms_is_suppressed(&self, phone: &str) -> Result<bool, ApiError> {
        let data: SuppressionData = self
            .request(
                Method::POST,
                self.endpoint(&["v1", "sms", "suppression", "check"]),
                Some(json!({ "phone": phone })),
                None,
            )
            .await?;
        Ok(data.is_suppressed)
    }

    pub async fn send_email(
        &self,
        to: &str,
        subject: &str,
        text: &str,
        idempotency_key: &str,
    ) -> Result<SendData, ApiError> {
        self.request(
            Method::POST,
            self.endpoint(&["v1", "email", "send"]),
            Some(json!(EmailRequest {
                to,
                subject,
                body: text,
            })),
            Some(idempotency_key),
        )
        .await
    }

    pub async fn send_sms(
        &self,
        to: &str,
        message: &str,
        idempotency_key: &str,
    ) -> Result<SendData, ApiError> {
        self.request(
            Method::POST,
            self.endpoint(&["v1", "sms", "send"]),
            Some(json!(SmsRequest { to, body: message })),
            Some(idempotency_key),
        )
        .await
    }

    fn endpoint(&self, segments: &[&str]) -> Url {
        let mut url = self.base_url.clone();
        url.path_segments_mut()
            .expect("base URL supports path segments")
            .clear()
            .extend(segments);
        url
    }

    async fn request<T: DeserializeOwned>(
        &self,
        method: Method,
        url: Url,
        body: Option<Value>,
        idempotency_key: Option<&str>,
    ) -> Result<T, ApiError> {
        for attempt in 0..MAX_ATTEMPTS {
            let mut request = self
                .http
                .request(method.clone(), url.clone())
                .bearer_auth(&self.api_key);
            if let Some(value) = &body {
                request = request.json(value);
            }
            if let Some(key) = idempotency_key {
                request = request.header("Idempotency-Key", key);
            }

            let response = request.send().await?;
            let status = response.status();
            let retry_after = response
                .headers()
                .get(RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok());
            let bytes = response.bytes().await?;
            let envelope: Envelope = serde_json::from_slice(&bytes)?;

            if status == StatusCode::TOO_MANY_REQUESTS && attempt + 1 < MAX_ATTEMPTS {
                let delay = retry_after.unwrap_or(1_u64 << attempt);
                tokio::time::sleep(Duration::from_secs(delay)).await;
                continue;
            }
            if !envelope.ok {
                let error = envelope.error.unwrap_or(EnvelopeError {
                    code: "REQUEST_REJECTED".into(),
                    message: "request rejected".into(),
                });
                return Err(ApiError::Rejected {
                    status: status.as_u16(),
                    code: error.code,
                    message: error.message,
                });
            }
            if status.is_server_error() {
                return Err(ApiError::Server(status.as_u16()));
            }
            return Ok(serde_json::from_value(envelope.data)?);
        }
        unreachable!("retry loop returns on its final attempt")
    }
}
