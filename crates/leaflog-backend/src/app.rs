//! Axum application: router, handlers, and the two ingress middlewares
//! (the production connection gate and the dev prefix simulator).

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use axum::extract::{ConnectInfo, Request, State};
use axum::http::header::HeaderMap;
use axum::http::{HeaderValue, Method, StatusCode, Uri};
use axum::middleware::{self, Next};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use crate::assets::{Assets, content_type};
use crate::ingress::{IngressPath, content_security_policy, render_index};

/// The single fixed remote address Home Assistant's ingress proxy
/// connects from. In production mode it is the only accepted peer.
pub const SUPERVISOR_INGRESS_IP: IpAddr = IpAddr::V4(Ipv4Addr::new(172, 30, 32, 2));

/// Request body limit: photo uploads (milestone M3) must fit through
/// ingress. Revisited when `ingress_stream` lands.
const BODY_LIMIT_BYTES: usize = 32 * 1024 * 1024;

/// Runtime mode controlling the connection gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Development: any remote address is accepted.
    Dev,
    /// Production: only the Supervisor ingress proxy may connect.
    Production,
}

impl Mode {
    /// Lowercase label used in logs and `/api/health`.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Dev => "development",
            Self::Production => "production",
        }
    }
}

/// Shared application state.
#[derive(Debug, Clone)]
pub struct AppState {
    /// Frontend asset source (embedded bundle or dev directory).
    pub assets: Assets,
    /// Runtime mode for the connection gate.
    pub mode: Mode,
    /// Simulated ingress prefix (development only).
    pub dev_prefix: Option<String>,
    /// SQLite database handle.
    pub db: crate::db::Db,
    /// On-disk photo store.
    pub photos: crate::photos::PhotoStore,
    /// Home Assistant bridge (live data, Supervisor REST).
    pub ha: std::sync::Arc<crate::ha::HaBridge>,
}

impl AppState {
    /// Creates the shared state.
    #[must_use]
    pub const fn new(
        assets: Assets,
        mode: Mode,
        dev_prefix: Option<String>,
        db: crate::db::Db,
        photos: crate::photos::PhotoStore,
        ha: std::sync::Arc<crate::ha::HaBridge>,
    ) -> Self {
        Self {
            assets,
            mode,
            dev_prefix,
            db,
            photos,
            ha,
        }
    }
}

/// Assembles the application router.
///
/// Routes are mounted at the root: Home Assistant's ingress proxy
/// forwards requests *without* the `/api/hassio_ingress/<token>`
/// prefix and sets `X-Ingress-Path`; the injected `<base href>` is what
/// makes the browser build prefixed URLs.
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/", get(index))
        .merge(crate::api::router())
        .fallback(fallback)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            supervisor_gate,
        ))
        // Outside the gate so gate rejections (403) are covered too.
        .layer(middleware::from_fn(security_headers))
        .layer(axum::extract::DefaultBodyLimit::max(BODY_LIMIT_BYTES))
        .with_state(state)
}

/// Strips the dev prefix before routing.
///
/// A `Router::layer` middleware runs after Axum has already picked a
/// route, which is too late to rewrite the path; this wrapper rewrites
/// first. A no-op when no prefix is set; see `tests/ingress.rs`.
#[derive(Debug, Clone)]
pub struct DevPrefixService<S> {
    inner: S,
    prefix: Option<String>,
}

impl<S> tower::Service<axum::extract::Request> for DevPrefixService<S>
where
    S: tower::Service<
            axum::extract::Request,
            Response = axum::response::Response,
            Error = std::convert::Infallible,
        > + Clone
        + Send
        + 'static,
    S::Future: Send,
{
    type Response = axum::response::Response;
    type Error = std::convert::Infallible;
    type Future = S::Future;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut request: axum::extract::Request) -> Self::Future {
        if let Some(prefix) = self.prefix.as_deref() {
            strip_dev_prefix(&mut request, prefix);
        }
        self.inner.call(request)
    }
}

/// The whole application as a serving-ready service with the dev-prefix
/// rewriter applied.
#[must_use]
pub fn service(state: AppState) -> DevPrefixService<Router> {
    let prefix = state.dev_prefix.clone();
    DevPrefixService {
        inner: build_router(state),
        prefix,
    }
}

/// `GET /api/health` — liveness and mode reporting.
// Handler extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
#[allow(clippy::unused_async)] // Axum handlers must be async fns; see docs/DECISIONS.md
async fn health(State(state): State<AppState>) -> Json<Health> {
    Json(Health {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        mode: state.mode.label(),
    })
}

/// Payload of [`health`].
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Health {
    /// `"ok"` when the backend is serving.
    pub status: &'static str,
    /// Crate version.
    pub version: &'static str,
    /// `"development"` or `"production"`.
    pub mode: &'static str,
}

/// `GET /` — the SPA entry point with the runtime base injected.
// Handler extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn index(State(state): State<AppState>, headers: HeaderMap) -> Response {
    serve_index(&state, &headers).await
}

/// SPA fallback: serve real assets, the app shell for deep links, and a
/// plain 404 for unknown API paths.
// Handler extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn fallback(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: Uri,
    method: Method,
) -> Response {
    let path = uri.path();
    if path.starts_with("/api/") {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    }
    if method != Method::GET && method != Method::HEAD {
        return (StatusCode::METHOD_NOT_ALLOWED, "method not allowed").into_response();
    }
    let rel = path.trim_start_matches('/');
    if let Some(bytes) = state.assets.get(rel).await {
        return (
            [(axum::http::header::CONTENT_TYPE, content_type(rel))],
            bytes,
        )
            .into_response();
    }
    serve_index(&state, &headers).await
}

/// Serves `index.html` with the ingress base injected, or a helpful 503
/// when the frontend has not been built.
async fn serve_index(state: &AppState, headers: &HeaderMap) -> Response {
    state.assets.index().await.map_or_else(
        || {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                "frontend not built; run `trunk build` (docs/DEV_SETUP.md)",
            )
                .into_response()
        },
        |template| {
            let base = resolve_base(state, headers);
            let mut response = Html(render_index(&template, &base)).into_response();
            // The CSP hashes the template's inline scripts (the Trunk
            // bootstrap) as well as the injected base script, so it
            // must be built from the same template that was rendered.
            let csp = HeaderValue::from_str(&content_security_policy(&base, &template))
                .expect("CSP is ASCII by construction");
            response
                .headers_mut()
                .insert(axum::http::header::CONTENT_SECURITY_POLICY, csp);
            response
        },
    )
}

/// The per-request ingress base: the sanitized `X-Ingress-Path` header,
/// falling back to the simulated dev prefix (development only).
fn resolve_base(state: &AppState, headers: &HeaderMap) -> IngressPath {
    let raw = headers
        .get("x-ingress-path")
        .and_then(|value| value.to_str().ok());
    let from_header = IngressPath::new(raw);
    if from_header.as_str().is_empty()
        && state.mode == Mode::Dev
        && let Some(prefix) = state.dev_prefix.as_deref()
    {
        return IngressPath::new(Some(prefix));
    }
    from_header
}

/// Security-header middleware: every response — API JSON, photos,
/// assets, the SPA shell, gate rejections, and error paths alike —
/// carries `X-Content-Type-Options: nosniff`, so a browser never
/// content-sniffs photo or asset bytes into an executable type. The app
/// lives on HA's origin (docs/THREAT-MODEL.md §2), where a sniffing
/// mismatch would be amplified well beyond Leaflog's own panel.
// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum middleware extractor contract; see docs/DECISIONS.md
async fn security_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        axum::http::header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response
}

/// Production connection gate: outside development, only Home
/// Assistant's ingress proxy address may talk to this server. There is
/// no login screen because HA has already authenticated the user.
// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum middleware extractor contract; see docs/DECISIONS.md
async fn supervisor_gate(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Response {
    if !peer_allowed(state.mode, addr.ip()) {
        return (
            StatusCode::FORBIDDEN,
            "forbidden: supervisor ingress connections only",
        )
            .into_response();
    }
    next.run(request).await
}

/// The gate decision, isolated for unit tests: in production only Home
/// Assistant's ingress proxy address passes; in development any peer
/// does.
const fn peer_allowed(mode: Mode, ip: IpAddr) -> bool {
    match mode {
        Mode::Dev => true,
        Mode::Production => match ip {
            IpAddr::V4(v4) => {
                let o = v4.octets();
                o[0] == 172 && o[1] == 30 && o[2] == 32 && o[3] == 2
            }
            IpAddr::V6(_) => false,
        },
    }
}

/// Development-only ingress simulator: rewrites a request whose path
/// starts with the configured prefix (for example
/// `/api/hassio_ingress/dev-42`) so a plain browser can play the role
/// of the ingress proxy. A no-op for unmatched paths and in production,
/// where the Supervisor strips the prefix before forwarding.
fn strip_dev_prefix(request: &mut Request, prefix: &str) {
    let path = request.uri().path();
    let stripped = path
        .strip_prefix(prefix)
        .filter(|rest| rest.is_empty() || rest.starts_with('/'))
        .map(|rest| {
            if rest.is_empty() {
                "/".to_owned()
            } else {
                rest.to_owned()
            }
        });
    if let Some(new_path) = stripped {
        let target = match request.uri().query() {
            Some(query) => format!("{new_path}?{query}"),
            None => new_path,
        };
        if let Ok(uri) = target.parse::<Uri>() {
            *request.uri_mut() = uri;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    use axum::body::Body;
    use axum::http::Request;

    use super::{Mode, SUPERVISOR_INGRESS_IP, peer_allowed, strip_dev_prefix};

    #[test]
    fn production_allows_exactly_the_ingress_proxy() {
        assert!(peer_allowed(Mode::Production, SUPERVISOR_INGRESS_IP));
        assert!(!peer_allowed(
            Mode::Production,
            IpAddr::V4(Ipv4Addr::new(172, 30, 32, 3))
        ));
        assert!(!peer_allowed(
            Mode::Production,
            IpAddr::V4(Ipv4Addr::LOCALHOST)
        ));
        assert!(!peer_allowed(
            Mode::Production,
            IpAddr::V6(Ipv6Addr::LOCALHOST)
        ));
    }

    #[test]
    fn development_allows_any_peer() {
        assert!(peer_allowed(Mode::Dev, IpAddr::V4(Ipv4Addr::LOCALHOST)));
        assert!(peer_allowed(Mode::Dev, SUPERVISOR_INGRESS_IP));
    }

    fn rewritten(path: &str, prefix: &str) -> String {
        let mut request = Request::builder()
            .uri(path)
            .body(Body::empty())
            .expect("static parts build a request");
        strip_dev_prefix(&mut request, prefix);
        request.uri().path().to_owned()
    }

    #[test]
    fn prefix_rewrite_rules() {
        let prefix = "/api/hassio_ingress/dev-42";
        assert_eq!(rewritten(prefix, prefix), "/");
        assert_eq!(rewritten(&format!("{prefix}/"), prefix), "/");
        assert_eq!(
            rewritten(&format!("{prefix}/api/strains"), prefix),
            "/api/strains"
        );
        assert_eq!(
            rewritten(&format!("{prefix}/pkg/app.css"), prefix),
            "/pkg/app.css"
        );
        // A path that merely shares characters is left alone.
        assert_eq!(
            rewritten(&format!("{prefix}9/pkg/app.css"), prefix),
            "/api/hassio_ingress/dev-429/pkg/app.css"
        );
    }
}
