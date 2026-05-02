use crate::{error, CGIError, Result};
use hyper::{http::HeaderValue, HeaderMap, Response};
use snafu::ResultExt;
use std::io::Write;
use bytes::Bytes;
use http_body_util::{Full};
use hyper::body::{Body};

#[derive(Debug)]
pub struct CGIResponse {
    pub headers: HeaderMap<HeaderValue>,
    pub status: String,
    pub reason: Option<String>,
    pub body: Bytes,
}

impl CGIResponse {
    pub async fn write_response_to_output(self, mut output: impl Write) -> Result<()> {
        self.write_status(&mut output).await?;
        self.write_headers(&mut output).await?;
        self.write_body(&mut output).await?;

        Ok(())
    }

    async fn write_status(&self, output: &mut impl Write) -> Result<()> {
        // If a canonical reason is present, write it in the status line.
        if let Some(reason) = &self.reason {
            output
                .write(format!("Status: {} {}\n", self.status, reason).as_bytes())
                .context(error::WriteResponseSnafu)?;
        } else {
            output
                .write(format!("Status: {}\n", self.status).as_bytes())
                .context(error::WriteResponseSnafu)?;
        }
        Ok(())
    }

    async fn write_headers(&self, output: &mut impl Write) -> Result<()> {
        for (key, value) in self.headers.iter() {
            let mut header_bytes = format!("{}: ", key).into_bytes();
            header_bytes.extend(value.as_bytes());
            header_bytes.extend(b"\n");
            output
                .write(&header_bytes)
                .context(error::WriteResponseSnafu)?;
        }

        output.write(b"\n").context(error::WriteResponseSnafu)?;

        Ok(())
    }

    async fn write_body(self, output: &mut impl Write) -> Result<()> {
        let body = self.body;

        output.write(body.as_ref()).context(error::WriteResponseSnafu)?;

        Ok(())
    }
}

