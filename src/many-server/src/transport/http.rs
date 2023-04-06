use crate::transport::LowLevelManyRequestHandler;
use anyhow::anyhow;
use coset::{CoseSign1, TaggedCborSerializable};
use std::fmt::Debug;
use std::io::Cursor;
use std::net::ToSocketAddrs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tiny_http::{Request, Response};
use tracing::info;

/// Maximum of 2MB per HTTP request.
const READ_BUFFER_LEN: usize = 1024 * 1024 * 2;

#[derive(Debug)]
pub struct HttpServer<E: LowLevelManyRequestHandler> {
    executor: E,
    term_signal: Arc<AtomicBool>,
}

impl<E: LowLevelManyRequestHandler> HttpServer<E> {
    pub fn new(executor: E) -> Self {
        Self {
            executor,
            term_signal: Arc::new(AtomicBool::new(false)),
        }
    }

    async fn handle_request(&self, request: &mut Request) -> Response<std::io::Cursor<Vec<u8>>> {
        match request.body_length() {
            Some(x) if x > READ_BUFFER_LEN => {
                // This is a transport error, and as such an HTTP error.
                return Response::empty(500u16).with_data(Cursor::new(vec![]), Some(0));
            }
            _ => {}
        }

        let mut v = Vec::new();
        let _ = request.as_reader().read_to_end(&mut v);

        let bytes = &v;

        tracing::debug!("request  len={}", bytes.len());
        tracing::trace!("request  {}", hex::encode(bytes));

        let envelope = match CoseSign1::from_tagged_slice(bytes) {
            Ok(cs) => cs,
            Err(e) => {
                tracing::debug!(r#"error description="{}""#, e.to_string());
                return Response::empty(500u16).with_data(Cursor::new(vec![]), Some(0));
            }
        };

        let response = self
            .executor
            .execute(envelope)
            .await
            .and_then(|r| r.to_tagged_vec().map_err(|e| e.to_string()));
        let bytes = match response {
            Ok(bytes) => bytes,
            Err(_e) => {
                return Response::empty(500u16).with_data(Cursor::new(vec![]), Some(0));
            }
        };
        tracing::debug!("response len={}", bytes.len());
        tracing::trace!("response {}", hex::encode(&bytes));

        Response::from_data(bytes)
    }

    /// Returns a mutable reference to an atomic bool. Set the bool to true to kill
    /// the server.
    pub fn term_signal(&mut self) -> Arc<AtomicBool> {
        Arc::clone(&self.term_signal)
    }

    pub async fn bind<A: ToSocketAddrs>(&self, addr: A) -> Result<(), anyhow::Error> {
        let server = tiny_http::Server::http(addr).map_err(|e| anyhow!("{}", e))?;

        loop {
            if let Some(mut request) = server.recv_timeout(Duration::from_millis(100))? {
                let response = self.handle_request(&mut request).await;

                // If there's a transport error (e.g. connection closed) on the response itself,
                // we don't actually care and just continue waiting for the next request.
                let _ = request.respond(response);
            }

            // Check for the term signal and break out.
            if self.term_signal.load(Ordering::Relaxed) {
                info!("Server shutting down gracefully...");
                break;
            }
        }

        Ok(())
    }
}
