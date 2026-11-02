//! Fire-and-forget JSON requests to the authserver. Each request gets a shared slot that the
//! response lands in; `super::poll_auth_responses` drains finished slots once per frame.
//!
//! Native builds run `reqwest` on the tokio runtime from `main`; web builds use `window.fetch`
//! with same-origin credentials so the `rots_session` cookie rides along.

use std::sync::Arc;

use bevy::{ecs::system::SystemParam, prelude::*};
use parking_lot::Mutex;
use serde_json::Value;

use super::AuthBaseUrl;

/// `Ok` carries the JSON body (`Null` for empty bodies such as 204). `Err` carries the
/// authserver's `{"error": code}` code, the HTTP status, or a transport error.
pub type HttpResult = Result<Value, String>;
pub type HttpSlot = Arc<Mutex<Option<HttpResult>>>;

/// Tokio runtime plus a shared HTTP client for auth requests.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Resource, Clone)]
pub struct AuthRuntime {
    runtime: Arc<tokio::runtime::Runtime>,
    client: reqwest::Client,
}

#[cfg(not(target_arch = "wasm32"))]
impl AuthRuntime {
    pub fn new(runtime: Arc<tokio::runtime::Runtime>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("failed to build auth HTTP client");
        Self { runtime, client }
    }
}

#[derive(Clone, Copy)]
enum Method {
    Get,
    Post,
}

/// Everything a system needs to start an auth request.
#[derive(SystemParam)]
pub struct AuthHttp<'w> {
    base: Res<'w, AuthBaseUrl>,
    #[cfg(not(target_arch = "wasm32"))]
    runtime: Res<'w, AuthRuntime>,
}

impl AuthHttp<'_> {
    /// POST `body` as JSON to `{base}{path}`.
    pub fn post(&self, path: &str, body: Value, bearer: Option<String>) -> HttpSlot {
        self.spawn(Method::Post, path, Some(body), bearer)
    }

    /// GET `{base}{path}`.
    pub fn get(&self, path: &str, bearer: Option<String>) -> HttpSlot {
        self.spawn(Method::Get, path, None, bearer)
    }

    fn spawn(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
        bearer: Option<String>,
    ) -> HttpSlot {
        let out = HttpSlot::default();
        let url = format!("{}{path}", self.base.0);
        let what = format!(
            "{} {url}",
            match method {
                Method::Get => "GET",
                Method::Post => "POST",
            }
        );
        #[cfg(not(target_arch = "wasm32"))]
        spawn_native(&self.runtime, method, url, what, body, bearer, out.clone());
        #[cfg(target_arch = "wasm32")]
        spawn_web(method, url, what, body, bearer, out.clone());
        out
    }
}

/// Turn a status + body into the slot result.
///
/// `what` is the request ("POST /api/v1/play"); it goes into every message that is not the
/// authserver's own error code, so a stale client or a proxy error names the call that failed
/// instead of leaving a bare `HTTP 404`. Callers match on the codes (`no_session`, ...), so those
/// are returned unchanged.
fn parse_response(status: u16, text: &str, what: &str) -> HttpResult {
    let json = if text.trim().is_empty() {
        Value::Null
    } else {
        serde_json::from_str(text).map_err(|e| {
            format!(
                "{what}: bad response ({status}): {e} (body: {})",
                snippet(text)
            )
        })?
    };
    if status >= 400 {
        if let Some(code) = json.get("error").and_then(Value::as_str) {
            debug!("{what}: {code}");
            return Err(code.to_string());
        }
        let body = if text.trim().is_empty() {
            String::new()
        } else {
            format!(" (body: {})", snippet(text))
        };
        return Err(format!("{what}: HTTP {status}{body}"));
    }
    Ok(json)
}

/// First part of a response body, on one line: a proxy's plain-text error is the useful part.
fn snippet(text: &str) -> String {
    const LIMIT: usize = 120;
    let flat: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    match flat.char_indices().nth(LIMIT) {
        Some((cut, _)) => format!("{}…", &flat[..cut]),
        None => flat,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn_native(
    rt: &AuthRuntime,
    method: Method,
    url: String,
    what: String,
    body: Option<Value>,
    bearer: Option<String>,
    out: HttpSlot,
) {
    let mut req = match method {
        Method::Get => rt.client.get(&url),
        Method::Post => rt.client.post(&url),
    };
    if let Some(body) = body {
        req = req.json(&body);
    }
    if let Some(token) = bearer {
        req = req.bearer_auth(token);
    }
    rt.runtime.spawn(async move {
        let result = async {
            let resp = req.send().await.map_err(|e| format!("{what}: {e}"))?;
            let status = resp.status().as_u16();
            let text = resp.text().await.map_err(|e| format!("{what}: {e}"))?;
            parse_response(status, &text, &what)
        }
        .await;
        *out.lock() = Some(result);
    });
}

#[cfg(target_arch = "wasm32")]
fn spawn_web(
    method: Method,
    url: String,
    what: String,
    body: Option<Value>,
    bearer: Option<String>,
    out: HttpSlot,
) {
    use wasm_bindgen::{JsCast, JsValue};
    use wasm_bindgen_futures::JsFuture;

    fn js_err(e: JsValue) -> String {
        e.as_string().unwrap_or_else(|| format!("{e:?}"))
    }

    wasm_bindgen_futures::spawn_local(async move {
        let result = async {
            let init = web_sys::RequestInit::new();
            init.set_method(match method {
                Method::Get => "GET",
                Method::Post => "POST",
            });
            init.set_credentials(web_sys::RequestCredentials::SameOrigin);
            let headers = web_sys::Headers::new().map_err(js_err)?;
            headers.set("accept", "application/json").map_err(js_err)?;
            if let Some(token) = bearer {
                headers
                    .set("authorization", &format!("Bearer {token}"))
                    .map_err(js_err)?;
            }
            if let Some(body) = body {
                headers
                    .set("content-type", "application/json")
                    .map_err(js_err)?;
                init.set_body(&JsValue::from_str(&body.to_string()));
            }
            init.set_headers(&headers);

            let request = web_sys::Request::new_with_str_and_init(&url, &init)
                .map_err(|e| format!("{what}: {}", js_err(e)))?;
            let window = web_sys::window().ok_or_else(|| "no window".to_string())?;
            let resp: web_sys::Response = JsFuture::from(window.fetch_with_request(&request))
                .await
                .map_err(js_err)?
                .dyn_into()
                .map_err(js_err)?;
            let text = JsFuture::from(resp.text().map_err(js_err)?)
                .await
                .map_err(js_err)?
                .as_string()
                .unwrap_or_default();
            parse_response(resp.status(), &text, &what)
        }
        .await;
        *out.lock() = Some(result);
    });
}
