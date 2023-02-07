# a demo wasi/wasm32 HTTP server runs on [WasmEdge](https://github.com/WasmEdge/WasmEdge) host
## play
build
```
docker buildx build --platform wasi/wasm32 -f Dockerfile.wasmedge -t zengxu/car-demo-wasm .
```
run
```
docker run --rm -dp 9100:9100 \
  --runtime=io.containerd.wasmedge.v1 \
  --platform=wasi/wasm32 \
  zengxu/car-demo-wasm
```
## test
apis
- GET /cars = to fetch all cars
- GET /cars/:id = to fetch a specific car
- POST /cars = to create a new car, `curl -XPOST 127.1:9100/cars -d '{"brand":"BYD","model":"Han","year":2023}'`

## important lib
[WasmEdge features](https://wasmedge.org/book/en/features/proposals.html), espcially Sockets
[tokio_wasi](https://github.com/WasmEdge/tokio),
[hyper_wasi](https://github.com/WasmEdge/hyper)

## [TODO]
- WasmEdge Redis
- WasmEdge MySQL

Further Study
- [wasmedge-db-examples](https://github.com/WasmEdge/wasmedge-db-examples)
- [microservice-rust-mysql](https://github.com/second-state/microservice-rust-mysql)
- [WasmEdge/redis-rs](https://github.com/WasmEdge/redis-rs) support is on the way
- [wasmedge-anna-client](https://github.com/WasmEdge/wasmedge-anna-client)

As show in [Spin Wit](https://github.com/fermyon/spin/tree/main/wit/ephemeral), how Spin Implement its Sockets in WASI? As it based on [bytecodealliance/wasmtime](https://github.com/bytecodealliance/wasmtime), how Wasmtime implement it?

The main projects used in creating the [Fermyon Platform](https://www.fermyon.dev/open-source):
- [bytecodealliance/wasmtime](https://github.com/bytecodealliance/wasmtime)
- [deislabs/bindle](https://github.com/deislabs/bindle)
- [deislabs/hippo](https://github.com/deislabs/hippo)
- hashicorp/consul, hashicorp/nomad, hashicorp/terraform, traefik/traefik

Integration with container orchestration systems, such as K8s, Nomad...