use serde::{Deserialize, Serialize};
use http_body_util::{combinators::BoxBody, BodyExt, Full, Empty};
use hyper::Response;
use bytes::Bytes;
use std::{
    convert::Infallible,
    fs,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
}

// 빈 응답문 BoxBody를 생성할 때 사용
pub fn empty() -> BoxBody<Bytes, Infallible> {
    Empty::<Bytes>::new().boxed()
}

// 일반적인 응답문 BoxBody를 생성할 때 사용
pub fn full<T: Into<Bytes>>(chunk: T) -> BoxBody<Bytes, Infallible> {
    Full::new(chunk.into()).boxed()
}

pub fn serve_static_file(path: &str) -> Result<Response<BoxBody<Bytes, Infallible>>, hyper::Error> {

    let login_html: Vec<u8> = fs::read(path)
                .unwrap_or_else(|_| {
                    b"Error Occured".to_vec()
                });
                
    Ok(Response::new(full(Bytes::from(login_html))))
}