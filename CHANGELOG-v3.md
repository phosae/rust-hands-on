# v3
## sqlite carstore
store cars in sqlite when run with env `DB_TYPE=sqlite`

follow doc of [rusqlite]https://github.com/rusqlite/rusqlite and Googled [example](https://rust-lang-nursery.github.io/rust-cookbook/database/sqlite.html), cars in DB look easier than previous memory version. Any `Lock` is needless here, as SQLite will take care of multi-thread context.

## ctl
wrap some qappctl command as HTTP service in path `/ctl/**`. Some thing just like [qappctl-shim](https://github.com/phosae/qappctl-shim)

Executing commands in Rust is as easy as Golang, almost. The verbose part is serialization, while the concise part is error handling.

```rust
use bytes::Buf;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::result::Result;

fn run<T: Default + DeserializeOwned>(cmd: &mut Command, action: String) -> Result<T, String> {
    match cmd.output() {
        Err(e) => Err(format!("err output {}", e.to_string())),
        Ok(o) => {
            if o.status.success() {
                // action doesn't need stdout json decoding
                if std::any::type_name::<T>() == "()" {
                    return Ok(T::default());
                }
                let mut de = serde_json::Deserializer::from_reader(o.stdout.reader());
                match T::deserialize(&mut de) {
                    Ok(t) => Ok(t),
                    Err(e) => Err(format!("err parse std json: {}", e)),
                }
            } else {
                let stdout = String::from_utf8(o.stdout).unwrap();
                let stderr = String::from_utf8(o.stderr).unwrap();
                Err(format!("err {}: {}{}", action, stdout, stderr))
            }
        }
    }
}

pub fn list_images() -> Result<Vec<Image>, String> {
    let mut binding = Command::new("qappctl");
    let cmd = binding.arg("images").arg("-o").arg("json");

    Ok(run::<Vec<Image>>(cmd, "qappctl images".to_owned())?)
}
```

```go
import (
	"encoding/json"
	"fmt"
	"log"
	"os/exec"
	"strconv"
)

func run(cmd *exec.Cmd, action string) ([]byte, error) {
	out, err := cmd.CombinedOutput()
	if err != nil {
		return nil, fmt.Errorf("err %s: %s, %s", action, err, string(out))
	}
	return out, nil
}

func ListImages() (images []Image, err error) {
	cmd := exec.Command("qappctl", "images", "-o", "json")
	out, err := run(cmd, "list images")
	if err != nil {
		return nil, err
	}
	return images, json.Unmarshal(out, &images)
}
```
more at [ctl.rs](https://github.com/phosae/qappctl-shim-rs/blob/c6516cf96115920d8ef29aed8050a57b63299363/src/ctl.rs) and [wrapper.go](https://github.com/phosae/qappctl-shim/blob/0553b468bd1d6bd7c0e020f70e27a9957ceec338/wrapper.go)

## HTTP router: mock Golang HTTP Handler interface
register routes like Golang with [ibraheemdev/matchit](https://github.com/ibraheemdev/matchit) and our Handler implementation

As an HTTP server become larger, it's time to import an HTTP multiplexer (akka. service dispather). The multiplexer can make route dispather faster, parse URL path paramether in one place, and provide more readable route information. Code comparasion are at commit (route all by mux)[https://github.com/phosae/qappctl-shim-rs/commit/c6516cf96115920d8ef29aed8050a57b63299363].

In Golang, with help of standard library `net/http` or 3rd library like `[gorilla mux](github.com/gorilla/mux)`, we can register HTTP routes like this:
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
The biggest differnece here is that the async function in Rust is implemented as Future Trait exposing to developer, while in Golang there no difference in sync or async code(just some channel). Async code in Rust is more complicated and the way Rust managing memory make it even harder. We will see it later.

Since there's no default HTTP multiplexer in hyper, [[ibraheemdev/matchit]] will be used here. Thing left to us is implementing something like Go's `http.Handler` interface and `http.HandleFunc` in `net/http`. The first is definately, a Trait, for dynamic dispatching, and the latter will turn any async function with same signature into the same Trait.

The comming Trait implementation is inspire by tower's Service Trait[[3]].

### mock tower's Service Trait
let's start from minimal.

### the finally
Finally we can do thing like this in Rust
``` Rust
let mut mux: HashMap<Method, matchit::Router<HandlerFn>> = Router::new();
add_route(&mut mux, "/ctl/images", Method::GET, BoxCloneHandler::new(handler_fn(Svc::list_images)));
add_route(&mut mux, "/ctl/images", Method::POST, BoxCloneHandler::new(handler_fn(Svc::push_image)));
```

### Reference
[1]: [Life of an HTTP request in a Go server](https://eli.thegreenplace.net/2021/life-of-an-http-request-in-a-go-server/)
[2]: [Programming Rust: Fast, Safe Systems Development 2nd Edition](https://www.oreilly.com/library/view/programming-rust-2nd/9781492052586/)
[3]: [Inventing the Service trait](https://tokio.rs/blog/2021-05-14-inventing-the-service-trait)
[4]: [tower guides: building a middleware from scratch](https://github.com/tower-rs/tower/blob/master/guides/building-a-middleware-from-scratch.md)
[hyper]: [hyper](https://github.com/hyperium/hyper)
[ibraheemdev/matchit]: [ibraheemdev/matchit](https://github.com/ibraheemdev/matchit)