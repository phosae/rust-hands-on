use bytes::Bytes;
use std::future::Future;

use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::{header, Request, Response, StatusCode};
use serde::Serialize;

type BoxBody = http_body_util::combinators::BoxBody<Bytes, hyper::Error>;

fn full<T: Into<Bytes>>(chunk: T) -> BoxBody {
    Full::new(chunk.into())
        .map_err(|never| match never {})
        .boxed()
}

fn mk_json_response<T>(value: &T) -> Response<BoxBody>
where
    T: ?Sized + Serialize,
{
    match serde_json::to_string(value) {
        Ok(json) => Response::builder()
            .header(header::CONTENT_TYPE, "application/json")
            .body(full(json))
            .unwrap(),
        Err(_) => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(full("Internal Server Error".as_bytes()))
            .unwrap(),
    }
}

#[allow(dead_code)]
async fn sugarfn(
    _: Request<Incoming>,
) -> std::result::Result<Response<BoxBody>, std::convert::Infallible> {
    Ok(mk_json_response("{}"))
}

fn _svcfn(
    r: Request<Incoming>,
) -> impl Future<Output = std::result::Result<Response<BoxBody>, std::convert::Infallible>> {
    async move {
        println!("{:?}", r);
        Ok(mk_json_response("{}"))
    }
}

#[test]
fn example() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let mut router = matchit::Router::new();
    let closure = |_: Request<Incoming>| async {
        Ok::<Response<BoxBody>, std::convert::Infallible>(mk_json_response("{}"))
    };
    let svc1 = tower::service_fn(closure);
    let bcsvc1 = tower::util::BoxService::new(svc1);

    let svc2 = tower::service_fn(_svcfn);
    let bsvc2 = tower::util::BoxService::new(svc2);

    router.insert("/v1/images", bcsvc1)?;
    router.insert("/v1/images/push", bsvc2)?;
    Ok(())
}
