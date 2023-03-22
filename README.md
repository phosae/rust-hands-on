# Hands-On Rust HTTP Server Based on Hyper 1.0+
play with
```
cargo run
```
## v1: write a stateless car server
copy & paste codes 
- [arealesramirez/rust-rest-api-hyper](https://github.com/arealesramirez/rust-rest-api-hyper) for apis
    - GET /cars to fetch all cars
    - GET /cars/:id = to fetch a specific car
    - POST /cars = to create a new car
- [hyper@1.0.0 examples](https://github.com/hyperium/hyper/tree/v1.0.0-rc.2/examples)
    - especially [web_api](https://github.com/hyperium/hyper/tree/v1.0.0-rc.2/examples/web_api.rs), 
    - learn some serde_json...
    - learn the type trick
      - `type GenericError = Box<dyn std::error::Error + Send + Sync>;`
      - `type Result<T> = std::result::Result<T, GenericError>;`
      - `type BoxBody = http_body_util::combinators::BoxBody<Bytes, hyper::Error>;`

## v2: store cars in memory
- POST    /cars
- PUT     /cars/{id}
- GET     /cars, /cars/{id}
- DELETE  /cars, /cars/{id}
- generate cars 🚀🚀🚀 `hey -z 10s -cpus 4 -c 4 -d '{"brand":"Tesla", "model": "Y", "year": 2023}' -m POST http://127.0.0.1:9100/cars` 
- see [CHANGELOG-v2](CHANGELOG-v2.md)

## v3

### sqlite carstore
store cars in sqlite when run with env `DB_TYPE=sqlite`

### ctl
wrap some qappctl command as HTTP service in path `/ctl/**`. Some thing just like [qappctl-shim](https://github.com/phosae/qappctl-shim)

### HTTP router: mock Golang HTTP Handler interface
register routes like Golang with [ibraheemdev/matchit](https://github.com/ibraheemdev/matchit) and our Handler implementation
```go
// Go
router := mux.NewRouter()
router.HandleFunc("/images", server.listImagesHandler).Methods("GET")
router.HandleFunc("/images", server.pushImageHandler).Methods("POST")
// Rust
let mut mux: HashMap<Method, matchit::Router<HandlerFn>> = Router::new();
add_route(&mut mux, "/ctl/images", Method::GET, BoxCloneHandler::new(handler_fn(Svc::list_images)));
add_route(&mut mux, "/ctl/images", Method::POST, BoxCloneHandler::new(handler_fn(Svc::push_image)));
```
See [CHANGELOG-v3](CHANGELOG-v3.md)

## v4
### HTTP Service middleware
- GET /test/sleep/:duration to trigger a timeout
- add timeout middleware, `curl -i 127.1:9100/test/sleep/3100`
- add log middleware
- add auth middleware, `curl -i 127.1:9100/test/sleep/3100 -H 'Bearer: zenx'`
- add MapResponse middleware
- add error_handling middleware

See [CHANGELOG-v4](CHANGELOG-v4.md)

### WebAssembly
add a demo wasi/wasm32 HTTP server, [car-demo](./car-demo-wasm/), based on v2

## v5 (doing)
- replace qappctl command with docker, make path `/ctl/**` more common

## [TODO]
- layerize middlewares
- all HTTP handler return Serializable T directly
- containerize binary as image and use github actions to build images
- support GraphQL (Optional)
- OpenAPI and Swagger (Optional)
  * code generator (maybe)
  * OpenAPI Specification from code comments
  * See [openapi-generator official](github.com/OpenAPITools/openapi-generator), [juhaku/utoipa](https://github.com/juhaku/utoipa), [paperclip-rs/paperclip](https://github.com/paperclip-rs/paperclip), [glademiller/openapiv3](https://github.com/glademiller/openapiv3)