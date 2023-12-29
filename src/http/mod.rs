mod babe_svc_ref;
mod handler;
mod lifetime_handler_sucks;
mod mock_tower_svc;
use http_body_util::{BodyExt, Full};

pub use self::handler::{handler_fn, BoxCloneHandler, Context, Handler};
pub mod into_response;

#[allow(dead_code)]
pub fn type_of<T>(_: &T) -> &str {
    std::any::type_name::<T>()
}

type BoxBody = http_body_util::combinators::BoxBody<bytes::Bytes, hyper::Error>;

fn full<T: Into<bytes::Bytes>>(chunk: T) -> BoxBody {
    Full::new(chunk.into())
        .map_err(|never| match never {})
        .boxed()
}

#[allow(dead_code)]
fn mk_err_response<T: Into<bytes::Bytes>>(
    code: hyper::StatusCode,
    body: T,
) -> hyper::Response<BoxBody> {
    hyper::Response::builder()
        .status(code)
        .body(full(body))
        .unwrap()
}
