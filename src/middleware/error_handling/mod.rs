use std::{convert::Infallible, fmt, future::Future, marker::PhantomData, marker::Sync};

use crate::http::into_response::IntoResponse;
use hyper::service::Service;
use hyper::Request;
use hyper_ext::ServiceExt;
//use into_response::IntoResponse;

pub struct HandleError<S, F, T> {
    inner: S,
    f: F,
    _extractor: PhantomData<fn() -> T>,
}

impl<S, F, T> HandleError<S, F, T> {
    #[allow(dead_code)]
    pub fn new(inner: S, f: F) -> Self {
        Self {
            inner,
            f,
            _extractor: PhantomData,
        }
    }
}

impl<S, F, T> Clone for HandleError<S, F, T>
where
    S: Clone,
    F: Clone,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            f: self.f.clone(),
            _extractor: PhantomData,
        }
    }
}

impl<S, F, E> fmt::Debug for HandleError<S, F, E>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HandleError")
            .field("inner", &self.inner)
            .field("f", &format_args!("{}", std::any::type_name::<F>()))
            .finish()
    }
}

impl<S, F, B, Fut, Res> Service<Request<B>> for HandleError<S, F, ()>
where
    S: Service<Request<B>> + Clone + Send + Sync + 'static,
    S::Response: IntoResponse + Send,
    S::Error: Send,
    S::Future: Send,
    F: FnOnce(S::Error) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Res> + Send,
    Res: IntoResponse,
    B: Send + 'static,
{
    type Response = crate::http::into_response::Response;
    type Error = Infallible;
    type Future = future::HandleErrorFuture;

    fn call(&self, req: Request<B>) -> Self::Future {
        let f = self.f.clone();

        // as hyper::Service change from tower_service
        //
        //      fn call(&mut self, req: Request) -> Self::Future
        //
        // to
        //
        //      fn call(&self, req: Request) -> Self::Future
        //
        // there's no need do things like https://github.com/tokio-rs/axum/blob/main/axum/src/error_handling/mod.rs
        //
        //     let clone = self.inner.clone();
        //     let inner = std::mem::replace(&mut self.inner, clone);
        //
        let inner = self.inner.clone();

        let future = Box::pin(async move {
            match inner.oneshot(req).await {
                Ok(res) => Ok(res.into_response()),
                Err(err) => {
                    let map_resp = f(err).await.into_response();
                    Ok(map_resp)
                }
            }
        });

        future::HandleErrorFuture { future }
    }
}

pub mod future {
    //! Future types.
    use crate::http::into_response::Response;

    use pin_project_lite::pin_project;
    use std::{
        convert::Infallible,
        future::Future,
        pin::Pin,
        task::{Context, Poll},
    };

    pin_project! {
        /// Response future for [`HandleError`].
        pub struct HandleErrorFuture {
            #[pin]
            pub(super) future: Pin<Box<dyn Future<Output = Result<Response, Infallible>>
                + Send
                + 'static
            >>,
        }
    }

    impl Future for HandleErrorFuture {
        type Output = Result<Response, Infallible>;

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            self.project().future.poll(cx)
        }
    }
}

pub mod hyper_ext {
    use futures_core::ready;
    use hyper::service::Service;
    use pin_project_lite::pin_project;
    use std::{
        fmt,
        future::Future,
        pin::Pin,
        task::{Context, Poll},
    };

    pin_project! {
        /// A [`Future`] consuming a [`Service`] and request, waiting until the [`Service`]
        /// is ready, and then calling [`Service::call`] with the request, and
        /// waiting for that [`Future`].
        #[derive(Debug)]
        pub struct Oneshot<S: Service<Req>, Req> {
            #[pin]
            state: State<S, Req>,
        }
    }

    pin_project! {
        #[project = StateProj]
        enum State<S:  hyper::service::Service<Req>, Req> {
            NotReady {
                svc: S,
                req: Option<Req>,
            },
            Called {
                #[pin]
                fut: S::Future,
            },
            Done,
        }
    }
    impl<S: Service<Req>, Req> State<S, Req> {
        fn not_ready(svc: S, req: Option<Req>) -> Self {
            Self::NotReady { svc, req }
        }

        fn called(fut: S::Future) -> Self {
            Self::Called { fut }
        }
    }

    impl<S, Req> fmt::Debug for State<S, Req>
    where
        S: Service<Req> + fmt::Debug,
        Req: fmt::Debug,
    {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                State::NotReady {
                    svc,
                    req: Some(req),
                } => f
                    .debug_tuple("State::NotReady")
                    .field(svc)
                    .field(req)
                    .finish(),
                State::NotReady { req: None, .. } => unreachable!(),
                State::Called { .. } => f.debug_tuple("State::Called").field(&"S::Future").finish(),
                State::Done => f.debug_tuple("State::Done").finish(),
            }
        }
    }

    impl<S, Req> Oneshot<S, Req>
    where
        S: Service<Req>,
    {
        #[allow(missing_docs)]
        pub fn new(svc: S, req: Req) -> Self {
            Oneshot {
                state: State::not_ready(svc, Some(req)),
            }
        }
    }

    impl<S, Req> Future for Oneshot<S, Req>
    where
        S: Service<Req>,
    {
        type Output = Result<S::Response, S::Error>;

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let mut this = self.project();
            loop {
                match this.state.as_mut().project() {
                    StateProj::NotReady { svc, req } => {
                        //let _ = ready!(svc.poll_ready(cx))?;
                        let f = svc.call(req.take().expect("already called"));
                        this.state.set(State::called(f));
                    }
                    StateProj::Called { fut } => {
                        let res = ready!(fut.poll(cx))?;
                        this.state.set(State::Done);
                        return Poll::Ready(Ok(res));
                    }
                    StateProj::Done => panic!("polled after complete"),
                }
            }
        }
    }

    pub trait ServiceExt<Request>: Service<Request> {
        fn oneshot(self, req: Request) -> Oneshot<Self, Request>
        where
            Self: Sized,
        {
            Oneshot::new(self, req)
        }
    }

    impl<T: ?Sized, Request> ServiceExt<Request> for T where T: Service<Request> {}
}
