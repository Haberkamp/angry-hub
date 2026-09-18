use std::any::type_name;

use futures::FutureExt;
use gpui::http_client::{AsyncBody, HttpClient, Request, Response, Result, Url, anyhow};

pub struct GpuiReqwestClient {
    client: reqwest::blocking::Client,
}

impl GpuiReqwestClient {
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: reqwest::blocking::Client::builder()
                .user_agent("angry-hub")
                .build()?,
        })
    }
}

impl HttpClient for GpuiReqwestClient {
    fn type_name(&self) -> &'static str {
        type_name::<Self>()
    }

    fn user_agent(&self) -> Option<&gpui::http_client::http::HeaderValue> {
        None
    }

    fn send(
        &self,
        req: Request<AsyncBody>,
    ) -> futures::future::BoxFuture<'static, Result<Response<AsyncBody>>> {
        let client = self.client.clone();
        async move {
            let method = req.method().clone();
            let url = req.uri().to_string();
            let headers = req.headers().clone();
            let worker =
                std::thread::spawn(move || -> Result<(u16, Vec<(String, Vec<u8>)>, Vec<u8>)> {
                    let mut builder = client.request(
                        reqwest::Method::from_bytes(method.as_str().as_bytes())
                            .map_err(|e| anyhow!(e))?,
                        &url,
                    );
                    for (name, value) in headers.iter() {
                        builder = builder.header(name.as_str(), value.as_bytes());
                    }
                    let resp = builder.send()?;
                    let status = resp.status().as_u16();
                    let resp_headers = resp
                        .headers()
                        .iter()
                        .map(|(name, value)| (name.as_str().to_string(), value.as_bytes().to_vec()))
                        .collect();
                    let bytes = resp.bytes()?.to_vec();
                    Ok((status, resp_headers, bytes))
                });
            let (status, resp_headers, bytes) = worker
                .join()
                .map_err(|_| anyhow!("http worker panicked"))??;
            let mut response = Response::builder().status(status);
            for (name, value) in resp_headers {
                response = response.header(name, value);
            }
            Ok(response.body(AsyncBody::from(bytes))?)
        }
        .boxed()
    }

    fn proxy(&self) -> Option<&Url> {
        None
    }
}
