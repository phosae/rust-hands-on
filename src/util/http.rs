/*
I once want to build a trait in `lifetime_handler_sucks.rs`

    trait Handler<'a, STRUCT, Request> {
        type Response;
        type Future: Future<Output = Self::Response> + 'a;
        fn call(&'a mut self, s: &'a STRUCT, ctx: Context, r: Request) -> Self::Future;
    }

because in Golang passing STRUCT by reference is so common, but in Rust, it sucks.
after read axum's [sqlx-postgres exmaple](https://github.com/tokio-rs/axum/tree/main/examples/sqlx-postgres) and
  read some state implemention code of axum, like
    // put state in Router struct
    let app = Router::new()
        .route(
            "/",
            get(using_connection_pool_extractor).post(using_connection_extractor),
        )
        .with_state(pool);

    // consume state object
    fn using_connection_pool_extractor(State(pool): State<PgPool>

internally axum's sate is based on Clone.

So finally I find impl trait with reference parameter is unneccessary, because Arc<T>, Rc<T> is well called `pass by reference` in Rust.
*/
use std::{future::Future, pin::Pin};

pub struct Context {
    pub vars: std::collections::HashMap<String, String>,
}

pub trait Handler<STRUCT, Request> {
    type Response;
    type Future: Future<Output = Self::Response>;
    fn call(&mut self, s: STRUCT, ctx: Context, request: Request) -> Self::Future;
}

// === ported from tower, don't known how it means.
impl<'a, S, STRUCT, Request> Handler<STRUCT, Request> for &'a mut S
where
    S: Handler<STRUCT, Request> + 'a,
{
    type Response = S::Response;
    type Future = S::Future;

    fn call(&mut self, s: STRUCT, ctx: Context, request: Request) -> S::Future {
        (**self).call(s, ctx, request)
    }
}

impl<S, STRUCT, Request> Handler<STRUCT, Request> for Box<S>
where
    S: Handler<STRUCT, Request> + ?Sized,
{
    type Response = S::Response;
    type Future = S::Future;

    fn call(&mut self, s: STRUCT, ctx: Context, request: Request) -> S::Future {
        (**self).call(s, ctx, request)
    }
}
// === ported from tower, don't known how it means.

#[derive(Copy, Clone)]
pub struct HandlerFn<F>(F);

pub fn handler_fn<F>(f: F) -> HandlerFn<F> {
    HandlerFn(f)
}

impl<F, Fut, STRUCT, Request, Response> Handler<STRUCT, Request> for HandlerFn<F>
where
    F: FnMut(STRUCT, Context, Request) -> Fut,
    Fut: Future<Output = Response>,
{
    type Response = Response;
    type Future = Fut;
    fn call(&mut self, s: STRUCT, ctx: Context, r: Request) -> Self::Future {
        (self.0)(s, ctx, r)
    }
}

trait HandlerExt<STRUCT, Request>: Handler<STRUCT, Request> {
    fn map_future<F, Fut, T>(self, f: F) -> MapFuture<Self, F>
    where
        Self: Sized,
        F: FnMut(Self::Future) -> Fut,
        Fut: Future<Output = T>,
    {
        MapFuture::new(self, f)
    }

    fn boxed(self) -> BoxHandler<STRUCT, Request, Self::Response>
    where
        Self: Sized + Send + 'static,
        Self::Future: Send + 'static,
    {
        BoxHandler::new(self)
    }

    fn boxed_clone(self) -> BoxCloneHandler<STRUCT, Request, Self::Response>
    where
        Self: Clone + Sized + Send + 'static,
        Self::Future: Send + 'static,
    {
        BoxCloneHandler::new(self)
    }
}

impl<T: ?Sized, STRUCT, Request> HandlerExt<STRUCT, Request> for T where T: Handler<STRUCT, Request> {}

#[derive(Clone)]
struct MapFuture<S, F> {
    inner: S,
    f: F,
}

impl<S, F> MapFuture<S, F> {
    #[allow(dead_code)]
    fn new(inner: S, f: F) -> Self {
        Self { inner, f }
    }
}

impl<S, F, Fut, STRUCT, Request, Reponse> Handler<STRUCT, Request> for MapFuture<S, F>
where
    S: Handler<STRUCT, Request>,
    F: FnMut(S::Future) -> Fut,
    // if we mark Fut as
    // Fut: Future<Output = Reponse> + 'static,
    // got err in BoxHandler and BoxCloneHanlder:
    //   the parameter type `Response` may not live long enough
    //   ...so that the type `Response` will meet its required lifetime bounds
    Fut: Future<Output = Reponse>,
{
    type Response = Reponse;
    type Future = Fut;

    fn call(&mut self, s: STRUCT, ctx: Context, r: Request) -> Self::Future {
        (self.f)(self.inner.call(s, ctx, r))
    }
}

type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;

pub struct BoxHandler<STRUCT, Request, Response> {
    inner:
        Box<dyn Handler<STRUCT, Request, Response = Response, Future = BoxFuture<Response>> + Send>,
}

impl<STRUCT, Request, Response> BoxHandler<STRUCT, Request, Response> {
    //impl<STRUCT, Request, Response: 'static> BoxHandler<STRUCT, Request, Response> {
    #[allow(dead_code)]
    pub fn new<H>(inner: H) -> Self
    where
        H: Handler<STRUCT, Request, Response = Response> + Send + 'static,
        H::Future: Send + 'static,
    {
        let inner = Box::new(inner.map_future(|f: H::Future| Box::pin(f) as _));
        BoxHandler { inner }
    }
}

impl<STRUCT, Request, Response> Handler<STRUCT, Request> for BoxHandler<STRUCT, Request, Response> {
    type Response = Response;
    type Future = BoxFuture<Response>;
    fn call(&mut self, s: STRUCT, ctx: Context, request: Request) -> Self::Future {
        self.inner.call(s, ctx, request)
    }
}

// === for multi thread
//
// ported from https://github.com/tower-rs/tower/pull/615/files
pub type ABoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>; // box future with lifetime parameter a'

pub struct BoxCloneHandler<S, R, U>(
    Box<dyn CloneHandler<S, R, Response = U, Future = ABoxFuture<'static, U>> + Send>,
);

impl<STRUCT, Request, Response> BoxCloneHandler<STRUCT, Request, Response> {
    pub fn new<S>(inner: S) -> Self
    where
        S: Handler<STRUCT, Request, Response = Response> + Clone + Send + 'static,
        S::Future: Send + 'static,
    {
        let inner = inner.map_future(|f| Box::pin(f) as _);
        BoxCloneHandler(Box::new(inner))
    }
}

impl<S, R, U> Handler<S, R> for BoxCloneHandler<S, R, U> {
    type Response = U;

    type Future = ABoxFuture<'static, U>;

    fn call(&mut self, s: S, ctx: Context, r: R) -> Self::Future {
        self.0.call(s, ctx, r)
    }
}

impl<STRUCT, Request, Response> Clone for BoxCloneHandler<STRUCT, Request, Response> {
    fn clone(&self) -> Self {
        Self(self.0.clone_box())
    }
}

trait CloneHandler<S, R>: Handler<S, R> {
    fn clone_box(
        &self,
    ) -> Box<dyn CloneHandler<S, R, Response = Self::Response, Future = Self::Future> + Send>;
}

impl<STRUCT, Request, T> CloneHandler<STRUCT, Request> for T
where
    T: Handler<STRUCT, Request> + Send + Clone + 'static,
{
    fn clone_box(
        &self,
    ) -> Box<dyn CloneHandler<STRUCT, Request, Response = T::Response, Future = T::Future> + Send>
    {
        Box::new(self.clone())
    }
}
