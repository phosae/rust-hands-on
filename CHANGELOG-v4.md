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
An endpoint `GET /test/sleep/:duration` have been added to `Svc` to help tiggering timeout. The origin `Service Trait Object` have been wrap as `Timeout Trait Object`
```rust
let svc = middleware::timeout::Timeout::new(svc, std::time::Duration::from_secs(3));
```

See full change at commit [todo!]()

[building-a-middleware-from-scratch]: https://github.com/tower-rs/tower/blob/74881d531141ba0f07b7f58e2a72e3594e5a665c/guides/building-a-middleware-from-scratch.md