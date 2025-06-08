use std::{
    collections::HashMap,
    env::{JoinPathsError, join_paths},
    error::Error,
    fs::{self, Permissions},
    io::Write,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use toml::from_str;
use uu_whoami::whoami;

fn get_config_dir() -> Result<PathBuf, Box<dyn Error>> {
    let username = whoami().expect("Cannot get username");

    let mut string = String::with_capacity(username.len() + 27);

    string.push_str("/home/");
    string.push_str(username.to_str().ok_or("Cannot translate username")?);
    string.push_str("/.config/dwl-launcher");

    Ok(PathBuf::from(string))
}

fn prepare() -> Result<(), Box<dyn Error>> {
    let dir = get_config_dir()?;

    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }

    let services_path = join_with_tail(&dir, "services")?;
    let service_file = fs::File::create_new(&services_path);

    if service_file.is_ok() {
        let default = ServiceFile::default();

        let data = toml::to_string(&default)?;

        write_string(data, services_path)?;
    }

    let envs_path = join_with_tail(&dir, "envs")?;
    let envs_file = fs::File::create_new(&envs_path);

    if envs_file.is_ok() {
        let default = Envs::from([
            ("XDG_CURRENT_DESKTOP".to_string(), "wlroots".to_string()),
            ("XDG_SESSION_TYPE".to_string(), "wayland".to_string()),
        ]);

        let data = toml::to_string(&default)?;

        write_string(data, envs_path)?;
    }

    Ok(())
}

fn read_to_struct<T: DeserializeOwned, S: AsRef<str>>(path: S) -> Result<T, Box<dyn Error>> {
    let file = fs::read_to_string(path.as_ref())?;

    Ok(from_str(&file)?)
}

fn join_with_tail<Str1: AsRef<Path>, Str2: AsRef<Path>>(
    root: Str1,
    tail: Str2,
) -> Result<PathBuf, JoinPathsError> {
    let root_buf = root.as_ref().to_path_buf();
    let tail_buf = tail.as_ref().to_path_buf();
    let os_string = join_paths(&[root_buf, tail_buf])?;

    Ok(os_string.into())
}

fn main() -> Result<(), Box<dyn Error>> {
    prepare()?;

    let dir = get_config_dir()?;

    let services: ServiceFile =
        read_to_struct(join_with_tail(&dir, "services")?.to_string_lossy())?;

    let envs: Envs = read_to_struct(join_with_tail(&dir, "envs")?.to_string_lossy())?;

    let script = generate_script(services);

    write_string(script, "/tmp/dwl_service")?;

    init(envs)?;

    Ok(())
}

type Envs = HashMap<String, String>;

fn init(envs: Envs) -> Result<Child, Box<dyn Error>> {
    let mut command = Command::new("/usr/local/bin/dwl");

    Ok(command
        .envs(envs)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .arg("-s \"/tmp/dwl_service\"")
        .spawn()?)
}

#[derive(Deserialize, Serialize, Debug)]
struct Service {
    name: String,
    exec: String,
}

#[derive(Deserialize, Serialize, Debug)]
struct ServiceFile {
    service: Vec<Service>,
}

impl Default for ServiceFile {
    fn default() -> Self {
        Self {
            service: vec![Service {
                    name: "Import environment".into(),
                    exec: "/sbin/systemctl --user import-environment DISPLAY WAYLAND_DISPLAY XDG_CURRENT_DESKTOP".into()
                }
            ]
        }
    }
}

/*

impl ServiceFile {
    fn new<Str1: Into<String>, Str2: Into<String>>(name: Str1, exec: Str2) -> Self {
        Self {
            service: vec![Service {
                    name: name.into(),
                    exec: exec.into()
                }
            ]
        }
    }
}

*/

fn generate_script(services: ServiceFile) -> String {
    let mut string = String::new();

    string.push_str("#!/bin/bash\n\n");

    for service in services.service {
        string.push_str("# ");
        string.push_str(&service.name);

        string.push('\n');

        string.push_str(&service.exec);
        string.push_str(" &\n");
    }

    println!("{string}");

    string
}

fn directory_of<P: AsRef<Path>>(input: P) -> Result<PathBuf, Box<dyn Error>> {
    input
        .as_ref()
        .parent()
        .map(PathBuf::from)
        .ok_or("Can't get parent directory".into())
}

fn write_string<Str: AsRef<str>, PathStr: AsRef<Path>>(
    string: Str,
    path: PathStr,
) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(directory_of(path.as_ref())?)?;

    let mut file = fs::File::create(path.as_ref())?;
    write!(file, "{}", string.as_ref())?;
    file.set_permissions(Permissions::from_mode(0o744))?;

    Ok(())
}
