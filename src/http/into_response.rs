use bytes::Bytes;
use http_body_util::{BodyExt, Empty, Full};
use hyper::{header, http::HeaderValue};
use std::{borrow::Cow, fmt};

pub type Response<T = BoxBody> = hyper::Response<T>;
pub type BoxBody = http_body_util::combinators::BoxBody<bytes::Bytes, Error>;

#[derive(Debug)]
pub struct Error {
    inner: tower::BoxError,
}

impl Error {
    /// Create a new `Error` from a boxable error.
    pub fn new(error: impl Into<tower::BoxError>) -> Self {
        Self {
            inner: error.into(),
        }
    }

    #[allow(dead_code)]
    /// Convert an `Error` back into the underlying boxed trait object.
    pub fn into_inner(self) -> tower::BoxError {
        self.inner
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&*self.inner)
    }
}

impl From<hyper::Error> for Error {
    fn from(he: hyper::Error) -> Self {
        Error {
            inner: tower::BoxError::from(he),
        }
    }
}

pub fn boxed<B>(body: B) -> BoxBody
where
    B: http_body_util::BodyExt<Data = bytes::Bytes> + Sync + Send + 'static,
    B::Error: Into<tower::BoxError>,
{
    try_downcast(body).unwrap_or_else(|body| body.map_err(Error::new).boxed())
}

pub(crate) fn try_downcast<T, K>(k: K) -> Result<T, K>
where
    T: 'static,
    K: Send + 'static,
{
    let mut k = Some(k);
    if let Some(k) = <dyn std::any::Any>::downcast_mut::<Option<T>>(&mut k) {
        Ok(k.take().unwrap())
    } else {
        Err(k.unwrap())
    }
}

pub trait IntoResponse {
    /// Create a response.
    fn into_response(self) -> Response;
}

impl<R> IntoResponse for (hyper::StatusCode, R)
where
    R: IntoResponse,
{
    fn into_response(self) -> Response {
        let mut res = self.1.into_response();
        *res.status_mut() = self.0;
        res
    }
}

impl<T, E> IntoResponse for Result<T, E>
where
    T: IntoResponse,
    E: IntoResponse,
{
    fn into_response(self) -> Response {
        match self {
            Ok(value) => value.into_response(),
            Err(err) => err.into_response(),
        }
    }
}

impl<B, F, E> IntoResponse for http_body_util::combinators::MapErr<B, F>
where
    B: http_body::Body<Data = Bytes> + Sync + Send + 'static,
    F: FnMut(B::Error) -> E + Sync + Send + 'static,
    E: Into<tower::BoxError>,
{
    fn into_response(self) -> Response {
        Response::new(boxed(self))
    }
}

impl IntoResponse for &'static str {
    fn into_response(self) -> Response {
        Cow::Borrowed(self).into_response()
    }
}

impl IntoResponse for String {
    fn into_response(self) -> Response {
        Cow::<'static, str>::Owned(self).into_response()
    }
}

impl IntoResponse for Cow<'static, str> {
    fn into_response(self) -> Response {
        let mut res = Full::from(self).into_response();
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8".as_ref()),
        );
        res
    }
}

impl<E> IntoResponse for http_body_util::combinators::BoxBody<bytes::Bytes, E>
where
    E: Into<tower::BoxError> + 'static,
{
    fn into_response(self) -> Response {
        Response::new(boxed(self))
    }
}

impl IntoResponse for Full<Bytes> {
    fn into_response(self) -> Response {
        Response::new(boxed(self))
    }
}

impl IntoResponse for Empty<Bytes> {
    fn into_response(self) -> Response {
        Response::new(boxed(self))
    }
}

impl<B> IntoResponse for Response<B>
where
    B: http_body::Body<Data = Bytes> + Sync + Send + 'static,
    B::Error: Into<tower::BoxError>,
{
    fn into_response(self) -> Response {
        self.map(boxed)
    }
}
