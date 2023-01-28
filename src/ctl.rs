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

#[allow(dead_code)]
pub fn pull_image_in_docker(tag: String) -> Result<(), String> {
    let mut binding = Command::new("docker");
    let cmd = binding.arg("inspect").arg("--type=image").arg(tag.clone());
    if run::<()>(cmd, "docker inspect".to_owned()).is_ok() {
        return Ok(());
    }
    binding = Command::new("docker");
    let cmd: &mut Command = binding.arg("pull").arg(tag);
    Ok(run::<()>(cmd, "docker pull".to_owned())?)
}

#[allow(dead_code)]
pub fn push_image(tag: String) -> Result<(), String> {
    let mut binding = Command::new("qappctl");
    let cmd = binding.arg("push").arg(tag);

    Ok(run::<()>(cmd, "docker push".to_owned())?)
}

#[derive(Serialize, Deserialize, std::fmt::Debug)]
pub struct Image {
    name: String,
    tag: String,
    ctime: String, // decode to rfc3339 later
}

#[allow(dead_code)]
pub fn list_images() -> Result<Vec<Image>, String> {
    let mut binding = Command::new("qappctl");
    let cmd = binding.arg("images").arg("-o").arg("json");

    Ok(run::<Vec<Image>>(cmd, "qappctl images".to_owned())?)
}

#[allow(dead_code)]
pub fn login(ak: String, sk: String) -> Result<(), String> {
    let mut binding = Command::new("qappctl");
    let cmd = binding.arg("login").arg("--ak").arg(ak).arg("--sk").arg(sk);

    Ok(run::<_>(cmd, "login".to_owned())?)
}

#[derive(Serialize, Deserialize, std::fmt::Debug)]
pub struct App {
    pub name: String,
    pub desc: String,
}

#[allow(dead_code)]
pub fn list_apps() -> Result<Vec<App>, String> {
    let mut binding = Command::new("qappctl");
    let cmd = binding.arg("list").arg("-o").arg("json");

    Ok(run::<Vec<App>>(cmd, "list apps".to_owned())?)
}

#[test]
fn test_list_apps() {
    match list_apps() {
        Ok(ret) => println!("list apps success:ret {:?}", ret),
        Err(e) => println!("login failed: {}", e),
    }
}

#[derive(Serialize, Deserialize, std::fmt::Debug)]
pub struct Flavor {
    name: String,
    cpu: i32,
    mem: i32,
    gpu: String,
}

#[allow(dead_code)]
pub fn list_flavors() -> Result<Vec<Flavor>, String> {
    let mut binding = Command::new("qappctl");
    let cmd = binding.arg("flavor").arg("-o").arg("json");
    Ok(run::<Vec<Flavor>>(cmd, "list flavors".to_owned())?)
}

#[derive(Serialize, Deserialize, std::fmt::Debug)]
pub struct Region {
    pub name: String,
    pub desc: String,
}

#[allow(dead_code)]
pub fn list_regions() -> Result<Vec<Region>, String> {
    let mut binding = Command::new("qappctl");
    let cmd = binding.arg("region").arg("-o").arg("json");
    Ok(run::<Vec<Region>>(cmd, "list regions".to_owned())?)
}

#[derive(Serialize, Deserialize, std::fmt::Debug)]
pub struct HealthCheck {
    path: String,
    timeout: u32, // OPTIONAL unit: second, default 3s
}

#[derive(Serialize, Deserialize, std::fmt::Debug)]
pub struct EnvVariable {
    key: String,
    value: String,
}

#[derive(Serialize, Deserialize, std::fmt::Debug)]
pub struct ConfigFile {
    filename: String,
    mount_path: String,
    content: String, // OPTIONAL
}

#[derive(Serialize, Deserialize, std::fmt::Debug)]
pub struct ReleaseConfig {
    name: String,
    files: Vec<ConfigFile>,
}
#[derive(Serialize, Deserialize, std::fmt::Debug)]
pub struct Release {
    name: String,
    desc: String, // OPTIONAL
    image: String,
    flavor: String,
    port: u32,
    ctime: String, // RFC3339

    command: Vec<String>, // OPTIONAL
    args: Vec<String>,    // OPTIONAL

    health_check: Option<HealthCheck>,

    env: Vec<EnvVariable>,       // OPTIONAL
    log_file_paths: Vec<String>, // OPTIONAL
    config: Option<ReleaseConfig>,
}

#[allow(dead_code)]
pub fn list_releases(app: String) -> Result<Vec<Release>, String> {
    let mut binding = Command::new("qappctl");
    let cmd = binding
        .arg("release")
        .arg("list")
        .arg(app)
        .arg("-o")
        .arg("json");
    Ok(run::<Vec<Release>>(cmd, "list releases".to_owned())?)
}

#[allow(dead_code)]
pub fn create_release(app: String, cfgdir: String) -> Result<(), String> {
    let mut binding = Command::new("qappctl");
    let cmd = binding
        .arg("release")
        .arg("create")
        .arg(app)
        .arg("-c")
        .arg(cfgdir);

    Ok(run::<()>(cmd, "create release".to_owned())?)
}

#[derive(Serialize, Deserialize, std::fmt::Debug, Default)]
pub struct Deploy {
    id: String,
    release: String,
    region: String,
    replicas: u32,
    ctime: String, // RFC3339
}

#[allow(dead_code)]
pub fn list_deploys(app: String, region: String) -> Result<Vec<Deploy>, String> {
    let mut binding = Command::new("qappctl");
    let cmd = binding
        .arg("deploy")
        .arg("list")
        .arg(app)
        .arg("--region")
        .arg(region)
        .arg("-o")
        .arg("json");
    Ok(run::<Vec<Deploy>>(cmd, "list deployments".to_owned())?)
}

// qappctl deploy create <app> --release <release> --region <region> --expect_replicas <num>
//
//	{
//	  "id": "h221201-1658-30080-p8gv",
//	  "release": "zenx-v0",
//	  "region": "z0",
//	  "replicas": 0,
//	  "ctime": "0001-01-01T00:00:00Z"
//	}
#[allow(dead_code)]
pub fn create_deploy(
    app: String,
    release: String,
    region: String,
    replicas: u32,
) -> Result<Deploy, String> {
    let mut binding = Command::new("qappctl");
    let cmd = binding
        .arg("deploy")
        .arg("create")
        .arg(app)
        .arg("--region")
        .arg(region)
        .arg("--release")
        .arg(release)
        .arg("--expect_replicas")
        .arg(replicas.to_string())
        .arg("-o")
        .arg("json");
    Ok(run::<Deploy>(cmd, "create deploy".to_owned())?)
}

#[allow(dead_code)]
pub fn delete_deploy(app: String, id: String, region: String) -> Result<(), String> {
    let mut binding = Command::new("qappctl");
    let cmd = binding
        .arg("deploy")
        .arg("delete")
        .arg(app)
        .arg("--id")
        .arg(id)
        .arg("--region")
        .arg(region);
    Ok(run::<()>(cmd, "delete deploy".to_owned())?)
}

#[derive(Serialize, Deserialize, std::fmt::Debug)]
pub struct Instance {
    ctime: String,
    id: String,
    status: String,
    ips: String, //OPTIONAL
}

#[allow(dead_code)]
pub fn list_instances(app: String, id: String, region: String) -> Result<Vec<Instance>, String> {
    let mut binding = Command::new("qappctl");
    let cmd = binding
        .arg("instance")
        .arg("list")
        .arg(app)
        .arg("--deloy")
        .arg(id)
        .arg("--region")
        .arg(region)
        .arg("-o")
        .arg("json");
    Ok(run::<Vec<Instance>>(cmd, "create release".to_owned())?)
}
