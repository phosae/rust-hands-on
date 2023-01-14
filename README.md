# rewrite qappctl-shim in Rust
play with
```
cargo run
```
## Step 1: write a stateless car server
copy & paste codes 
- [arealesramirez/rust-rest-api-hyper](https://github.com/arealesramirez/rust-rest-api-hyper) for apis
    - GET /cars = to fetch all cars
    - GET /cars/:id = to fetch a specific car
    - POST /cars = to create a new car
- [hyper@1.0.0 examples](https://github.com/hyperium/hyper/tree/v1.0.0-rc.2/examples)
    - especially [web_api](https://github.com/hyperium/hyper/tree/v1.0.0-rc.2/examples/web_api.rs), 
    - learn some serde_json...
    - learn the type trick
      - `type GenericError = Box<dyn std::error::Error + Send + Sync>;`
      - `type Result<T> = std::result::Result<T, GenericError>;`
      - `type BoxBody = http_body_util::combinators::BoxBody<Bytes, hyper::Error>;`

## Step 2: store cars in memory
- see [CHANGELOG-v2](CHANGELOG-v2.md)

## [TODO]Step 3: do the true thing ———— rewrite [qappctl-shim](https://github.com/phosae/qappctl-shim)
1. do rewrite
2. dockerize building
3. use github actions to build images