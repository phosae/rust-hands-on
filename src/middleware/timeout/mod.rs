use pin_project_lite::pin_project;
use std::fmt;
use std::task::Poll;
use std::time::Duration;
use std::{future::Future, pin::Pin};

pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

pin_project! {
    pub struct ResponseFuture<F> {
        #[pin]
        response_future: F,
        #[pin]
        sleep: tokio::time::Sleep,
    }
}

impl<F, Response, Error> Future for ResponseFuture<F>
where
    F: Future<Output = Result<Response, Error>>,
    Error: Into<BoxError>,
{
    type Output = Result<Response, BoxError>;

    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        match this.response_future.poll(cx) {
            Poll::Ready(result) => {
                let result = result.map_err(Into::into);
                return Poll::Ready(result);
            }
            Poll::Pending => {}
        }

        match this.sleep.poll(cx) {
            Poll::Ready(()) => {
                // Construct and return a timeout error.
                let error = Box::new(TimeoutError(()));
                return Poll::Ready(Err(error));
            }
            Poll::Pending => {}
        }

        Poll::Pending
    }
}

#[derive(Debug, Default)]
pub struct TimeoutError(pub(super) ());

impl fmt::Display for TimeoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("request timed out")
    }
}

impl std::error::Error for TimeoutError {}

#[derive(Debug, Clone)]
pub struct Timeout<S> {
    inner: S,
    timeout: Duration,
}

impl<S> Timeout<S> {
    pub fn new(inner: S, timeout: Duration) -> Self {
        Timeout { inner, timeout }
    }
}

impl<S, Request> hyper::service::Service<Request> for Timeout<S>
where
    S: hyper::service::Service<Request>,
    S::Error: Into<BoxError>,
{
    type Response = S::Response;
    type Error = BoxError;
    type Future = ResponseFuture<S::Future>;

    fn call(&mut self, req: Request) -> Self::Future {
        let response_future = self.inner.call(req);
        let sleep = tokio::time::sleep(self.timeout);

        ResponseFuture {
            response_future,
            sleep,
        }
    }
}
