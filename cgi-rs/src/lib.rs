//! # cgi-rs
//! This module provides mechanisms for mapping a CGI request to hyper::{Request/Response} types, which are used by
//! `hyper` and other HTTP implementations.
//!
//! Current limitations:
//! * Only provides the needed utilities to create CGI scripts, not CGI servers.
//! * Only "Document"-type responses are supported.
//! * Only a subset of the CGI environment variables are hoisted into Requests.
//! * Does not support Windows.
//!
//! ## Examples
//! ### Parsing an HTTP Request
//! ```rust
//! use hyper::Request;
//! use hyper::body::Bytes;
//! use http_body_util::Full;
//! use cgi_rs::CGIRequest;
//!
//! // In a CGI environment, the CGI server would set these variables, as well as others.
//! std::env::set_var("REQUEST_METHOD", "GET");
//! std::env::set_var("CONTENT_LENGTH", "0");
//! std::env::set_var("REQUEST_URI", "/");
//!
//! let cgi_request: Request<Full<Bytes>> = CGIRequest::<Full<Bytes>>::from_env()
//!     .and_then(Request::try_from).unwrap();
//!
//! assert_eq!(cgi_request.method(), "GET");
//! assert_eq!(cgi_request.uri().path(), "/");
//! ```
//!
//! ### Querying for Additional CGI Variables
//! It's simple enough to fetch environment variables, but `cgi-rs` provides a convenient way to fetch and parse
//! CGI environment variables (referred to as "meta-variables" by RFC3875)
//!
//! ```rust
//! # std::env::set_var("REQUEST_METHOD", "GET");
//! let method = cgi_rs::MetaVariableKind::RequestMethod.try_from_env().unwrap();
//!
//! assert_eq!(method.as_str().unwrap(), "GET");
//! ```

use snafu::OptionExt;
use std::env;
use std::ffi::OsString;
// We need to be able to access environment variables as octet sequences.
// While this works, it prevents us from supporting Windows.
use std::os::unix::ffi::OsStrExt;

pub mod request;
pub mod response;

pub use request::CGIRequest;
pub use response::CGIResponse;

/// Contains the value of a CGI "meta-variable".
///
/// Meta-variables are environment variables that are hoisted into Requests as headers.
/// While typically ASCII or UTF-8, the RFC does not clarify an encoding, so these variables are stored as OsString
/// values.
pub struct MetaVariable {
    pub kind: MetaVariableKind,
    pub value: OsString,
}

impl MetaVariable {
    /// Returns the value of the MetaVariable as a String.
    ///
    /// Returns an error if the MetaVariable is not UTF-8 encoded.
    pub fn as_str(&self) -> Result<&str> {
        self.value
            .as_os_str()
            .to_str()
            .context(error::InvalidVariableEncodingSnafu {
                kind: self.kind,
                value: self.value.clone(),
                encoding: "UTF-8",
            })
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.value.as_bytes()
    }
}

// https://datatracker.ietf.org/doc/html/rfc3875#section-4.1
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MetaVariableKind {
    AuthType,
    ContentLength,
    ContentType,
    GatewayInterface,
    PathInfo,
    PathTranslated,
    QueryString,
    RemoteAddr,
    RemoteHost,
    RequestIdent,
    RemoteUser,
    RequestMethod,
    ScriptName,
    ServerName,
    ServerPort,
    ServerProtocol,
    ServerSoftware,

    // Not in the RFC, but emitted from httpd's mod_cgi
    UniqueID,
    HttpHost,
    HttpUserAgent,
    HttpAccept,
    HttpCookie,
    ServerSignature,
    DocumentRoot,
    RequestScheme,
    ContextDocumentRoot,
    ServerAdmin,
    ScriptFilename,
    RemotePort,
    RequestUri,
}

impl MetaVariableKind {
    fn as_str(&self) -> &'static str {
        match self {
            MetaVariableKind::AuthType => "AUTH_TYPE",
            MetaVariableKind::ContentLength => "CONTENT_LENGTH",
            MetaVariableKind::ContentType => "CONTENT_TYPE",
            MetaVariableKind::GatewayInterface => "GATEWAY_INTERFACE",
            MetaVariableKind::PathInfo => "PATH_INFO",
            MetaVariableKind::PathTranslated => "PATH_TRANSLATED",
            MetaVariableKind::QueryString => "QUERY_STRING",
            MetaVariableKind::RemoteAddr => "REMOTE_ADDR",
            MetaVariableKind::RemoteHost => "REMOTE_HOST",
            MetaVariableKind::RequestIdent => "REQUEST_IDENT",
            MetaVariableKind::RemoteUser => "REMOTE_USER",
            MetaVariableKind::RequestMethod => "REQUEST_METHOD",
            MetaVariableKind::ScriptName => "SCRIPT_NAME",
            MetaVariableKind::ServerName => "SERVER_NAME",
            MetaVariableKind::ServerPort => "SERVER_PORT",
            MetaVariableKind::ServerProtocol => "SERVER_PROTOCOL",
            MetaVariableKind::ServerSoftware => "SERVER_SOFTWARE",
            MetaVariableKind::UniqueID => "UNIQUE_ID",
            MetaVariableKind::HttpHost => "HTTP_HOST",
            MetaVariableKind::HttpUserAgent => "HTTP_USER_AGENT",
            MetaVariableKind::HttpAccept => "HTTP_ACCEPT",
            MetaVariableKind::ServerSignature => "SERVER_SIGNATURE",
            MetaVariableKind::DocumentRoot => "DOCUMENT_ROOT",
            MetaVariableKind::RequestScheme => "REQUEST_SCHEME",
            MetaVariableKind::ContextDocumentRoot => "CONTEXT_DOCUMENT_ROOT",
            MetaVariableKind::ServerAdmin => "SERVER_ADMIN",
            MetaVariableKind::ScriptFilename => "SCRIPT_FILENAME",
            MetaVariableKind::RemotePort => "REMOTE_PORT",
            MetaVariableKind::RequestUri => "REQUEST_URI",
            MetaVariableKind::HttpCookie => "HTTP_COOKIE"
        }
    }

    pub fn from_env(&self) -> Option<MetaVariable> {
        env::var_os(self.as_str()).map(|value| MetaVariable { kind: *self, value })
    }

    pub fn try_from_env(&self) -> Result<MetaVariable> {
        let kind = *self;
        env::var_os(self.as_str())
            .map(|value| MetaVariable { kind, value })
            .context(error::MetaVariableNotSetSnafu { kind })
    }
}

impl std::fmt::Display for MetaVariableKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

pub mod error {
    use super::*;
    use snafu::Snafu;

    #[derive(Debug, Snafu)]
    #[snafu(visibility(pub))]
    pub enum CGIError {
        #[snafu(display("Failed to parse environment variables: {}", source))]
        ParseEnv { source: std::env::VarError },

        #[snafu(display("Error fetching meta-varaible '{}' from environment: not set", kind))]
        MetaVariableNotSet { kind: MetaVariableKind },

        #[snafu(display("Failed to parse content-length: {}", source))]
        InvalidContentLength { source: std::num::ParseIntError },

        #[snafu(display(
            "Failed to parse meta-variable '{}' value '{}' as {}",
            kind,
            value.to_string_lossy(),
            encoding
        ))]
        InvalidVariableEncoding {
            kind: MetaVariableKind,
            value: OsString,
            encoding: &'static str,
        },

        #[snafu(display("Failed to read request body from stdin: {}", source))]
        ReadRequestBody { source: std::io::Error },

        #[snafu(display("Failed to parse request: {}", source))]
        RequestParse { source: hyper::http::Error },

        #[snafu(display("Failed to gather response into buffer"))]
        BuildResponse,

        #[snafu(display("Failed to write response: {}", source))]
        WriteResponse { source: std::io::Error },
    }
}

pub use error::CGIError;
type Result<T> = std::result::Result<T, CGIError>;
