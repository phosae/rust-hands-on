# v4

## timeout middleware based on `hyper::service::Service Trait`
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

Once a timeout reached, the server close the connection and client got an empty reply. It's better to return some HTTP StatusCode, such as `408 Request Timeout` or `503 Service Unavailable`.

Implementing a wrapper at head of `Service Trait` to do Err Translation is a good idea, but this halted at make it compile

```rust
pub struct TopMiddleware<S> {
    inner: S,
}

impl<S, Request> hyper::service::Service<Request> for Middleware<S>
where
    S: hyper::service::Service<Request>,
    //S::Error: Into<BoxError>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn call(&mut self, req: Request) -> Self::Future {
        let fut = self.inner.call(req);
        return async move {
            match fut.await {
                Ok(ret) => Ok(ret),
                Err(e) => {
                    if util::type_of(e) == "TimeoutError" {
                        Ok(util::mk_err_response(
                            hyper::StatusCode::REQUEST_TIMEOUT,
                            "",
                        ))
                    } else {
                        Err(e)
                    }
                }
            }
        };
    }
}
```
Got complain
```
error[E0308]: mismatched types
   --> src/middleware/mod.rs:25:16
    |
25  |           return async move {
    |  ________________-
26  | |             match fut.await {
27  | |                 Ok(ret) => Ok(ret),
28  | |                 Err(e) => {
...   |
38  | |             }
39  | |         };
    | |         ^
    | |         |
    | |_________expected associated type, found `async` block
    |           arguments to this function are incorrect
    |
    = note: expected associated type `<S as Service<Request>>::Future`
                 found `async` block `[async block@src/middleware/mod.rs:25:16: 39:10]`
    = help: consider constraining the associated type `<S as Service<Request>>::Future` to `[async block@src/middleware/mod.rs:25:16: 39:10]` or calling a method that returns `<S as Service<Request>>::Future`
    = note: for more information, visit https://doc.rust-lang.org/book/ch19-03-advanced-traits.html
```
It's time to read [tower-rs/tower] and [tokio-rs/axum] source code and find some good solution.

[building-a-middleware-from-scratch]: https://github.com/tower-rs/tower/blob/74881d531141ba0f07b7f58e2a72e3594e5a665c/guides/building-a-middleware-from-scratch.md
[tower-rs/tower]: https://github.com/tower-rs/tower
[tokio-rs/axum]: https://github.com/tokio-rs/axum