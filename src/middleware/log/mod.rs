use hyper::service::Service;
use hyper::Request;
use pin_project_lite::pin_project;
use std::future::Future;
use std::{pin::Pin, task::Poll, time::Instant};

pin_project! {
    pub struct ResponseFuture<F> {
        #[pin]
        pub(crate) response_future: F,
        pub(crate) start: Instant,
        pub(crate) reqinfo: String,
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
                println!(
                    "{}, elapsed: {:.2?} ms",
                    this.reqinfo,
                    this.start.elapsed().as_millis()
                );
                return Poll::Ready(result);
            }
            Poll::Pending => Poll::Pending,
        }
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

impl<S, ReqBody> Service<Request<ReqBody>> for LogRequest<S>
where
    S: Service<Request<ReqBody>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = ResponseFuture<S::Future>;

    fn call(&self, req: Request<ReqBody>) -> Self::Future {
        let start = std::time::Instant::now();
        let reqinfo = format!(
            "request method={}, uri={}, version={:?}",
            req.method(),
            req.uri(),
            req.version()
        );
        let response_future = self.inner.call(req);
        ResponseFuture {
            response_future,
            start,
            reqinfo,
        }
    }
}

// #[derive(Debug, Clone)]
// struct FailureLogRequest<S> {
//     inner: S,
// }

// impl<S, ReqBody> Service<Request<ReqBody>> for FailureLogRequest<S>
// where
//     S: Service<Request<ReqBody>>,
// {
//     type Response = S::Response;
//     type Error = S::Error;
//     type Future = ResponseFuture<S::Future>;

//     fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
//         async {
//             let start = std::time::Instant::now();
//             let response = self.inner.call(req).await;
//             println!("elapsed: {:.2?} ms", start.elapsed().as_millis());
//             response
//         }
//     }
// }
