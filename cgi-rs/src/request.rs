use std::convert::Infallible;
use crate::{error, CGIError, MetaVariable, MetaVariableKind, Result};
use hyper::Request;
use hyper::body::{Body, Bytes};
use snafu::ResultExt;
use std::io::{stdin, Read};
use http_body_util::combinators::BoxBody;
use http_body_util::Full;

pub struct CGIRequest<B>  {
    pub request_body: B
}

impl <B> CGIRequest<B> where B: Body {
    pub fn from_env() -> Result<CGIRequest<Full<Bytes>>> {
        let content_length = MetaVariableKind::ContentLength
            .from_env()
            .map(|content_length| {
                content_length
                    .as_str()
                    .and_then(|s| s.parse().context(error::InvalidContentLengthSnafu))
            })
            .transpose()?
            .unwrap_or_default();

        let read_content = Self::request_body_from_env(content_length)?;

        let request_body = Bytes::from(read_content);

        let full = Full::from(request_body);

        let result = CGIRequest { request_body: full };

        Ok(result)
    }

    pub fn var(&self, kind: MetaVariableKind) -> Option<MetaVariable> {
        kind.from_env()
    }

    fn try_var(&self, kind: MetaVariableKind) -> Result<MetaVariable> {
        kind.try_from_env()
    }

    fn request_body_from_env(content_length: usize) -> Result<Vec<u8>> {
        let mut request_body = vec![0u8; content_length];
        stdin()
            .read_exact(&mut request_body)
            .context(error::ReadRequestBodySnafu)
            .and(Ok(request_body))
    }

    pub fn uri(&self) -> Result<String> {
        // Some CGI implementations (e.g. Apache) set REQUEST_URI, which isn't in the RFC
        self.var(MetaVariableKind::RequestUri)
            .map(|uri| Ok(uri.as_str()?.to_string()))
            .unwrap_or_else(|| {

                let path_info_str = match MetaVariableKind::PathInfo.try_from_env() {
                    Ok(meta_variable) => {
                       String::from(meta_variable.as_str().unwrap_or(""))
                    }
                    Err(_) => {
                        String::from("")
                    }
                };

                let script_name = MetaVariableKind::ScriptName.try_from_env()?;
                let query_string = MetaVariableKind::QueryString.try_from_env()?;
                Ok(format!(
                    "{}{}?{}",
                    script_name.as_str()?,
                    path_info_str,
                    query_string.as_str()?
                ))
            })
    }
}

macro_rules! try_set_headers {
    ($request_builder:expr, $cgi_request:expr, $([$header:expr, $value:expr]),* $(,)?) => {
        $(
            if let Some(value) = $cgi_request.var($value) {
                $request_builder = $request_builder.header($header, value.as_bytes());
            }
        )*
    };
}

impl <B>TryFrom<CGIRequest<B>> for Request<B> where B: Body {
    type Error = CGIError;

    fn try_from(cgi_request: CGIRequest<B>) -> Result<Self> {

        let mut request_builder = Request::builder()
            .method(
                cgi_request
                    .try_var(MetaVariableKind::RequestMethod)?
                    .as_bytes(),
            )
            .uri(cgi_request.uri()?);

        try_set_headers!(
            request_builder,
            cgi_request,
            ["Content-Length", MetaVariableKind::ContentLength],
            ["Accept", MetaVariableKind::HttpAccept],
            ["Host", MetaVariableKind::HttpHost],
            ["User-Agent", MetaVariableKind::HttpUserAgent],
            ["Cookie", MetaVariableKind::HttpCookie],
        );

        request_builder
            .body(cgi_request.request_body)
            .context(error::RequestParseSnafu)
    }
}
