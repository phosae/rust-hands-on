# rewrite qappctl-shim in Rust
play with
```
cargo run
```
## v1: write a stateless car server
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
see [CHANGELOG-v3](CHANGELOG-v3.md)

## [TODO]
- containerize binary as image and use github actions to build images
- integrate with [tower middleware](github.com/tower-rs/tower)
- Authentication(base on tower middleware) and GraphQL (Optional)
- OpenAPI and Swagger (Optional)
  * code generator (maybe)
  * OpenAPI Specification from code comments
  * See [openapi-generator official](github.com/OpenAPITools/openapi-generator), [juhaku/utoipa](https://github.com/juhaku/utoipa), [paperclip-rs/paperclip](https://github.com/paperclip-rs/paperclip), [glademiller/openapiv3](https://github.com/glademiller/openapiv3)