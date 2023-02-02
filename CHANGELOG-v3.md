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

## import HTTP router
As an HTTP server become larger, it's time to import an HTTP multiplexer (akka. service dispather). 

Go [How to implement Golang HTTP Handler interface like Rust Trait for router](./impl-go-httphandler-in-rust.md) for detail.
