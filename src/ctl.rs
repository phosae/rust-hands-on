use bytes::Buf;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::process::{Command, Output, Stdio};
use std::result::Result;
use std::str;

fn run<T: Default + DeserializeOwned>(
    out: Result<Output, std::io::Error>,
    action: String,
) -> Result<T, String> {
    match out {
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
                    Err(e) => Err(format!(
                        "err parse std json: {}, {:#?}",
                        e,
                        str::from_utf8(&o.stdout)
                    )),
                }
            } else {
                let stdout = String::from_utf8(o.stdout).unwrap();
                let stderr = String::from_utf8(o.stderr).unwrap();
                Err(format!("err {}: {}{}", action, stdout, stderr))
            }
        }
    }
}

pub fn push_image(tag: String) -> Result<(), String> {
    let ret = Command::new("docker").arg("push").arg(tag).output();
    Ok(run::<()>(ret, "docker push".to_owned())?)
}

#[derive(Serialize, Deserialize, std::fmt::Debug)]
pub struct Image {
    #[serde(rename(deserialize = "Repository", serialize = "name"))]
    name: String,
    #[serde(rename(deserialize = "Tag", serialize = "tag"))]
    tag: String,
    #[serde(rename(deserialize = "CreatedAt", serialize = "ctime"))]
    ctime: String, // decode to rfc3339 later
    #[serde(alias = "Size")]
    size: String,
}

// list_images by docker image ls --format "{{json . }}" | jq -s
pub fn list_images() -> Result<Vec<Image>, String> {
    let dockerchild = Command::new("docker")
        .arg("image")
        .arg("ls")
        .arg("--format")
        .arg("{{json . }}")
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let o = Command::new("jq")
        .stdin(Stdio::from(dockerchild.stdout.unwrap()))
        .arg("-s")
        .output();

    Ok(run::<Vec<Image>>(o, "docker images".to_owned())?)
}
