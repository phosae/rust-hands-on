use futures_core::ready;
use hyper::{service::Service, Request, Response};
use pin_project_lite::pin_project;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

#[derive(Clone, Debug)]
pub struct AsyncRequireAuthorization<S, T> {
    inner: S,
    auth: T,
}

impl<S, T> AsyncRequireAuthorization<S, T> {
    define_inner_service_accessors!();
}

impl<S, T> AsyncRequireAuthorization<S, T> {
    /// Authorize requests using a custom scheme.
    ///
    /// The `Authorization` header is required to have the value provided.
    pub fn new(inner: S, auth: T) -> AsyncRequireAuthorization<S, T> {
        Self { inner, auth }
    }
}

impl<ReqBody, ResBody, S, Auth> Service<Request<ReqBody>> for AsyncRequireAuthorization<S, Auth>
where
    Auth: AsyncAuthorizeRequest<ReqBody, ResponseBody = ResBody>,
    S: Service<Request<Auth::RequestBody>, Response = Response<ResBody>> + Clone,
{
    type Response = Response<ResBody>;
    type Error = S::Error;
    type Future = ResponseFuture<Auth, S, ReqBody>;

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        let inner = self.inner.clone();
        let authorize = self.auth.authorize(req);

        ResponseFuture {
            state: State::Authorize { authorize },
            service: inner,
        }
    }
}

pin_project! {
    /// Response future for [`AsyncRequireAuthorization`].
    pub struct ResponseFuture<Auth, S, ReqBody>
    where
        Auth: AsyncAuthorizeRequest<ReqBody>,
        S: Service<Request<Auth::RequestBody>>,
    {
        #[pin]
        state: State<Auth::Future, S::Future>,
        service: S,
    }
}

pin_project! {
    #[project = StateProj]
    enum State<A, SFut> {
        Authorize {
            #[pin]
            authorize: A,
        },
        Authorized {
            #[pin]
            fut: SFut,
        },
    }
}

impl<Auth, S, ReqBody, B> Future for ResponseFuture<Auth, S, ReqBody>
where
    Auth: AsyncAuthorizeRequest<ReqBody, ResponseBody = B>,
    S: Service<Request<Auth::RequestBody>, Response = Response<B>>,
{
    type Output = Result<Response<B>, S::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        loop {
            match this.state.as_mut().project() {
                StateProj::Authorize { authorize } => {
                    let auth = ready!(authorize.poll(cx));
                    match auth {
                        Ok(req) => {
                            let fut = this.service.call(req);
                            this.state.set(State::Authorized { fut })
                        }
                        Err(res) => {
                            return Poll::Ready(Ok(res));
                        }
                    };
                }
                StateProj::Authorized { fut } => {
                    return fut.poll(cx);
                }
            }
        }
    }
}

/// Trait for authorizing requests.
pub trait AsyncAuthorizeRequest<B> {
    /// The type of request body returned by `authorize`.
    ///
    /// Set this to `B` unless you need to change the request body type.
    type RequestBody;

    /// The body type used for responses to unauthorized requests.
    type ResponseBody;

    /// The Future type returned by `authorize`
    type Future: Future<Output = Result<Request<Self::RequestBody>, Response<Self::ResponseBody>>>;

    /// Authorize the request.
    ///
    /// If the future resolves to `Ok(request)` then the request is allowed through, otherwise not.
    fn authorize(&mut self, request: Request<B>) -> Self::Future;
}

impl<B, F, Fut, ReqBody, ResBody> AsyncAuthorizeRequest<B> for F
where
    F: FnMut(Request<B>) -> Fut,
    Fut: Future<Output = Result<Request<ReqBody>, Response<ResBody>>>,
{
    type RequestBody = ReqBody;
    type ResponseBody = ResBody;
    type Future = Fut;

    fn authorize(&mut self, request: Request<B>) -> Self::Future {
        self(request)
    }
}
