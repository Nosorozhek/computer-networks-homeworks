use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use hyper::body::Incoming;
use hyper::service::Service;
use hyper::{Request, Response};
use std::collections::HashMap;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::RwLock;

use crate::regular_request_handler::handle_regular_request;
use crate::{CachedMetadata, black_list::BlackList};

#[derive(Clone)]
pub struct ProxyService {
    black_list: Arc<BlackList>,
    cached_requests: Arc<RwLock<HashMap<String, CachedMetadata>>>,
    cache_dir: PathBuf,
}

impl ProxyService {
    pub fn new(black_list: Arc<BlackList>, cache_dir: String) -> Self {
        Self {
            black_list: black_list,
            cached_requests: Arc::default(),
            cache_dir: PathBuf::from(cache_dir),
        }
    }

    pub fn contains_address(&self, address: &str) -> bool {
        self.black_list.contains_address(address)
    }

    pub async fn cache_response(&self, uri: String, metadata: CachedMetadata, body: Bytes) {
        let path = self.cache_dir.join(&metadata.file_name);

        if let Err(e) = fs::write(&path, &body).await {
            log::error!("Failed to write cache file: {:?}", e);
            return;
        }

        let mut guard = self.cached_requests.write().await;
        guard.insert(uri, metadata);
    }

    pub async fn search_cached(&self, uri: &str) -> Option<(CachedMetadata, Bytes)> {
        let metadata = {
            let guard = self.cached_requests.read().await;
            guard.get(uri).cloned()?
        };

        let path = self.cache_dir.join(&metadata.file_name);
        match fs::read(path).await {
            Ok(data) => Some((metadata, Bytes::from(data))),
            Err(_) => None,
        }
    }
}

impl Service<Request<Incoming>> for ProxyService {
    type Response = Response<BoxBody<Bytes, anyhow::Error>>;

    type Error = anyhow::Error;

    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn call(&self, req: Request<Incoming>) -> Self::Future {
        let service = self.clone();
        let future = async move {
            log::debug!("Received request: {:?}", req);
            handle_regular_request(req, service).await
        };
        Box::pin(future)
    }
}
