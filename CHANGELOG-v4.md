# v4
## timeout middleware
following [building-a-middleware-from-scratch], the middleware mod was added, with a implementation of timeout logic.

`pub struct Timeout<S>` is just a implemetation of `hyper::service::Service Trait` with a chained `hyper::service::Service Trait Object` in struct.

```rust
pub struct Timeout<S> {
    inner: S,
    timeout: Duration,
}

impl<S, Request> hyper::service::Service<Request> for Timeout<S>
where
    S: hyper::service::Service<Request>,
    S::Error: Into<BoxError>,
{
    type Response = S::Response;
    type Error = BoxError;
    type Future = ResponseFuture<S::Future>;

    fn call(&mut self, req: Request) -> Self::Future {
        let response_future = self.inner.call(req);
        let sleep = tokio::time::sleep(self.timeout);

        ResponseFuture {
            response_future,
            sleep,
        }
    }
}
```
The real timeout job is done by ResponseFuture's `Future Trait` implementation: once the inner service timeout, return an TimeoutError.
```rust
fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
    let this = self.project();
    match this.response_future.poll(cx) {
        Poll::Ready(result) => {
            let result = result.map_err(Into::into);
            return Poll::Ready(result);
        }
        Poll::Pending => {}
    }

    match this.sleep.poll(cx) {
        Poll::Ready(()) => {
            // Construct and return a timeout error.
            let error = Box::new(TimeoutError(()));
            return Poll::Ready(Err(error));
        }
        Poll::Pending => {}
    }

    Poll::Pending
}
```
An endpoint `GET /test/sleep/:duration` have been added to `Svc` to help tiggering timeout. The origin `Service Trait Object` have been wrap as `Timeout Trait Object`. Any HTTP request takes more than 3 seconds will be terminated. 
```rust
let svc = middleware::timeout::Timeout::new(svc, std::time::Duration::from_secs(3));
```
See full change at [commit](https://github.com/phosae/qappctl-shim-rs/commit/1725e9c5695f58bc9b7d63af2d7a35265465dc71)

Finally it works.
```
## sleep 1s, got 200
curl -i http://127.0.0.1:9100/test/sleep/1000
HTTP/1.1 200 OK
content-type: application/json
content-length: 4
date: Sat, 04 Feb 2023 00:59:35 GMT

"{}"

## sleep 3.1s, got an empty reply
curl -i http://127.0.0.1:9100/test/sleep/3100
curl: (52) Empty reply from server
```

Once a timeout reached, the server close the connection and client got an empty reply. Full code is [here](./src/middleware/timeout/mod.rs)

It's better to return some HTTP StatusCode, such as `408 Request Timeout` or `503 Service Unavailable`. It willl be covered in `error_handling middleware`.

## log middleware
As `timeout middleware` is a copy/paste implementation, It's time to implement the simpler `log middleware`, which log the time cost of each request.

Put Future return by inner Service in async closure, save the start time before of it, await the Future response, got the time cost and log it —— seems great.

```rust
#[derive(Debug, Clone)]
struct FailureLogRequest<S> {
    inner: S,
}

impl<S, ReqBody> Service<Request<ReqBody>> for FailureLogRequest<S>
where
    S: Service<Request<ReqBody>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = ResponseFuture<S::Future>;

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        async {
            let start = std::time::Instant::now();
            let response = self.inner.call(req).await;
            println!("elapsed: {:.2?} ms", start.elapsed().as_millis());
            response
        }
    }
}
```
But this halted at make it to compile
```
error[E0308]: mismatched types
   --> src/middleware/log/mod.rs:90:9
    |
90  | /         async {
91  | |             let start = std::time::Instant::now();
92  | |             let response = self.inner.call(req).await;
93  | |             println!("elapsed: {:.2?} ms", start.elapsed().as_millis());
94  | |             response
95  | |         }
    | |         ^
    | |         |
    | |_________expected struct `ResponseFuture`, found `async` block
    |           arguments to this function are incorrect
    |
    = note:     expected struct `middleware::log::ResponseFuture<<S as hyper::service::Service<Request<ReqBody>>>::Future>`
            found `async` block `[async block@src/middleware/log/mod.rs:90:9: 95:10]`
```
After read some middleware code of  [tower-rs/tower], [tower-rs/tower-http] and [tokio-rs/axum], I found that the solution have been already given by `timeout middleware` code. That is, in Service call it returns an custom `ResponseFuture`. The `ResponseFuture` carry the inner `response_future`, with neccessary field set from `Request`(here is start time and request info). In `ResponseFuture`'s Future poll function, it returns `Poll::Pending` when the inner is pending and log the time cost of request when the inner reponse_future is `Poll::Ready`.

```rust
use std::{pin::Pin, task::Poll, time::Instant};
use hyper::service::Service;
use hyper::Request;
use pin_project_lite::pin_project;
use std::future::Future;

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
            Poll::Pending =>  Poll::Pending,
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

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
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
```
It works.
```
# cargo run
Listening on http://127.0.0.1:9100
request method=GET, uri=/test/sleep/3100, version=HTTP/1.1, elapsed: 3001 ms
```
Remembering the reading of [Programming Rust 2nd Edition]'s chapter 20 `Asynchronous Programming`, it point out that
```
In a synchronous function, all local variables live on the stack, but in an asynchronous function, local variables that are alive across an await must be located in the future, so they’ll be available when it is polled again. Borrowing a reference to such a variable borrows a part of the future.

Rust requires that values not be moved while they are borrowed. ... The borrow checker treats variables as the roots of ownership trees, but unlike variables stored on the stack, variables stored in futures get moved if the future itself moves. ... Futures of async functions are a blind spot for the borrow checker, which Rust must cover somehow if it wants to keep its memory safety promises.

Rust’s solution to this problem rests on the insight that futures are always safe to move when they are first created, and only become unsafe to move when they are polled. A future that has just been created by calling an asynchronous function simply holds a resumption point and the argument values. These are only in scope for the asynchronous function’s body, which has not yet begun execution. Only polling a future can borrow its contents.

This, then, is Rust’s strategy for keeping futures safe: a future can’t become dangerous to move until it’s polled; you can’t poll a future until you’ve constructed a Pin-wrapped pointer to it; and once you’ve done that, the future can’t be moved.
```
The crate [pin-project] make handling Pin/Unpin field in struct right and easy:

```rust
let this = self.project();
match this.response_future.poll(cx) {
    Poll::Ready(result) => {
        println!(
            "{}, elapsed: {:.2?} ms",
            this.reqinfo, // self.reqinfo,
            this.start.elapsed().as_millis()
        );
        return Poll::Ready(result);
    }
    Poll::Pending => Poll::Pending,
}
```
Using reqinfo field by `self.reqinfo` will get complain
```
borrow of moved value: `self`
borrow occurs due to deref coercion to `middleware::log::ResponseFuture<F>`
```

## auth middleware, middleware util
The `auth middleware` take a function to consume `&Request` for authorization. If passed, call the inner Service. Otherwise end the request by returning `401  UNAUTHORIZED`. Like `timeout middleware` and `log middleware`, things got done in a custom `ResponseFuture`, too. Any HTTP request with `Bearer` header without value `zenx` will be rejected.

```
curl -i 127.1:9100/test/sleep/3100 -H 'Bearer: notzenx'
HTTP/1.1 401 Unauthorized
server-owner: zenx
content-length: 0
date: Sun, 05 Feb 2023 09:33:58 GMT
```
Full code is [here](./src/middleware/auth/async_require_authorization.rs)

Sometimes, user needs to alter the Response returned by http Handler, for example, in `main.rs` we add a custom HTTP Header `Server-Owner: zenx` for each HTTP Response, `util::MapResponse` helps this. 

```
curl -i 127.1:9100/test/sleep/3
HTTP/1.1 200 OK
content-type: application/json
server-owner: zenx
content-length: 4
date: Sun, 05 Feb 2023 09:34:28 GMT

"{}"
```
Full code is [here](./src/middleware/util/mod.rs)

 In [tokio-rs/axum middleware], there're `MapRequest middleware` and `MapResponse middleware` implemented in similar way.

## error_handling middleware (transform Error to Response)
Inspired by the [tokio-rs/axum's key-value-store example], error throwed by middleware (such as timeout middleware), or HTTP Request parse error(such as hyper::Error), can be transform to user friendly HTTP Response.

The error_handling middleware will detect error returned by the inner Service, and transform it to Response. Full code is [here](./src/middleware/error_handling/mod.rs) and most of it are ported from [tokio-rs/axum]'s [error_handling](https://github.com/tokio-rs/axum/blob/19596584dae8ec6fc733d47dcdd1d874c52d484a/axum/src/error_handling/mod.rs) and [into_response](https://github.com/tokio-rs/axum/blob/19596584dae8ec6fc733d47dcdd1d874c52d484a/axum-core/src/response/into_response.rs).

At this moment I still can't figure out why there's a `Oneshot struct`. 

[building-a-middleware-from-scratch]: https://github.com/tower-rs/tower/blob/74881d531141ba0f07b7f58e2a72e3594e5a665c/guides/building-a-middleware-from-scratch.md
[tower-rs/tower]: https://github.com/tower-rs/tower
[tower-rs/tower-http]: https://github.com/tower-rs/tower-http
[tokio-rs/axum]: https://github.com/tokio-rs/axum
[tokio-rs/axum middleware]: https://github.com/tokio-rs/axum/tree/main/axum/src/middleware
[tokio-rs/axum's key-value-store example]: https://github.com/tokio-rs/axum/tree/main/examples/key-value-store
[Programming Rust 2nd Edition]: https://www.oreilly.com/library/view/programming-rust-2nd/9781492052586
[pin-project]: https://crates.io/crates/pin-project