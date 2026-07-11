//! HTTP(S) resource fetching for renders.
//!
//! Replaces `blitz-net`, whose reqwest dependency hard-enables the
//! `native-tls` backend and therefore links OpenSSL on Linux. Depending on
//! reqwest directly with its rustls backend keeps the dependency graph pure
//! Rust and the manylinux wheel builds self-contained.

use blitz_traits::net::{NetHandler, NetProvider, Request};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::Semaphore;

const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64; rv:60.0) Gecko/20100101 Firefox/81.0";

/// Per-request timeout so a hung server can't stall the render settle loop.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Matches real browsers' per-origin cap of 6.
const PER_HOST_MAX_CONCURRENT: usize = 6;

/// Fetches http(s) resources with reqwest, tracking in-flight requests so
/// the render loop can wait for all resources (including cascading ones,
/// e.g. fonts referenced from fetched CSS) to settle.
pub(crate) struct HttpProvider {
    client: reqwest::Client,
    in_flight: Arc<AtomicUsize>,
    per_host_limits: Arc<Mutex<HashMap<String, Arc<Semaphore>>>>,
}

impl HttpProvider {
    pub(crate) fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .expect("failed to build HTTP client");

        Self {
            client,
            in_flight: Arc::new(AtomicUsize::new(0)),
            per_host_limits: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// True when no requests are in flight
    pub(crate) fn is_empty(&self) -> bool {
        self.in_flight.load(Ordering::SeqCst) == 0
    }
}

impl NetProvider for HttpProvider {
    fn fetch(&self, _doc_id: usize, request: Request, handler: Box<dyn NetHandler>) {
        self.in_flight.fetch_add(1, Ordering::SeqCst);
        let guard = InFlightGuard(self.in_flight.clone());

        let client = self.client.clone();
        let per_host_limits = self.per_host_limits.clone();

        tokio::spawn(async move {
            let _guard = guard;

            let host_key = request
                .url
                .host_str()
                .map(str::to_owned)
                .unwrap_or_default();
            let semaphore = {
                let mut map = per_host_limits.lock().unwrap();
                map.entry(host_key)
                    .or_insert_with(|| Arc::new(Semaphore::new(PER_HOST_MAX_CONCURRENT)))
                    .clone()
            };
            let _permit = semaphore
                .acquire()
                .await
                .expect("per-host semaphore was closed");

            let result = async {
                let response = client
                    .request(request.method, request.url.clone())
                    .headers(request.headers)
                    .send()
                    .await?
                    .error_for_status()?;
                let url = response.url().to_string();
                let bytes = response.bytes().await?;
                Ok::<_, reqwest::Error>((url, bytes))
            }
            .await;

            // Failed resources get no response; they render as missing,
            // matching blitz-net's behavior.
            if let Ok((url, bytes)) = result {
                handler.bytes(url, bytes);
            }
        });
    }
}

/// Decrements the in-flight counter when the fetch task finishes,
/// including on panic or cancellation.
struct InFlightGuard(Arc<AtomicUsize>);

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}
