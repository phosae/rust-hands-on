use std::{future::Future, pin::Pin};

#[derive(Clone)]
pub struct Context {
    pub vars: std::collections::HashMap<String, String>,
}

pub trait Handler<'a, STRUCT, Request> {
    type Response;
    type Future: Future<Output = Self::Response> + 'a;
    fn call(&'a mut self, s: &'a STRUCT, ctx: Context, r: Request) -> Self::Future;
}

#[derive(Copy, Clone)]
pub struct HandlerFn<F>(F);

#[allow(dead_code)]
pub fn handler_fn<F>(f: F) -> HandlerFn<F> {
    HandlerFn(f)
}

impl<'a, F, Fut, STRUCT: 'a, Request, Response> Handler<'a, STRUCT, Request> for HandlerFn<F>
where
    F: FnMut(&'a STRUCT, Context, Request) -> Fut,
    Fut: Future<Output = Response> + 'a,
{
    type Response = Response;
    type Future = Fut;
    fn call(&'a mut self, s: &'a STRUCT, ctx: Context, r: Request) -> Self::Future {
        (self.0)(s, ctx, r)
    }
}

trait HandlerExt<'a, STRUCT: 'a, Request: 'a>: Handler<'a, STRUCT, Request> {
    fn map_future<F, Fut, T>(self, f: F) -> MapFuture<Self, F>
    where
        Self: Sized,
        F: FnMut(<Self as Handler<'a, STRUCT, Request>>::Future) -> Fut,
        Fut: Future<Output = T>,
    {
        MapFuture::new(self, f)
    }

    fn boxed(self) -> BoxHandler<'a, STRUCT, Request, Self::Response>
    where
        Self: Sized + Send + 'static,
        Self::Future: Send + 'static,
    {
        BoxHandler::new(self)
    }
}

impl<'a, STRUCT: 'a, Request: 'a, T: ?Sized + Handler<'a, STRUCT, Request>>
    HandlerExt<'a, STRUCT, Request> for T
{
}

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

impl<'a, S, F, Fut, STRUCT, Request, Reponse> Handler<'a, STRUCT, Request> for MapFuture<S, F>
where
    S: Handler<'a, STRUCT, Request>,
    F: FnMut(S::Future) -> Fut,
    Fut: Future<Output = Reponse> + 'a,
{
    type Response = Reponse;
    type Future = Fut;

    fn call(&'a mut self, s: &'a STRUCT, ctx: Context, r: Request) -> Self::Future {
        (self.f)(self.inner.call(s, ctx, r))
    }
}

#[allow(dead_code)]
type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;

#[allow(dead_code)]
pub struct BoxHandler<'a, STRUCT, Request, Response> {
    inner: Box<
        dyn Handler<'a, STRUCT, Request, Response = Response, Future = BoxFuture<Response>>
            + Send
            + 'a,
    >,
}

impl<'a, STRUCT: 'a, Request: 'a, Response: 'a> BoxHandler<'a, STRUCT, Request, Response> {
    #[allow(dead_code)]
    pub fn new<SH>(inner: SH) -> Self
    where
        SH: Handler<'a, STRUCT, Request, Response = Response> + Send + 'static,
        SH::Future: Send + 'static,
    {
        let inner = Box::new(inner.map_future(|f: SH::Future| Box::pin(f) as _));
        BoxHandler { inner }
    }
}

impl<'a, STRUCT: 'a, Request: 'a, Response: 'a> Handler<'a, STRUCT, Request>
    for BoxHandler<'a, STRUCT, Request, Response>
{
    type Response = Response;
    type Future = BoxFuture<Response>;
    fn call(&'a mut self, s: &'a STRUCT, ctx: Context, r: Request) -> BoxFuture<Response> {
        self.inner.call(s, ctx, r)
    }
}

// === for multi thread
//
// ported from https://github.com/tower-rs/tower/pull/615/files
pub struct CloneBoxHandler<'a, STRUCT, Request, Response>(
    Box<
        dyn CloneHandler<'a, STRUCT, Request, Response = Response, Future = BoxFuture<Response>>
            + Send,
    >,
);

impl<'a, STRUCT: 'a, Request: 'a, Response: 'a> CloneBoxHandler<'a, STRUCT, Request, Response> {
    #[allow(dead_code)]
    pub fn new<S>(inner: S) -> Self
    where
        S: Handler<'a, STRUCT, Request, Response = Response> + Clone + Send + 'static,
        S::Future: Send + 'static,
    {
        let inner = inner.map_future(|f| Box::pin(f) as _);
        CloneBoxHandler(Box::new(inner))
    }
}

pub type ABoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

impl<'a, STRUCT: 'a, Request: 'a, Response: 'a> Handler<'a, STRUCT, Request>
    for CloneBoxHandler<'a, STRUCT, Request, Response>
{
    type Response = Response;

    type Future = ABoxFuture<'a, Response>;

    fn call(&'a mut self, s: &'a STRUCT, ctx: Context, r: Request) -> Self::Future {
        self.0.call(s, ctx, r)
    }
}

impl<'a, STRUCT: 'a, Request: 'a, Response: 'a> Clone
    for CloneBoxHandler<'a, STRUCT, Request, Response>
{
    fn clone(&self) -> Self {
        Self(self.0.clone_box())
    }
}

trait CloneHandler<'a, STRUCT: 'a, Request: 'a>: Handler<'a, STRUCT, Request> {
    fn clone_box(
        &self,
    ) -> Box<
        dyn CloneHandler<'a, STRUCT, Request, Response = Self::Response, Future = Self::Future>
            + Send,
    >;
}

impl<'a, STRUCT: 'a, Request: 'a, T: 'a> CloneHandler<'a, STRUCT, Request> for T
where
    T: Handler<'a, STRUCT, Request> + Send + Clone + 'static,
{
    fn clone_box(
        &self,
    ) -> Box<dyn CloneHandler<'a, STRUCT, Request, Response = T::Response, Future = T::Future> + Send>
    {
        Box::new(self.clone())
    }
}
