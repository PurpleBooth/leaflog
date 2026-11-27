//! Integration tests for ingress-correct serving (spec §13 acceptance:
//! works under an arbitrary prefix with no absolute-path requests, and
//! rejects non-Supervisor addresses in production mode).

use std::net::SocketAddr;

use axum::ServiceExt as _;
use std::path::Path;

use axum::http::{HeaderValue, StatusCode, header};
use leaflog_backend::{AppState, Assets, Db, Mode};
use serde_json::Value;

/// The simulated ingress prefix used for dev-mode tests.
const DEV_PREFIX: &str = "/api/hassio_ingress/it-42";

/// Spawns a backend on an ephemeral port with the fixture frontend and
/// returns its base URL.
async fn spawn_backend(mode: Mode, dev_prefix: Option<String>) -> String {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dist");
    let db = Db::open_in_memory().unwrap();
    let state = AppState::new(
        Assets::Disk(fixture),
        mode,
        dev_prefix,
        db,
        leaflog_backend::photos::PhotoStore::new(std::env::temp_dir()),
        offline_bridge(),
    );
    let app =
        leaflog_backend::app::service(state).into_make_service_with_connect_info::<SocketAddr>();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}")
}

/// Asserts the HTML references no root-absolute or absolute-URL assets.
/// The injected `<base>` tag itself is removed first — it is the one
/// allowed place the (origin-relative) ingress prefix appears.
fn assert_relative_assets_only(html: &str) {
    let mut without_base = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find("<base") {
        without_base.push_str(&rest[..start]);
        match rest[start..].find('>') {
            Some(end) => rest = &rest[start + end + 1..],
            None => {
                rest = "";
            }
        }
    }
    without_base.push_str(rest);
    assert!(
        !without_base.contains("src=\"/"),
        "root-absolute src found in: {html}"
    );
    assert!(
        !without_base.contains("href=\"/"),
        "root-absolute href (besides the injected base) found in: {html}"
    );
    assert!(
        !without_base.contains("http://") && !without_base.contains("https://"),
        "absolute URL found in: {html}"
    );
}

#[tokio::test]
async fn health_reports_ok_and_mode() {
    let base = spawn_backend(Mode::Dev, None).await;
    let health: Value = reqwest::get(format!("{base}/api/health"))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(health["status"], "ok");
    assert_eq!(health["mode"], "development");
    assert!(health["version"].as_str().is_some_and(|v| !v.is_empty()));
}

#[tokio::test]
async fn index_injects_runtime_base_from_ingress_header() {
    let base = spawn_backend(Mode::Dev, None).await;
    let html = reqwest::Client::new()
        .get(format!("{base}/"))
        .header("X-Ingress-Path", "/api/hassio_ingress/abc123")
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(html.contains(r#"<base href="/api/hassio_ingress/abc123/">"#));
    assert!(html.contains(r#"window.__LEAFLOG_BASE__="/api/hassio_ingress/abc123";"#));
    assert_relative_assets_only(&html);
}

#[tokio::test]
async fn ingress_header_trailing_slash_is_normalized() {
    let base = spawn_backend(Mode::Dev, None).await;
    let html = reqwest::Client::new()
        .get(format!("{base}/"))
        .header("X-Ingress-Path", "/api/hassio_ingress/abc123/")
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(html.contains(r#"<base href="/api/hassio_ingress/abc123/">"#));
}

#[tokio::test]
async fn index_without_header_serves_root_base() {
    let base = spawn_backend(Mode::Dev, None).await;
    let html = reqwest::get(format!("{base}/"))
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(html.contains(r#"<base href="/">"#));
    assert!(html.contains("window.__LEAFLOG_BASE__=\"\";"));
    assert_relative_assets_only(&html);
}

#[tokio::test]
async fn hostile_ingress_header_fails_safe_to_root() {
    let base = spawn_backend(Mode::Dev, None).await;
    for hostile in [
        r#"/a"><script>alert(1)</script>"#,
        "/../etc",
        "relative/path",
        "/a b",
    ] {
        let html = reqwest::Client::new()
            .get(format!("{base}/"))
            .header("X-Ingress-Path", hostile)
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert!(html.contains(r#"<base href="/">"#), "hostile {hostile:?}");
        assert!(!html.contains("<script>alert"), "hostile {hostile:?}");
    }
}

#[tokio::test]
async fn deep_link_serves_the_spa_with_injection() {
    let base = spawn_backend(Mode::Dev, None).await;
    let response = reqwest::Client::new()
        .get(format!("{base}/timeline/day/42"))
        .header("X-Ingress-Path", "/api/hassio_ingress/tok")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .is_some_and(|v| v.to_str().unwrap().starts_with("text/html"))
    );
    let html = response.text().await.unwrap();
    assert!(html.contains(r#"window.__LEAFLOG_BASE__="/api/hassio_ingress/tok";"#));
}

#[tokio::test]
async fn assets_are_served_with_content_types() {
    let base = spawn_backend(Mode::Dev, None).await;
    let css = reqwest::get(format!("{base}/pkg/app.css")).await.unwrap();
    assert_eq!(css.status(), StatusCode::OK);
    assert_eq!(
        css.headers().get(header::CONTENT_TYPE).unwrap(),
        "text/css; charset=utf-8"
    );
    assert_eq!(css.text().await.unwrap(), "body { color: green; }\n");
}

/// Extracts the exact content of every inline `<script>` (no `src`
/// attribute) in `html` — an independent re-derivation of how browsers
/// hash inline scripts (byte-for-byte, no whitespace trimming;
/// verified in Chromium 152), so the test cross-checks the
/// implementation instead of calling it.
fn inline_script_contents(html: &str) -> Vec<String> {
    let mut contents = Vec::new();
    let mut rest = html;
    while let Some((_, after_open)) = rest.split_once("<script") {
        let (attributes, after_attributes) = after_open.split_once('>').unwrap_or((after_open, ""));
        let Some((body, tail)) = after_attributes.split_once("</script>") else {
            break;
        };
        if !attributes
            .split_ascii_whitespace()
            .any(|token| token.starts_with("src") || token.starts_with("SRC"))
        {
            contents.push(body.to_owned());
        }
        rest = tail;
    }
    contents
}

#[tokio::test]
async fn csp_allows_every_inline_script_in_the_served_html() {
    let base = spawn_backend(Mode::Dev, None).await;
    let response = reqwest::Client::new()
        .get(format!("{base}/"))
        .header("X-Ingress-Path", "/api/hassio_ingress/abc123")
        .send()
        .await
        .unwrap();
    let csp = response
        .headers()
        .get(header::CONTENT_SECURITY_POLICY)
        .expect("index response carries a CSP")
        .to_str()
        .unwrap()
        .to_owned();
    let html = response.text().await.unwrap();

    // The fixture reproduces the real Trunk 0.21 output shape: the
    // wasm-bindgen bootstrap is an INLINE module script (no src) whose
    // text names per-build hashed files. Together with the injected
    // base script the page carries exactly two inline scripts, and the
    // CSP must carry one matching hash for each — otherwise the SPA
    // never boots (blank panel; see docs/DECISIONS.md D040).
    let inline = inline_script_contents(&html);
    assert_eq!(
        inline.len(),
        2,
        "fixture must serve the Trunk bootstrap plus the injected base script"
    );
    assert!(
        inline.iter().any(|script| script.contains("wasmBindings")),
        "the Trunk-shaped bootstrap must be part of the served page"
    );

    use base64::Engine as _;
    use sha2::Digest as _;
    let engine = base64::engine::general_purpose::STANDARD;
    for script in &inline {
        let hash = engine.encode(sha2::Sha256::digest(script.as_bytes()));
        assert!(
            csp.contains(&format!("'sha256-{hash}'")),
            "CSP must allow the inline script {script:?} by hash; policy: {csp}"
        );
    }

    let script_src = csp
        .split(';')
        .find(|directive| directive.trim().starts_with("script-src"))
        .expect("script-src directive present");
    assert!(
        !script_src.contains("'unsafe-inline'"),
        "script-src must never allow unsafe-inline: {csp}"
    );
}

#[tokio::test]
async fn responses_carry_nosniff_security_header() {
    let base = spawn_backend(Mode::Dev, None).await;
    let client = reqwest::Client::new();

    // The ingress-injected SPA entry point must not be content-sniffable.
    let index = client
        .get(format!("{base}/"))
        .header("X-Ingress-Path", "/api/hassio_ingress/abc123")
        .send()
        .await
        .unwrap();
    assert_eq!(index.status(), StatusCode::OK);
    assert_eq!(
        index
            .headers()
            .get("x-content-type-options")
            .map(|value| value.to_str().unwrap()),
        Some("nosniff"),
        "index response must carry X-Content-Type-Options: nosniff"
    );

    // Static assets served by the fallback handler equally must not be
    // content-sniffable (photo/asset responses are the sniffing target).
    let asset = client
        .get(format!("{base}/pkg/app.css"))
        .send()
        .await
        .unwrap();
    assert_eq!(asset.status(), StatusCode::OK);
    assert_eq!(
        asset
            .headers()
            .get("x-content-type-options")
            .map(|value| value.to_str().unwrap()),
        Some("nosniff"),
        "asset response must carry X-Content-Type-Options: nosniff"
    );
}

#[tokio::test]
async fn unknown_api_path_is_plain_404() {
    let base = spawn_backend(Mode::Dev, None).await;
    let response = reqwest::get(format!("{base}/api/definitely-not-here"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn non_get_asset_path_is_405() {
    let base = spawn_backend(Mode::Dev, None).await;
    let response = reqwest::Client::new()
        .post(format!("{base}/whatever"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn dev_prefix_simulation_strips_and_injects() {
    let base = spawn_backend(Mode::Dev, Some(DEV_PREFIX.to_owned())).await;
    let client = reqwest::Client::new();

    // The exact prefix serves the index.
    let index = client
        .get(format!("{base}{DEV_PREFIX}"))
        .send()
        .await
        .unwrap();
    assert_eq!(index.status(), StatusCode::OK);
    let html = index.text().await.unwrap();
    assert!(html.contains(&format!(r#"<base href="{DEV_PREFIX}/">"#)));
    assert!(html.contains(&format!(r#"window.__LEAFLOG_BASE__="{DEV_PREFIX}";"#)));

    // Deep links through the prefix serve the SPA.
    let deep = client
        .get(format!("{base}{DEV_PREFIX}/spike"))
        .send()
        .await
        .unwrap();
    assert_eq!(deep.status(), StatusCode::OK);
    assert!(deep.text().await.unwrap().contains("__LEAFLOG_BASE__"));

    // API routes must resolve through the prefix too (regression: the
    // rewrite must happen before routing, not in a Router::layer).
    let api = client
        .get(format!("{base}{DEV_PREFIX}/api/health"))
        .send()
        .await
        .unwrap();
    assert_eq!(api.status(), StatusCode::OK);
    let body: serde_json::Value = api.json().await.unwrap();
    assert_eq!(body["status"], "ok");

    // Assets resolve through the prefix.
    let css = client
        .get(format!("{base}{DEV_PREFIX}/pkg/app.css"))
        .send()
        .await
        .unwrap();
    assert_eq!(css.status(), StatusCode::OK);

    // A path that merely shares characters with the prefix is not
    // stripped. Unstripped, it is an unknown /api/... path (ingress
    // prefixes live under /api/), so it must 404 — and it must not
    // serve the asset it almost-names.
    let sibling = client
        .get(format!("{base}{DEV_PREFIX}9/pkg/app.css"))
        .send()
        .await
        .unwrap();
    assert_eq!(
        sibling.status(),
        StatusCode::NOT_FOUND,
        "prefix-sharing path must not be stripped and served as an asset"
    );
}

#[tokio::test]
async fn production_rejects_foreign_addresses() {
    let base = spawn_backend(Mode::Production, None).await;
    let client = reqwest::Client::new();
    for path in ["/", "/api/health", "/pkg/app.css"] {
        let response = client.get(format!("{base}{path}")).send().await.unwrap();
        assert_eq!(
            response.status(),
            StatusCode::FORBIDDEN,
            "production must reject non-Supervisor peers for {path}"
        );
    }
}

/// A bridge pointed at an unroutable address: REST/WS fail fast and the
/// app must still work (spec: works with HA unreachable).
fn offline_bridge() -> std::sync::Arc<leaflog_backend::ha::HaBridge> {
    std::sync::Arc::new(leaflog_backend::ha::HaBridge::spawn(
        "http://127.0.0.1:1".to_owned(),
        leaflog_backend::ha::SupervisorToken::new("test-token"),
        leaflog_backend::Db::open_in_memory().unwrap(),
    ))
}

#[tokio::test]
async fn theme_harness_serves_in_dev_mode_under_the_prefix() {
    let base = spawn_backend(Mode::Dev, Some(DEV_PREFIX.to_owned())).await;
    let response = reqwest::get(format!("{base}{DEV_PREFIX}/theme-harness"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .is_some_and(|value| value.to_str().unwrap().starts_with("text/html"))
    );
    let html = response.text().await.unwrap();
    assert!(
        html.contains("setHaTheme"),
        "harness must expose the switcher"
    );
    // The frame target is relative, so the page stays prefix-safe.
    assert!(html.contains(r#"src="./""#));
    assert_relative_assets_only(&html);
}

#[tokio::test]
async fn theme_harness_is_not_routed_in_production() {
    let base = spawn_backend(Mode::Production, None).await;
    // The connection gate rejects foreign peers first; the harness
    // route is not even registered in production mode.
    let response = reqwest::get(format!("{base}/theme-harness")).await.unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn assets_revalidate_with_their_etag() {
    let base = spawn_backend(Mode::Dev, None).await;
    let client = reqwest::Client::new();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dist/pkg/app.js");
    let disk = std::fs::read_to_string(&fixture).unwrap();

    let first = client
        .get(format!("{base}/pkg/app.js"))
        .send()
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::OK);
    let etag = first
        .headers()
        .get(header::ETAG)
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    assert!(etag.starts_with('"'), "strong ETag");
    // Unhashed names are revalidated, never reused blind.
    assert_eq!(
        first.headers().get(header::CACHE_CONTROL).unwrap(),
        "no-cache"
    );
    assert_eq!(first.text().await.unwrap(), disk);

    // Unchanged: the browser's copy is still good, nothing is sent.
    let revalidated = client
        .get(format!("{base}/pkg/app.js"))
        .header(header::IF_NONE_MATCH, etag.as_str())
        .send()
        .await
        .unwrap();
    assert_eq!(revalidated.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(
        revalidated.headers().get(header::ETAG).unwrap(),
        etag.as_str()
    );
    assert_eq!(
        revalidated.headers().get(header::CACHE_CONTROL).unwrap(),
        "no-cache"
    );
    assert!(revalidated.bytes().await.unwrap().is_empty());

    // The list form browsers send, weak tags included, matches too.
    let listed = client
        .get(format!("{base}/pkg/app.js"))
        .header(header::IF_NONE_MATCH, format!("W/{etag}, \"deadbeef\""))
        .send()
        .await
        .unwrap();
    assert_eq!(listed.status(), StatusCode::NOT_MODIFIED);

    // Another validator means another file: the body comes back.
    let changed = client
        .get(format!("{base}/pkg/app.js"))
        .header(header::IF_NONE_MATCH, "\"deadbeef\"")
        .send()
        .await
        .unwrap();
    assert_eq!(changed.status(), StatusCode::OK);
    assert_eq!(changed.text().await.unwrap(), disk);
}

#[tokio::test]
async fn index_revalidates_per_rendered_page() {
    let base = spawn_backend(Mode::Dev, None).await;
    let client = reqwest::Client::new();
    let ingress = "/api/hassio_ingress/abc123";
    async fn get(
        client: reqwest::Client,
        base: &str,
        ingress: &str,
        etag: Option<HeaderValue>,
    ) -> reqwest::Response {
        let mut request = client
            .get(format!("{base}/"))
            .header("X-Ingress-Path", ingress);
        if let Some(etag) = etag {
            request = request.header(header::IF_NONE_MATCH, etag);
        }
        request.send().await.unwrap()
    }

    let first = get(client.clone(), &base, ingress, None).await;
    assert_eq!(first.status(), StatusCode::OK);
    let etag = first.headers().get(header::ETAG).unwrap().clone();
    assert_eq!(
        first.headers().get(header::CACHE_CONTROL).unwrap(),
        "no-cache"
    );
    let html = first.text().await.unwrap();

    let same = get(client.clone(), &base, ingress, Some(etag.clone())).await;
    assert_eq!(same.status(), StatusCode::NOT_MODIFIED);
    assert!(same.bytes().await.unwrap().is_empty());

    // Another ingress path is another page (the injected base differs).
    let other = get(client, &base, "/api/hassio_ingress/other", Some(etag)).await;
    assert_eq!(other.status(), StatusCode::OK);
    assert_ne!(other.text().await.unwrap(), html);
}

#[tokio::test]
async fn compressible_responses_are_encoded_with_brotli_or_gzip() {
    let base = spawn_backend(Mode::Dev, None).await;
    let client = reqwest::Client::new();
    let plain = client
        .get(format!("{base}/"))
        .send()
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap();
    assert!(plain.len() > 32, "fixture page clears the size threshold");

    for (accepted, expected) in [("br, gzip", "br"), ("gzip", "gzip")] {
        let response = client
            .get(format!("{base}/"))
            .header(header::ACCEPT_ENCODING, accepted)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_ENCODING).unwrap(),
            expected
        );
        assert_eq!(
            response.headers().get(header::VARY).unwrap(),
            "accept-encoding",
            "caches need to know the representation varies"
        );
        let encoded = response.bytes().await.unwrap();
        assert_ne!(encoded, plain, "the body is the encoded representation");
    }

    // Without an offer the identity representation is served untouched.
    let identity = client.get(format!("{base}/")).send().await.unwrap();
    assert!(identity.headers().get(header::CONTENT_ENCODING).is_none());
    assert_eq!(identity.bytes().await.unwrap(), plain);
}

#[tokio::test]
async fn api_responses_are_never_stored() {
    let base = spawn_backend(Mode::Dev, None).await;
    let response = reqwest::get(format!("{base}/api/health")).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CACHE_CONTROL).unwrap(),
        "no-store"
    );
}

#[tokio::test]
async fn listener_speaks_http2_prior_knowledge() {
    let base = spawn_backend(Mode::Dev, None).await;
    let client = reqwest::Client::builder()
        .http2_prior_knowledge()
        .build()
        .unwrap();
    let response = client
        .get(format!("{base}/api/health"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.version(), reqwest::Version::HTTP_2);
}
