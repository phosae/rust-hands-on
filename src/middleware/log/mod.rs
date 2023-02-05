use std::{pin::Pin, task::Poll, time::Instant};

use hyper::service::Service;
use pin_project_lite::pin_project;
use std::future::Future;

pin_project! {
    pub struct ResponseFuture<F> {
        #[pin]
        pub(crate) response_future: F,
        pub(crate) start: Instant,
    }
}

impl<F, Response> Future for ResponseFuture<F>
where
    F: Future<Output = Response>,
{
    type Output = Response;

    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        match this.response_future.poll(cx) {
            Poll::Ready(result) => {
                println!("Elapsed: {:.2?} ms", this.start.elapsed().as_millis());
                return Poll::Ready(result);
            }
            Poll::Pending => {}
        }
        Poll::Pending
    }
}

#[derive(Debug, Clone)]
pub struct LogRequest<S> {
    inner: S,
}

impl<S> LogRequest<S> {
    pub fn new(inner: S) -> Self {
        LogRequest { inner }
    }
}

impl<S, Request> Service<Request> for LogRequest<S>
where
    S: Service<Request>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = ResponseFuture<S::Future>;

    fn call(&mut self, req: Request) -> Self::Future {
        let start = std::time::Instant::now();
        let response_future = self.inner.call(req);
        ResponseFuture {
            response_future,
            start,
        }
    }
}
