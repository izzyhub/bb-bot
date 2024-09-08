use paste::paste;
use std::env;
use std::fs;
use std::path::Path;
use std::str::FromStr;

macro_rules! p {
    ($($tokens: tt)*) => {{
        if env::var("BUILD_PRINT_EXPANDED_ENV").unwrap_or_default() == "true" {
            println!("cargo:warning={}", format!($($tokens)*));
        }
    }}
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    embuild::espidf::sysenv::output();
    Ok(())
}

fn build_env() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("build_env.rs");
    dotenvy::from_filename("build.env")?;
    let mut lines: Vec<String> = vec![];

    macro_rules! env_string {
        ($name: expr, $default: expr) => {
            paste! {
                lines.push(format!(
                   "pub static {}: &static str = \"{}\";",
                   $name,
                   env::var($name).unwrap_or($default.into())
                ));
            }
        };
    }
    macro_rules! env_number {
        ($name: expr, $type: ty, $default: expr) => {
            paste! {
                lines.push(format!(
                        "pub static {}: {} = {};",
                        $name,
                        stringify!($type),
                        env::var($name).map(|s| $type::from_str(s.as_str()).unwrap()).unwrap_or($default)
                ));
            }
        };
    }

    env_string!("WIFI_SSID", "WIFI_PASS");

    for l in lines.iter() {
        p!("cargo:warning={}", l)
    }
    fs::write(&dest_path, lines.join("\n")).unwrap();
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=build.env");

    Ok(())
}
