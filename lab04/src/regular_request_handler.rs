use bytes::Bytes;
use headers::{CacheControl, ETag, HeaderMapExt, Host, IfModifiedSince, IfNoneMatch, LastModified};
use http::StatusCode;
use http_body_util::{BodyExt, Full, combinators::BoxBody};
use hyper::{Method, Request, Response};
use std::time::{Duration, SystemTime};

use crate::CachedMetadata;
use crate::connect_request_handler::handle_connect_request;
use crate::proxy_service::ProxyService;

struct UriParts {
    host: String,
    path: String,
    uri: String,
}

fn split_uri(req: &Request<hyper::body::Incoming>) -> UriParts {
    let (host, path) = if let Some(host) = req.uri().host() {
        let path = req
            .uri()
            .path_and_query()
            .map(|pq| pq.as_str())
            .unwrap_or("/");
        (host.to_string(), path.to_string())
    } else {
        let full_path = req.uri().path().trim_start_matches('/');
        let (host_str, path_str) = match full_path.split_once('/') {
            Some((h, p)) => (h, format!("/{}", p)),
            None => (full_path, "/".to_string()),
        };
        let query = req
            .uri()
            .query()
            .map(|q| format!("?{}", q))
            .unwrap_or_default();
        (host_str.to_string(), format!("{}{}", path_str, query))
    };
    let uri = format!("https://{}{}", host, path);

    UriParts { host, path, uri }
}

fn convert_response<E: Into<anyhow::Error> + 'static>(
    response: Response<BoxBody<Bytes, E>>,
) -> Response<BoxBody<Bytes, anyhow::Error>> {
    response.map(|body| {
        let stream_with_anyhow_error = body.map_err(|e| anyhow::anyhow!(e));
        BoxBody::new(stream_with_anyhow_error)
    })
}

pub async fn handle_regular_request(
    req: Request<hyper::body::Incoming>,
    shared_state: ProxyService,
) -> anyhow::Result<http::Response<BoxBody<Bytes, anyhow::Error>>> {
    let UriParts { host, path, uri } = split_uri(&req);

    if shared_state.contains_address(&host) {
        log::warn!("BLOCKED: {}", uri);
        let mut resp = create_response("<h1>Blocked by proxy<h1>");
        *resp.status_mut() = StatusCode::FORBIDDEN;
        return Ok(convert_response(resp));
    }

    if req.method() == Method::CONNECT {
        return handle_connect_request(req);
    }

    let mut req = req;
    *req.uri_mut() = path.parse()?;
    if let Ok(auth) = host.parse::<http::uri::Authority>() {
        req.headers_mut().typed_insert(Host::from(auth));
    }

    let mut cached_entry = None;
    let is_get_request = req.method() == Method::GET;
    if is_get_request {
        if let Some((metadata, body)) = shared_state.search_cached(&uri).await {
            if SystemTime::now() < metadata.expires_at {
                log::info!("CACHE HIT: {} -> {}", uri, metadata.status);
                let mut response = create_response(body);
                *response.status_mut() = metadata.status;
                *response.headers_mut() = metadata.headers.clone();
                return Ok(convert_response(response));
            } else {
                if let Some(etag) = metadata.headers.typed_get::<ETag>() {
                    req.headers_mut().typed_insert(IfNoneMatch::from(etag));
                }
                if let Some(last_mod) = metadata.headers.typed_get::<LastModified>() {
                    req.headers_mut()
                        .typed_insert(IfModifiedSince::from(SystemTime::from(last_mod)));
                }
                cached_entry = Some((metadata, body));
            }
        }
    }

    let client = reqwest::Client::new();
    let resp = client
        .request(req.method().clone(), &uri)
        .headers(req.headers().clone())
        .body(req.into_body().collect().await?.to_bytes())
        .send()
        .await;
    let response = match resp {
        Ok(r) => r,
        Err(e) => {
            log::warn!("NOT FOUND {}: {}", uri, e);
            let body = Full::new(Bytes::from("502 Bad Gateway: Upstream unreachable")).boxed();

            let error_response = Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .header("Content-Type", "text/plain")
                .body(body)
                .unwrap();

            return Ok(convert_response(error_response));
        }
    };

    log::info!(
        "PROXY: {} {} -> {}",
        if is_get_request { "GET" } else { "POST" },
        uri,
        response.status()
    );

    log::debug!(
        "Received response: {} {:?}",
        response.status(),
        response.headers()
    );
    if response.status() == StatusCode::NOT_MODIFIED {
        if let Some((metadata, body)) = cached_entry {
            let mut response = create_response(body);
            *response.status_mut() = metadata.status;
            *response.headers_mut() = metadata.headers;
            return Ok(convert_response(response));
        }
    }

    let cache_duration_seconds = response
        .headers()
        .typed_get::<CacheControl>()
        .and_then(|cc| cc.max_age())
        .map(|dur| dur.as_secs())
        .unwrap_or(0);

    if is_get_request && cache_duration_seconds > 0 && response.status() == StatusCode::OK {
        let (parts, body) = http::Response::from(response).into_parts();
        let collected_body_bytes = body.collect().await?.to_bytes();
        let expires_at = SystemTime::now() + Duration::from_secs(cache_duration_seconds);

        let metadata = CachedMetadata::new(&uri, expires_at, parts.status, parts.headers.clone());

        shared_state
            .cache_response(uri, metadata, collected_body_bytes.clone())
            .await;

        let new_body = Full::new(collected_body_bytes)
            .map_err(|n| match n {})
            .boxed();
        return Ok(Response::from_parts(parts, new_body));
    }
    Ok(convert_response(
        http::Response::from(response).map(|b| b.boxed()),
    ))
}

fn create_response<T: Into<Bytes>>(chunk: T) -> Response<BoxBody<Bytes, hyper::Error>> {
    Response::new(
        Full::new(chunk.into())
            .map_err(|never| match never {})
            .boxed(),
    )
}
