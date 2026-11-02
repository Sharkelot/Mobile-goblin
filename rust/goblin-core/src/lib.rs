//! Hermes dashboard protocol primitives for the Rust Mobile Goblin client.
//! This crate powers the native Rust Android app.

use serde::Deserialize;
use serde_json::{Value, json};
use std::net::IpAddr;
use std::time::Duration;
use thiserror::Error;
use url::Url;

#[derive(Debug, Error)]
pub enum GoblinError {
    #[error("Gateway address is required")]
    EmptyAddress,
    #[error("Gateway URL must use http or https")]
    UnsupportedScheme,
    #[error("Gateway URL is invalid: {0}")]
    InvalidUrl(#[from] url::ParseError),
    #[error("Refusing to send a password over public HTTP")]
    InsecureLogin,
    #[error("Gateway request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Gateway returned HTTP {0}: {1}")]
    HttpStatus(reqwest::StatusCode, String),
    #[error("JSON-RPC frame is invalid: {0}")]
    InvalidFrame(String),
}

/// Normalized dashboard base URL. Scheme-less addresses use HTTP, as Hermes Desktop does.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GatewayUrl(Url);

impl GatewayUrl {
    pub fn parse(input: &str) -> Result<Self, GoblinError> {
        let input = input.trim();
        if input.is_empty() {
            return Err(GoblinError::EmptyAddress);
        }
        let with_scheme = if input.contains("://") {
            input.to_owned()
        } else {
            format!("http://{input}")
        };
        let mut url = Url::parse(&with_scheme)?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(GoblinError::UnsupportedScheme);
        }
        if url.host_str().is_none() {
            return Err(GoblinError::InvalidFrame("Gateway URL has no host".into()));
        }
        url.set_query(None);
        url.set_fragment(None);
        let path = url.path().trim_end_matches('/').to_owned();
        url.set_path(&path);
        Ok(Self(url))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str().trim_end_matches('/')
    }

    pub fn resolve(&self, path: &str) -> Result<Url, GoblinError> {
        Ok(self
            .0
            .join(&format!("{}/{path}", self.0.path().trim_end_matches('/')))?)
    }

    pub fn websocket_url(&self) -> Result<Url, GoblinError> {
        let mut url = self.resolve("api/ws")?;
        url.set_scheme(if self.0.scheme() == "https" {
            "wss"
        } else {
            "ws"
        })
        .map_err(|_| GoblinError::UnsupportedScheme)?;
        Ok(url)
    }

    pub fn is_exposed(&self) -> bool {
        self.0.scheme() != "https" && !is_private_host(self.0.host_str().unwrap_or_default())
    }
}

/// Private network names and addresses, including Tailscale's 100.64.0.0/10 range.
pub fn is_private_host(host: &str) -> bool {
    let host = host
        .trim_matches(['[', ']'])
        .trim_end_matches('.')
        .to_ascii_lowercase();
    if host == "localhost"
        || [
            ".ts.net",
            ".local",
            ".lan",
            ".home.arpa",
            ".internal",
            ".localhost",
        ]
        .iter()
        .any(|suffix| host.ends_with(suffix))
        || (!host.contains('.') && !host.contains(':'))
    {
        return true;
    }
    match host.parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) => {
            let [a, b, _, _] = ip.octets();
            ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || (a == 100 && (64..=127).contains(&b))
        }
        Ok(IpAddr::V6(ip)) => {
            ip.is_loopback() || ip.is_unique_local() || ip.is_unicast_link_local()
        }
        Err(_) => false,
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct GatewayStatus {
    pub version: Option<String>,
    #[serde(default)]
    pub auth_required: bool,
    #[serde(default)]
    pub auth_providers: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct WsTicket {
    pub ticket: String,
    #[serde(default = "default_ticket_ttl")]
    pub ttl_seconds: u32,
}

const fn default_ticket_ttl() -> u32 {
    30
}

/// REST operations share one cookie store, so login and WS-ticket requests use the same session.
#[derive(Clone)]
pub struct GatewayClient {
    pub url: GatewayUrl,
    http: reqwest::Client,
}

impl GatewayClient {
    pub fn new(url: GatewayUrl) -> Result<Self, GoblinError> {
        let http = reqwest::Client::builder()
            .cookie_store(true)
            .timeout(Duration::from_secs(20))
            .build()?;
        Ok(Self { url, http })
    }

    pub async fn status(&self) -> Result<GatewayStatus, GoblinError> {
        let response = self
            .http
            .get(self.url.resolve("api/status")?)
            .send()
            .await?;
        Ok(checked(response).await?.json().await?)
    }

    pub async fn password_login(
        &self,
        provider: &str,
        username: &str,
        password: &str,
    ) -> Result<(), GoblinError> {
        if self.url.is_exposed() {
            return Err(GoblinError::InsecureLogin);
        }
        let response = self
            .http
            .post(self.url.resolve("auth/password-login")?)
            .json(&json!({"provider": provider, "username": username, "password": password}))
            .send()
            .await?;
        checked(response).await?;
        Ok(())
    }

    pub async fn ws_ticket(&self) -> Result<WsTicket, GoblinError> {
        let response = self
            .http
            .post(self.url.resolve("api/auth/ws-ticket")?)
            .send()
            .await?;
        Ok(checked(response).await?.json().await?)
    }

    pub async fn sessions(&self, limit: u32, offset: u32) -> Result<Value, GoblinError> {
        let response = self
            .http
            .get(self.url.resolve("api/sessions")?)
            .query(&[("limit", limit.min(100)), ("offset", offset)])
            .send()
            .await?;
        Ok(checked(response).await?.json().await?)
    }
}

async fn checked(response: reqwest::Response) -> Result<reqwest::Response, GoblinError> {
    if response.status().is_success() {
        Ok(response)
    } else {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        let brief: String = body.chars().take(200).collect();
        Err(GoblinError::HttpStatus(status, brief))
    }
}

/// Frames from the dashboard's newline-delimited JSON-RPC WebSocket.
#[derive(Clone, Debug, PartialEq)]
pub enum RpcFrame {
    Event {
        kind: String,
        payload: Value,
        session_id: Option<String>,
        seq: Option<u64>,
    },
    ServerRequest {
        id: String,
        method: String,
        params: Value,
    },
    Response {
        id: String,
        result: Result<Value, RpcFault>,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct RpcFault {
    pub code: i64,
    pub message: String,
    pub data: Option<Value>,
}

pub fn parse_rpc_line(line: &str) -> Result<RpcFrame, GoblinError> {
    let frame: Value =
        serde_json::from_str(line).map_err(|e| GoblinError::InvalidFrame(e.to_string()))?;
    let object = frame
        .as_object()
        .ok_or_else(|| GoblinError::InvalidFrame("object required".into()))?;
    if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Err(GoblinError::InvalidFrame("jsonrpc 2.0 required".into()));
    }
    let id = object.get("id").and_then(Value::as_str).map(str::to_owned);
    let method = object.get("method").and_then(Value::as_str);
    match (id, method) {
        (_, Some("event")) => {
            let params = object
                .get("params")
                .and_then(Value::as_object)
                .ok_or_else(|| GoblinError::InvalidFrame("event params missing".into()))?;
            let kind = params
                .get("type")
                .and_then(Value::as_str)
                .ok_or_else(|| GoblinError::InvalidFrame("event type missing".into()))?;
            Ok(RpcFrame::Event {
                kind: kind.to_owned(),
                payload: params.get("payload").cloned().unwrap_or(Value::Null),
                session_id: params
                    .get("session_id")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                seq: params.get("seq").and_then(Value::as_u64),
            })
        }
        (Some(id), Some(method)) => Ok(RpcFrame::ServerRequest {
            id,
            method: method.to_owned(),
            params: object.get("params").cloned().unwrap_or_else(|| json!({})),
        }),
        (Some(id), None) => {
            let result = if let Some(error) = object.get("error") {
                Err(serde_json::from_value(error.clone())
                    .map_err(|e| GoblinError::InvalidFrame(e.to_string()))?)
            } else {
                Ok(object.get("result").cloned().unwrap_or(Value::Null))
            };
            Ok(RpcFrame::Response { id, result })
        }
        _ => Err(GoblinError::InvalidFrame("method or id missing".into())),
    }
}

pub fn rpc_request(id: &str, method: &str, params: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
}

pub fn rpc_response(id: &str, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}
