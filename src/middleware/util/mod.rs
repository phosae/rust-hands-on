use futures_util::{future::Map, future::MapOk, FutureExt, TryFutureExt};

#[derive(Clone)]
pub struct MapResponse<S, F> {
    inner: S,
    f: F,
}

impl<S, F> MapResponse<S, F> {
    /// Creates a new `MapResponse` service.
    pub fn new(inner: S, f: F) -> Self {
        MapResponse { f, inner }
    }
}

opaque_future! {
    /// Response future from [`MapResponse`] services.
    ///
    /// [`MapResponse`]: crate::util::MapResponse
    pub type MapResponseFuture<F, N> = MapOk<F, N>;
}

impl<S, F, Request, Response> hyper::service::Service<Request> for MapResponse<S, F>
where
    S: hyper::service::Service<Request>,
    F: FnOnce(S::Response) -> Response + Clone,
{
    type Response = Response;
    type Error = S::Error;
    type Future = MapResponseFuture<S::Future, F>;

    #[inline]
    fn call(&self, request: Request) -> Self::Future {
        MapResponseFuture::new(self.inner.call(request).map_ok(self.f.clone()))
    }
}

#[derive(Clone)]
pub struct MapResult<S, F> {
    inner: S,
    f: F,
}

opaque_future! {
    /// Response future from [`MapResult`] services.
    ///
    /// [`MapResult`]: crate::util::MapResult
    pub type MapResultFuture<F, N> = Map<F, N>;
}

impl<S, F, Request, Response, Error> hyper::service::Service<Request> for MapResult<S, F>
where
    S: hyper::service::Service<Request>,
    Error: From<S::Error>,
    F: FnOnce(Result<S::Response, S::Error>) -> Result<Response, Error> + Clone,
{
    type Response = Response;
    type Error = Error;
    type Future = MapResultFuture<S::Future, F>;

    #[inline]
    fn call(&self, request: Request) -> Self::Future {
        MapResultFuture::new(self.inner.call(request).map(self.f.clone()))
    }
}
