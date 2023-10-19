use std::env;
use std::fs;
use std::path::Path;

const CONFIG: &str = "config.rs";
const LOCAL_API_URL: &str = "https://login.screenly.local";
const STAGE_API_URL: &str = "https://api.screenlyappstage.com";
const PROD_API_URL: &str = "https://api.screenlyapp.com";

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join(CONFIG);

    let api_server = match env::var_os("API_SERVER_NAME") {
        Some(val) => {
            let api_server_name = val.into_string().unwrap();
            match api_server_name.as_str() {
                "local" => LOCAL_API_URL,
                "stage" => STAGE_API_URL,
                "prod" => PROD_API_URL,
                _ => {
                    panic!("Invalid API_SERVER_NAME. Use one of: local, stage, prod");
                }
            }
        }
        None => PROD_API_URL,
    };
    fs::write(
        dest_path,
        format!("pub const API_BASE_URL: &str = \"{}\";", api_server),
    )
    .unwrap();
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=API_SERVER_NAME");
}
