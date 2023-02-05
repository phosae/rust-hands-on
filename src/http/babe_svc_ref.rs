use std::{future::Future, pin::Pin};

#[derive(Clone, Debug)]
struct Svc;

#[allow(dead_code)]
impl Svc {
    async fn a(&self) -> Vec<String> {
        vec!["a".to_owned()]
    }

    fn b<'a>(&'a self) -> impl Future<Output = Vec<String>> + 'a {
        async move {
            print!("{:?}", self);
            vec!["b".to_owned()]
        }
    }
}

trait SvcHandler<'a> {
    type Ret;
    type Future: Future<Output = Self::Ret> + 'a;
    fn call(&'a mut self, s: &'a Svc) -> Self::Future;
}

#[derive(Copy, Clone)]
struct SvcFn<F>(F);

#[allow(dead_code)]
fn svc_fn<F>(f: F) -> SvcFn<F> {
    SvcFn(f)
}

impl<'a, F, Fut, T> SvcHandler<'a> for SvcFn<F>
where
    F: FnMut(&'a Svc) -> Fut,
    Fut: Future<Output = T> + 'a,
{
    type Ret = T;
    type Future = Fut;
    fn call(&'a mut self, s: &'a Svc) -> Self::Future {
        (self.0)(s)
    }
}

trait SvcHanlderExt<'a>: SvcHandler<'a> {
    fn map_future<F, Fut, T>(self, f: F) -> MapFuture<Self, F>
    where
        Self: Sized,
        F: FnMut(<Self as SvcHandler<'a>>::Future) -> Fut,
        Fut: Future<Output = T>,
    {
        MapFuture::new(self, f)
    }

    fn boxed(self) -> BoxSvcHanlder<'a, Self::Ret>
    where
        Self: Sized + Send + 'static,
        Self::Future: Send + 'static,
    {
        BoxSvcHanlder::new(self)
    }
}

impl<'a, T: ?Sized + SvcHandler<'a>> SvcHanlderExt<'a> for T {}

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

impl<'a, S, F, Fut, T> SvcHandler<'a> for MapFuture<S, F>
where
    S: SvcHandler<'a>,
    F: FnMut(S::Future) -> Fut,
    Fut: Future<Output = T> + 'a,
{
    type Ret = T;
    type Future = Fut;

    fn call(&'a mut self, s: &'a Svc) -> Self::Future {
        (self.f)(self.inner.call(s))
    }
}

#[allow(dead_code)]
type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;

#[allow(dead_code)]
struct BoxSvcHanlder<'a, T> {
    inner: Box<dyn SvcHandler<'a, Ret = T, Future = BoxFuture<T>> + Send + 'a>,
}

impl<'a, T: 'a> BoxSvcHanlder<'a, T> {
    #[allow(dead_code)]
    pub fn new<SH>(inner: SH) -> Self
    where
        SH: SvcHandler<'a, Ret = T> + Send + 'static,
        SH::Future: Send + 'static,
    {
        let inner = Box::new(inner.map_future(|f: SH::Future| Box::pin(f) as _));
        BoxSvcHanlder { inner }
    }
}

impl<'a, T: 'a> SvcHandler<'a> for BoxSvcHanlder<'a, T> {
    type Ret = T;
    type Future = BoxFuture<T>;
    fn call(&'a mut self, s: &'a Svc) -> BoxFuture<T> {
        self.inner.call(s)
    }
}

#[test]
fn test_box_svc() {
    let _svc = Svc {};

    let ha1 = BoxSvcHanlder::new(svc_fn(Svc::a));
    let ha2 = svc_fn(Svc::b).boxed();
    let ha3 = svc_fn(Svc::b).boxed();
    let ha4 = svc_fn(|_: &Svc| async { vec!["9".to_owned(), "10".to_owned()] }).boxed();

    let mut router = matchit::Router::new();
    router
        .insert("/box/a/cars/:car_id/regions/:region", ha1)
        .unwrap();
    router.insert("/a/boxed/regions/:region", ha2).unwrap();
    router.insert("/b/boxed/cars/:car_id", ha3).unwrap();
    router.insert("/closore_boxed/cars/:car_id", ha4).unwrap();
}
