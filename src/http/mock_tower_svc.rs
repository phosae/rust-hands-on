use std::{future::Future, pin::Pin};

#[derive(Clone, Debug)]
struct Svc;

impl Svc {
    #[allow(dead_code)]
    // desugar as fn -> impl Future
    // fn a<'a>(&'a mut self) -> impl Future<Output = Vec<String>> + 'a {
    //     async move { vec!["a".to_owned()] }
    // }
    async fn a(self) -> Vec<String> {
        vec!["a".to_owned()]
    }

    #[allow(dead_code)]
    async fn aa(self) -> Vec<String> {
        vec!["aa".to_owned()]
    }

    // two fn b(&Svc)-> impl Future<Output = Vec<String>> are different different opaque types
    #[allow(dead_code)]
    fn b<'a>(self) -> impl Future<Output = Vec<String>> + 'a {
        async move {
            print!("{:?}", self);
            vec!["b".to_owned()]
        }
    }

    #[allow(dead_code)]
    fn box_a(self) -> Pin<Box<dyn Future<Output = Vec<String>>>> {
        Box::pin(self.a())
    }
    #[allow(dead_code)]
    fn box_aa(self) -> Pin<Box<dyn Future<Output = Vec<String>>>> {
        Box::pin(self.aa())
    }
}

//#[derive(Copy, Clone)]
struct SvcFn<F>(F);

#[allow(dead_code)]
fn svc_fn<F>(f: F) -> SvcFn<F> {
    SvcFn(f)
}

impl<F, Fut, T> SvcHandler for SvcFn<F>
where
    F: FnMut(Svc) -> Fut,
    Fut: Future<Output = T> + 'static,
{
    type Ret = T;
    type Future = Fut;
    fn call(&mut self, s: Svc) -> Self::Future {
        (self.0)(s)
    }
}

trait SvcHandler {
    type Ret;
    type Future: Future<Output = Self::Ret>;
    fn call(&mut self, s: Svc) -> Self::Future;
}

// #[test]
// fn test_svc_fn_notpass() {
//     let ha1: Box<dyn SvcHandler<Ret = Vec<String>, Future = _>> = Box::new(svc_fn(Svc::a));
//     let ha2: Box<dyn SvcHandler<Ret = Vec<String>, Future = _>> = Box::new(svc_fn(Svc::aa));
//     let mut router = matchit::Router::new();
//     router.insert("/v1/images", ha1).unwrap();
//     router.insert("/v1/images/push", ha2).unwrap();
// }

#[test]
fn test_svc_fn() {
    let box_ha1: Box<dyn SvcHandler<Ret = Vec<String>, Future = _>> = Box::new(svc_fn(Svc::box_a));
    let box_ha2: Box<dyn SvcHandler<Ret = Vec<String>, Future = _>> = Box::new(svc_fn(Svc::box_aa));
    let mut router = matchit::Router::new();
    router.insert("/box_a", box_ha1).unwrap();
    router.insert("/box_aa", box_ha2).unwrap();
}

trait SvcHanlderExt: SvcHandler {
    fn map_future<F, Fut, T>(self, f: F) -> MapFuture<Self, F>
    where
        Self: Sized,
        F: FnMut(Self::Future) -> Fut,
        Fut: Future<Output = T>,
    {
        MapFuture::new(self, f)
    }

    fn boxed(self) -> BoxSvcHanlder<Self::Ret>
    where
        Self: Sized + Send + 'static,
        Self::Future: Send + 'static,
    {
        BoxSvcHanlder::new(self)
    }
}

impl<T: ?Sized> SvcHanlderExt for T where T: SvcHandler {}

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

impl<S, F, Fut, T> SvcHandler for MapFuture<S, F>
where
    S: SvcHandler,
    F: FnMut(S::Future) -> Fut,
    Fut: Future<Output = T>,
{
    type Ret = T;
    type Future = Fut;

    fn call(&mut self, s: Svc) -> Self::Future {
        (self.f)(self.inner.call(s))
    }
}

// #[test]
// fn test_map_fut_failed() {
//     let ha1 =
//         svc_fn(Svc::a).map_future(|f: impl Future<Output = Vec<String>>| Box::pin(f) as _);
// }

#[allow(dead_code)]
type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;

#[allow(dead_code)]
struct BoxSvcHanlder<T> {
    inner: Box<dyn SvcHandler<Ret = T, Future = BoxFuture<T>> + Send>,
}

impl<T> BoxSvcHanlder<T> {
    #[allow(dead_code)]
    pub fn new<SH>(inner: SH) -> Self
    where
        SH: SvcHandler<Ret = T> + Send + 'static,
        SH::Future: Send + 'static,
    {
        let inner = Box::new(inner.map_future(|f: SH::Future| Box::pin(f) as _));
        BoxSvcHanlder { inner }
    }
}

impl<T> SvcHandler for BoxSvcHanlder<T> {
    type Ret = T;
    type Future = BoxFuture<T>;
    fn call(&mut self, s: Svc) -> BoxFuture<T> {
        self.inner.call(s)
    }
}

#[test]
fn test_box_svc() {
    let _svc = Svc {};

    let ha1 = BoxSvcHanlder::new(svc_fn(Svc::a));
    let ha2 = svc_fn(Svc::a).boxed();

    let ha3 = svc_fn(Svc::b).boxed();
    let ha4 = svc_fn(|_: Svc| async { vec!["9".to_owned(), "10".to_owned()] }).boxed();
    let mut router = matchit::Router::new();
    router
        .insert("/box/a/cars/:car_id/regions/:region", ha1)
        .unwrap();
    router.insert("/a/boxed/regions/:region", ha2).unwrap();
    router.insert("/b/boxed/cars/:car_id", ha3).unwrap();
    router.insert("/closore_boxed/cars/:car_id", ha4).unwrap();

    match router.at("/box/a/cars/15/regions/cn") {
        Ok(m) => {
            for p in m.params.iter() {
                println!("k:{},v:{}", p.0, p.1)
            }
        }
        Err(e) => eprintln!("route match err {}", e),
    }
}
