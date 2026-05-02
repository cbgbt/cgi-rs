//! # tower-cgi
//! This crate provides a mechanism for serving CGI requests using types that implement `tower::Service`.
//! This allows you to use existing frameworks built on tower, like `axum`, to write CGI scripts:
//!
//! ```rust
//! use axum::{routing::get, Router};
//! use tower_cgi::serve_cgi;
//!
//! #[tokio::main]
//! async fn main() {
//!     # std::env::set_var("REQUEST_METHOD", "GET");
//!     # std::env::set_var("CONTENT_LENGTH", "0");
//!     # std::env::set_var("REQUEST_URI", "/");
//!     let app = Router::new().route("/", get(|| async { "Hello, World!" }));
//!     serve_cgi(app).await.unwrap();
//! }
//! ```

use cgi_rs::{CGIError, CGIRequest, CGIResponse};
use snafu::ResultExt;
use std::convert::Infallible;
use std::fmt::Debug;
use std::io::Write;
use http_body_util::{Full, BodyExt};
use hyper::body::{Body, Bytes};
use hyper::{Request, Response};
use tower::{Service, ServiceExt};

/// Serve a CGI application.
///
/// Responses are emitted to stdout per the CGI RFC3875
pub async fn serve_cgi<S, B>(app: S) -> Result<()>
where
    S: Service<Request<Full<Bytes>>, Response = Response<B>, Error = Infallible>
        + Clone
        + Send
        + 'static,
    B: Body, <B as Body>::Error: Debug
{
    serve_cgi_with_output(std::io::stdout(), app).await
}

/// Serve a CGI application.
///
/// Responses are emitted to the provided output stream.
pub async fn serve_cgi_with_output<S, B>(output: impl Write, app: S) -> Result<()>
where
    S: Service<Request<Full<Bytes>>, Response = Response<B>, Error = Infallible>
        + Clone
        + Send
        + 'static,
    B: Body, <B as Body>::Error: Debug
{
    let request = CGIRequest::<Full<Bytes>>::from_env()
        .and_then(Request::try_from)
        .context(error::CGIRequestParseSnafu)?;

    let response = app
        .oneshot(request)
        .await
        .expect("The Error type is Infallible, this should never fail.");

    let headers = response.headers().clone();
    let status = response.status().to_string();
    let reason = response.status().canonical_reason().map(|s| s.to_string());

    let collected = response.into_body().collect().await;

    let body_bytes = collected.unwrap().to_bytes();

    let cgi_response = CGIResponse {
        headers,
        status,
        reason,
        body: body_bytes,
    };

    cgi_response
        .write_response_to_output(output)
        .await
        .context(error::CGIResponseWriteSnafu)
}

mod error {
    use super::*;
    use snafu::Snafu;

    #[derive(Debug, Snafu)]
    #[snafu(visibility(pub))]
    pub enum CgiServiceError {
        #[snafu(display("Failed to parse CGI HTTP request: {}", source))]
        CGIRequestParse { source: CGIError },

        #[snafu(display("Failed to convert HTTP response into CGI response: {}", source))]
        CGIResponseParse { source: CGIError },

        #[snafu(display("Failed to write CGI response: {}", source))]
        CGIResponseWrite { source: CGIError },
    }
}

pub use error::CgiServiceError;
type Result<T> = std::result::Result<T, CgiServiceError>;
