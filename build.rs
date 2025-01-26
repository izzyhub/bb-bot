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
