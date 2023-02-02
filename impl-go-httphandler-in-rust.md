# How to implement Golang HTTP Handler interface like Rust Trait for router
The goal: registering routes like Golang with [ibraheemdev/matchit](https://github.com/ibraheemdev/matchit) and our Handler implementation

As an HTTP server become larger, it's time to import an HTTP multiplexer (akka. service dispather). The multiplexer can make route dispather faster, parse URL path paramether in one place, and provide more readable route information. Code comparasion are at commit [route all by mux](https://github.com/phosae/qappctl-shim-rs/commit/c6516cf96115920d8ef29aed8050a57b63299363).

In Golang, with help of standard library `net/http` or 3rd library like [gorilla/mux](https://github.com/gorilla/mux), we can register HTTP routes like this:
```go
router := mux.NewRouter()
router.HandleFunc("/images", server.listImagesHandler).Methods("GET")
router.HandleFunc("/images", server.pushImageHandler).Methods("POST")
```
The Go standard library's HTTP server do low level things in proto and take a Handler interface for incoming request handling
```go
type Handler interface {
	ServeHTTP(ResponseWriter, *Request)
}
```
A HTTP multiplexer is just a Handler that use list/map/tree to holds routes and Handles and delegate requests to matching Handler.[[1]] A helper function called `http.HandleFunc` in `net/http` turns any Go function with signature `func(w http.ResponseWriter, req *http.Request)` into Handler interface, and then it call be registered to multiplexer.

In Rust, as we build our HTTP server on top of [hyper](https://github.com/hyperium/hyper), which, similarly, take a Service Trait for incoming request handling

```rust
pub trait Service<Request> {
    /// Responses given by the service.
    type Response;
    /// Errors produced by the service.
    type Error;
    /// The future response value.
    type Future: Future<Output = Result<Self::Response, Self::Error>>;
    /// Process the request and return the response asynchronously.
    fn call(&mut self, req: Request) -> Self::Future;
}
```
The biggest differnece here is that the async function in Rust is implemented as Future Trait exposing to developer, while in Golang there's no difference in sync or async code(just some channel). Async code in Rust is more complicated and the way Rust managing memory make it even harder. We will see it later.

Since there's no default HTTP multiplexer in hyper, [ibraheemdev/matchit] will be used here. Thing left to us is implementing something like Go's `http.Handler` interface and `http.HandleFunc` in `net/http`. The first is definately, a Trait, for dynamic dispatching, and the latter will turn any async function with same signature into the same Trait.

The comming Trait implementation is inspire by tower's Service Trait[[2]].

## start from mocking tower's Service Trait
let's start from minimal.

Firstly, let's define a Trait `SvcHandler`, which simplely accept a `Svc` and return a future of any Generic Type.

```rust
trait SvcHandler {
    type Ret;
    type Future: Future<Output = Self::Ret>;
    fn call(&mut self, s: Svc) -> Self::Future;
}
```
The goal is turnning aysnc Svc method or function that accept Svc as argument and return any type into `SvcHandler` Trait.
```rust
impl Svc {
    async fn a(self) -> Vec<String> {
        vec!["a".to_owned()]
    }
}

async fn b(s: Svc) -> Vec<String> {
    async move {
        print!("{:?}", s);
        vec!["b".to_owned()]
    }
}
```
Then it's to write some helper function `svc_fn` that wrapping target function in struct `SvcFn`, and imlement the Trait for the struct.

```rust
struct SvcFn<F>(F);

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
```

Seemingly everything will be work like this (note `matchit::Router` is  [ibraheemdev/matchit]'s multiplexer implementation and  `matchit::Router::insert` is for route registering)

```rust
impl Svc {
    async fn a(self) -> Vec<String> {
        vec!["a".to_owned()]
    }
    async fn aa(self) -> Vec<String> {
        vec!["aa".to_owned()]
    }
}
#[test]
fn test_sv_fn() {
    let ha1 = svc_fn(Svc::a);
    let ha2 = svc_fn(Svc::aa);
    let mut router = matchit::Router::new();
    router.insert("/a", ha1).unwrap(); // 
    router.insert("/aa", ha2).unwrap();
}
```
But the compiler will complain about something like that 
```
|     router.insert("/v1/images/push", ha2).unwrap();
|            ------                    ^^^ expected fn item, found a different fn item
|            |
|            arguments to this function are incorrect
|
 = note: expected struct `mock_tower_svc::SvcFn<fn(mock_tower_svc::Svc) -> impl Future<Output = Vec<std::string::String>> {mock_tower_svc::Svc::a}>`
            found struct `mock_tower_svc::SvcFn<fn(mock_tower_svc::Svc) -> impl Future<Output = Vec<std::string::String>> {mock_tower_svc::Svc::aa}>`
```

It turns out that even async method `a` and `aa` have same signature but name, their types are different in rust (while in Golang their types seem no difference).

Ok let's try declare variable as Trait Object

```rust
#[test]
fn test_sv_fn() {
    let _ha1: dyn SvcHandler<Ret = Vec<String>, Future = _> = svc_fn(Svc::a);
    let _ha2: dyn SvcHandler<Ret = Vec<String>, Future = _> = svc_fn(Svc::aa);
}
```
We got new complain
```
the size for values of type `dyn mock_tower_svc::SvcHandler<Ret = Vec<std::string::String>, Future = _>` cannot be known at compilation time
```
So Box it and retry
```rust
#[test]
fn test_sv_fn() {
    let ha1: Box<dyn SvcHandler<Ret = Vec<String>, Future = _>> = Box::new(svc_fn(Svc::a));
    let ha2: Box<dyn SvcHandler<Ret = Vec<String>, Future = _>> = Box::new(svc_fn(Svc::aa));
    let mut router = matchit::Router::new();
    router.insert("/v1/images", ha1).unwrap();
    router.insert("/v1/images/push", ha2).unwrap();
}
```
At this time the compiler still complains 

```
--> src/util/mock_tower_svc.rs:72:38
   |
72 |     router.insert("/v1/images/push", ha2).unwrap();
   |            ------                    ^^^ expected opaque type, found a different opaque type
   |            |
   |            arguments to this function are incorrect
   |
note: while checking the return type of the `async fn`
  --> src/util/mock_tower_svc.rs:12:25
   |
12 |     async fn a(self) -> Vec<String> {
   |                         ^^^^^^^^^^^ checked the `Output` of this `async fn`, expected opaque type
note: while checking the return type of the `async fn`
  --> src/util/mock_tower_svc.rs:17:26
   |
17 |     async fn aa(self) -> Vec<String> {
   |                          ^^^^^^^^^^^ checked the `Output` of this `async fn`, found opaque type
   = note: expected struct `Box<dyn mock_tower_svc::SvcHandler<Ret = Vec<std::string::String>, Future = impl Future<Output = Vec<std::string::String>>>>` (opaque type at <src/util/mock_tower_svc.rs:12:25>)
              found struct `Box<dyn mock_tower_svc::SvcHandler<Ret = Vec<std::string::String>, Future = impl Future<Output = Vec<std::string::String>>>>` (opaque type at <src/util/mock_tower_svc.rs:17:26>)
   = note: distinct uses of `impl Trait` result in different opaque types
```
So I Googled 「distinct uses of `impl Trait` result in different opaque types」 and find this useful [discussion](https://users.rust-lang.org/t/error-distinct-uses-of-impl-trait-result-in-different-opaque-types/46862). Guys in this discussion pointed I should return a `Pin<Box<dyn Future<T>>>`. Then I add two methods for Svc and do tests
```rust
impl Svc {
    fn box_a(self) -> Pin<Box<dyn Future<Output = Vec<String>>>> {
        Box::pin(self.a())
    }
    fn box_aa(self) -> Pin<Box<dyn Future<Output = Vec<String>>>> {
        Box::pin(self.aa())
    }
}
#[test]
fn test_svc_fn() {
    let box_ha1: Box<dyn SvcHandler<Ret = Vec<String>, Future = _>> = Box::new(svc_fn(Svc::box_a));
    let box_ha2: Box<dyn SvcHandler<Ret = Vec<String>, Future = _>> = Box::new(svc_fn(Svc::box_aa));
    let mut router = matchit::Router::new();
    router.insert("/box_a", box_ha1).unwrap();
    router.insert("/box_aa", box_ha2).unwrap();
}
```
It works. The direction is clear: wrap the function from returning dynamic `impl Future<Output = Vec<std::string::String>>` to `Pin<Box<dyn Future<Output = Vec<String>>>>`.

After read the article [Inventing the Service trait] and the [tower-rs/tower] source code, especially [Service Trait](https://github.com/tower-rs/tower/blob/tower-0.4.13/tower-service/src/lib.rs), [ServiceExt Trait](https://github.com/tower-rs/tower/blob/tower-0.4.13/tower/src/util/mod.rs), [BoxService](https://github.com/tower-rs/tower/blob/tower-0.4.13/tower/src/util/boxed/sync.rs) and [BoxCloneService] parts, I find things can be done by adding  some helper function `map_future` for `SvcHandler Trait`. 

```rust
trait SvcHanlderExt: SvcHandler {
    fn map_future<F, Fut, T>(self, f: F) -> MapFuture<Self, F>
    where
        Self: Sized,
        F: FnMut(Self::Future) -> Fut,
        Fut: Future<Output = T>,
    {
        MapFuture::new(self, f)
    }
}

impl<T: ?Sized> SvcHanlderExt for T where T: SvcHandler {}
```
At same time, helper struct MapFuture is be imported.
```rust
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
```
Can everything work? 
```rust
fn test_map_fut_failed() {
    let _ha1 = svc_fn(Svc::a).map_future(|f: dyn Future<Output = Vec<String>>| Box::pin(f) as _);
}
```
The compiler complains 
```
type mismatch in closure arguments
expected closure signature `fn(impl Future<Output = Vec<std::string::String>>) -> _`
   found closure signature `fn((dyn Future<Output = Vec<std::string::String>> + 'static)) -> _`
```
But if we edit `dyn Future<Output = Vec<String>>` into `impl Future<Output = Vec<String>>`, it complains that 
```
`impl Trait` only allowed in function and inherent method return types, not in closure param`
```
How could I pass a function argument that the compiler don't even permit? So the later I find [tower-rs/tower] have this [trick in BoxService](https://github.com/tower-rs/tower/blob/04527aeb439761875a3e4f96d2090622731bc719/tower/src/util/boxed/sync.rs#L39).
```rust
pub struct BoxService<T, U, E> {
    inner: Box<dyn Service<T, Response = U, Error = E, Future = BoxFuture<U, E>> + Send>,
}

type BoxFuture<T, E> = Pin<Box<dyn Future<Output = Result<T, E>> + Send>>;

impl<T, U, E> BoxService<T, U, E> {
    pub fn new<S>(inner: S) -> Self
    where
        S: Service<T, Response = U, Error = E> + Send + 'static,
        S::Future: Send + 'static,
    {
        let inner = Box::new(inner.map_future(|f: S::Future| Box::pin(f) as _));
        BoxService { inner }
    }
}
```
The problem solved by add a helper struct `BoxSvcHanlder` which implement our `SvcHandler Trait`

```rust
type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;

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
```
and a convenient function `boxed` to SvcHanlderExt Trait
```rust
trait SvcHanlderExt: SvcHandler {
    ...

    fn boxed(self) -> BoxSvcHanlder<Self::Ret>
    where
        Self: Sized + Send + 'static,
        Self::Future: Send + 'static,
    {
        BoxSvcHanlder::new(self)
    }
}
```
Then we achieved the expected goal: turnning aysnc Svc method or function that accept Svc as argument and return any type into `SvcHandler` Trait.Full code can be found at [here](./src/util/mock_tower_svc.rs).

```rust
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
```
## make SvcHandler Trait generic
Based on previous toy `SvcHandler Trait`, let's make a more generic version. New goals this time are:
- import new argument `Context` to carry variables include URL path parameter or some other framework info. Though I know there's some thread-local solution in Rust, this time we just mimic Golang
- turn `Svc` to generic parameter `STRUCT`, so the handler can wrap from method for any struct
- make handler can be passed between different threads

```
pub struct Context {
    pub vars: std::collections::HashMap<String, String>,
}

pub trait Handler<STRUCT, Request> {
    type Response;
    type Future: Future<Output = Self::Response>;
    fn call(&mut self, s: STRUCT, ctx: Context, request: Request) -> Self::Future;
}
```

`SvcHandler Trait` is renamed to `Handler Trait`. The `handler_fn`, `HandlerExt`, `MapFuture`, `BoxHandler` are quite similar to previous. In order to make `Handler Trait` working fine in multi-thread context, something need to added. Since [tower-rs/tower]'s [BoxCloneService] is a `Clone + Send` boxed `Service`, just take the idea and implement ours (maybe just copy/paste :).

The core is make Handler Trait Clonable, it's done by implement the `CloneHandler Trait` for any Handler Trait object type
```rust
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
```
The rest is very similar to previous `BoxSvcHandler`, just map_future and Box::pin, but impl `Clone Trait`. Full code can be found at [here](./src/util/http.rs).

```rust
impl<STRUCT, Request, Response> Clone for BoxCloneHandler<STRUCT, Request, Response> {
    fn clone(&self) -> Self {
        Self(self.0.clone_box()) // delegate to the new CloneHandler feature
    }
}
```
## the finally
Finally we can do thing like this in Rust
``` Rust
let mut mux: HashMap<Method, matchit::Router<HandlerFn>> = Router::new();
add_route(&mut mux, "/ctl/images", Method::GET, BoxCloneHandler::new(handler_fn(Svc::list_images)));
add_route(&mut mux, "/ctl/images", Method::POST, BoxCloneHandler::new(handler_fn(Svc::push_image)));
```
## the suck thing: what about `pass by reference`
I once want to build `Hanle trait` [this way](./src/util/lifetime_handler_sucks.rs)

    trait Handler<'a, STRUCT, Request> {
        type Response;
        type Future: Future<Output = Self::Response> + 'a;
        fn call(&'a mut self, s: &'a STRUCT, ctx: Context, r: Request) -> Self::Future;
    }

because in Golang passing STRUCT by reference is so common, but in Rust, it sucks. When it comes to multi-thread and async context, as abstract  grows, write struct/trait/function signature is such a pain and pass compiling is so fucking hard. Trivial code are at [lifetime_handler_sucks.rs](./src/util/lifetime_handler_sucks.rs).

after read axum's [sqlx-postgres exmaple](https://github.com/tokio-rs/axum/tree/main/examples/sqlx-postgres) and read some state implemention code of axum, like

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

You guys would better remember that `pass by reference` in Rust is borrow, `pass by value` in Rust is move. It takes almost one week to figure out the why, `Chapter 4. Ownership and Moves` and `Chapter 5. References` in [Programming Rust 2nd Edition] really helps.

## Further Reading
1. https://eli.thegreenplace.net/2021/life-of-an-http-request-in-a-go-server/
2. [Inventing the Service trait](https://tokio.rs/blog/2021-05-14-inventing-the-service-trait)
3. [Programming Rust 2nd Edition](https://www.oreilly.com/library/view/programming-rust-2nd/9781492052586/)

[1]: https://eli.thegreenplace.net/2021/life-of-an-http-request-in-a-go-server
[2]: https://tokio.rs/blog/2021-05-14-inventing-the-service-trait
[Inventing the Service trait]: https://tokio.rs/blog/2021-05-14-inventing-the-service-trait
[Programming Rust 2nd Edition]: https://www.oreilly.com/library/view/programming-rust-2nd/9781492052586
[hyper]: https://github.com/hyperium/hyper
[ibraheemdev/matchit]: https://github.com/ibraheemdev/matchit
[tower-rs/tower]: https://github.com/tower-rs/tower
[BoxCloneService]: https://github.com/tower-rs/tower/blob/tower-0.4.13/tower/src/util/boxed_clone.rs