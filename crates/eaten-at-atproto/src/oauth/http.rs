//! The library's HTTP client, backed by the guarded client so token and
//! PAR requests are subject to the same policy as every other fetch.

use atrium_xrpc::http::{Request, Response};
use atrium_xrpc::HttpClient;
use url::Url;

use crate::http::{GuardedClient, RawRequest};

#[derive(Debug, Clone)]
pub(super) struct Adapter(pub(super) GuardedClient);

type BoxError = Box<dyn std::error::Error + Send + Sync + 'static>;

impl HttpClient for Adapter {
    async fn send_http(&self, request: Request<Vec<u8>>) -> Result<Response<Vec<u8>>, BoxError> {
        let (parts, body) = request.into_parts();
        let url = Url::parse(&parts.uri.to_string())?;
        let response = self
            .0
            .send_raw(RawRequest {
                method: parts.method,
                url,
                headers: parts.headers,
                body,
            })
            .await?;
        let mut builder = Response::builder().status(response.status);
        if let Some(headers) = builder.headers_mut() {
            *headers = response.headers;
        }
        Ok(builder.body(response.body.to_vec())?)
    }
}
