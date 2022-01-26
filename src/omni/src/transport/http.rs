use crate::identity::cose::CoseKeyIdentity;
use crate::transport::{HandlerExecutorAdapter, LowLevelOmniRequestHandler, OmniRequestHandler};
use anyhow::anyhow;
use minicose::CoseSign1;
use std::fmt::Debug;
use std::io::Cursor;
use std::net::ToSocketAddrs;
use tiny_http::{Request, Response};

/// Maximum of 2MB per HTTP request.
const READ_BUFFER_LEN: usize = 1024 * 1024 * 2;

#[derive(Debug)]
pub struct HttpServer<E: LowLevelOmniRequestHandler> {
    executor: E,
}

impl<H: OmniRequestHandler> HttpServer<HandlerExecutorAdapter<H>> {
    pub fn simple(identity: CoseKeyIdentity, handler: H) -> Self {
        Self::new(HandlerExecutorAdapter::new(handler, identity))
    }
}

impl<E: LowLevelOmniRequestHandler> HttpServer<E> {
    pub fn new(executor: E) -> Self {
        Self { executor }
    }

    async fn handle_request(
        &self,
        request: &mut Request,
        _buffer: &mut [u8],
    ) -> Response<std::io::Cursor<Vec<u8>>> {
        match request.body_length() {
            Some(x) if x > READ_BUFFER_LEN => {
                // This is a transport error, and as such an HTTP error.
                return Response::empty(500).with_data(Cursor::new(vec![]), Some(0));
            }
            _ => {}
        }

        let mut v = Vec::new();
        let _ = request.as_reader().read_to_end(&mut v);

        let bytes = &v;

        tracing::debug!("request  len={}", bytes.len());

        let envelope = match CoseSign1::from_bytes(bytes) {
            Ok(cs) => cs,
            Err(e) => {
                tracing::debug!(r#"error description="{}""#, e.to_string());
                return Response::empty(500).with_data(Cursor::new(vec![]), Some(0));
            }
        };

        let response = self
            .executor
            .execute(envelope)
            .await
            .and_then(|r| r.to_bytes().map_err(|e| e.to_string()));
        let bytes = match response {
            Ok(bytes) => bytes,
            Err(_e) => {
                return Response::empty(500).with_data(Cursor::new(vec![]), Some(0));
            }
        };
        tracing::debug!("response len={}", bytes.len());

        Response::from_data(bytes)
    }

    pub fn bind<A: ToSocketAddrs>(&self, addr: A) -> Result<(), anyhow::Error> {
        let mut buffer: Vec<u8> = Vec::new();
        buffer.resize(READ_BUFFER_LEN, 0);
        let server = tiny_http::Server::http(addr).map_err(|e| anyhow!("{}", e))?;

        let runtime = tokio::runtime::Runtime::new().unwrap();

        for mut request in server.incoming_requests() {
            runtime.block_on(async {
                let response = self
                    .handle_request(&mut request, buffer.as_mut_slice())
                    .await;

                // If there's a transport error (e.g. connection closed) on the response itself,
                // we don't actually care and just continue waiting for the next request.
                let _ = request.respond(response);
            });
        }

        Ok(())
    }
}
