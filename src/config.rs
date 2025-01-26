//use anyhow::{Context, Result};

pub const WIFI_SSID: &str = env!("WIFI_SSID");
pub const WIFI_PASS: &str = env!("WIFI_PASS");
pub const BOTIFACTORY_URL: &str = env!("BOTIFACTORY_URL");
pub const BOTIFACTORY_PROJECT_NAME: &str = env!("BOTIFACTORY_PROJECT_NAME");
pub const BOTIFACTORY_CHANNEL_NAME: &str = env!("BOTIFACTORY_CHANNEL_NAME");
pub const RELEASE_VERSION: &str = env!("BOTIFACTORY_RELEASE_VERSION");
/*
#[derive(Debug)]
pub struct Config {
    pub wifi_ssid: &'static str,
    pub wifi_pass: &'static str,
}

impl Config {
    pub fn load() -> Result<Self> {
        Ok(Self {
            wifi_ssid: env!("WIFI_SSID"),
            wifi_pass: env!("WIFI_PASS"),
        })
    }
}
*/
