pub mod timeout;

#[derive(Debug, Clone)]
pub struct Middleware<S> {
    #[allow(dead_code)]
    inner: S,
}

impl<S> Middleware<S> {
    #[allow(dead_code)]
    pub fn new(inner: S) -> Self {
        Middleware { inner }
    }
}

// impl<S, Request> hyper::service::Service<Request> for Middleware<S>
// where
//     S: hyper::service::Service<Request>,
//     //S::Error: Into<BoxError>,
// {
//     type Response = S::Response;
//     type Error = S::Error;
//     type Future = S::Future;

//     fn call(&mut self, req: Request) -> Self::Future {
//         let fut = self.inner.call(req);
//         return async move {
//             match fut.await {
//                 Ok(ret) => Ok(ret),
//                 Err(e) => {
//                     if util::type_of(e) == "TimeoutError" {
//                         Ok(util::mk_err_response(
//                             hyper::StatusCode::REQUEST_TIMEOUT,
//                             "",
//                         ))
//                     } else {
//                         Err(e)
//                     }
//                 }
//             }
//         };
//     }
// }
