use core::str;
use embedded_svc::{
    http::client::{Client, Response as RequestResponse},
    io::Read,
    wifi::{ClientConfiguration, Configuration as WifiConfiguration},
};

use esp_idf_hal::gpio;
use esp_idf_hal::prelude::Peripherals;
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::peripheral,
    nvs,
    ota,
    http::client::{Configuration as HttpConfiguration, EspHttpConnection, Method},
    nvs::EspDefaultNvsPartition,
    timer::EspTaskTimerService,
    wifi::{AsyncWifi, EspWifi},
};
use esp_idf_sys as _;
use esp_idf_sys::{esp, esp_app_desc, EspError};
use log::info;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
//use botifactory_common::{Botifactory, Identifier, NewRelease};
use bytes::{BufMut, BytesMut};

mod config;
mod error;

use error::BBBotError;

use anyhow::Result;
use botifactory_types::{ProjectBody, ReleaseBody};
use serde::Deserialize;
use serde_json::from_slice;
use thiserror::Error;

use versions::Versioning;

#[derive(Error, Debug)]
pub enum NetworkError {
    #[error("not a full response")]
    IncompleteResponse,
    #[error("json deserialize error")]
    JsonError(#[from] serde_json::Error),
    #[error("String error")]
    Utf8Error(#[from] std::str::Utf8Error),
    #[error("Bad version")]
    BadVersion,
}

fn read_response_string<'a, C>(response: &'a mut RequestResponse<C>) -> Result<String, NetworkError> 
where
    C: embedded_svc::http::client::Connection,
{
    let status = response.status();
    println!("status: {status}");
    let mut valid_utf8 = BytesMut::with_capacity(1024);
    match status {
        200..=299 => {
            //let mut buf = BytesMut::with_capacity(1024);
            let mut buf = [0_u8; 256];
            let mut reader = response;
            let mut offset = 0;
            let mut total = 0;
            println!("offset: {offset}");
            while let Ok(size) = Read::read(&mut reader, &mut buf[offset..]) {
                println!("size: {size}");
                println!("offset: {offset}");
                if size == 0 {
                    // It might be nice to check if we have any left over bytes here (ie. the offset > 0)
                    // as this would mean that the response ended with an invalid UTF-8 sequence, but for the
                    // purposes of this training we are assuming that the full response will be valid UTF-8
                    println!("breaking");
                    break;
                }
                total += size;
                println!("total: {total}");

                let size_plus_offset = size + offset;
                match str::from_utf8(&buf[..size_plus_offset]) {
                    Ok(text) => {
                        println!("text: {text}");
                        valid_utf8.extend_from_slice(&buf[..size_plus_offset]);
                        offset = 0;
                    }
                    Err(error) => {
                        println!("error: {error}");
                        let valid_up_to = error.valid_up_to();
                        unsafe {
                            println!("{}", str::from_utf8_unchecked(&buf[..valid_up_to]));
                        }
                        valid_utf8.extend_from_slice(&buf[..valid_up_to]);
                        offset = size_plus_offset - valid_up_to;
                    }
                }
            }
        }
        _ => {
            println!("unexpected response code: {status}");
            return Err(NetworkError::IncompleteResponse.into());
        }
    };
    //println!("Incomplete response???");
    //return Err(NetworkError::IncompleteResponse.into());
    Ok(str::from_utf8(&valid_utf8)?.to_string())
}

/*
fn read_response_json<'a, 'b, T, C>(response: &'a mut RequestResponse<C>) -> Result<T, NetworkError>
where
    T: Deserialize<'b>,
    C: embedded_svc::http::client::Connection,
{
    let text_response = read_response_string(response)?;
    Ok(serde_json::from_str::<T>(&text_response)?)
}
*/

fn test() -> Result<()> {
    info!("test");
    let connection = EspHttpConnection::new(&HttpConfiguration::default())?;
    info!("created connection");
    let mut client = Client::wrap(connection);
    info!("created client");
    let headers = [("accept", "application/json")];

    /*
    let health_url = format!("{}/health_check", config::BOTIFACTORY_URL);
    info!("health_url: {health_url}");
    let request = client.request(Method::Get, &health_url, &headers)?;
    info!("requesting...");
    let response = request.submit()?;
    info!("responded");
    let status = response.status();
    println!("status: {status}");
    */

    let project_url = format!(
        "{}/project/{}",
        config::BOTIFACTORY_URL,
        config::BOTIFACTORY_PROJECT_NAME
    );
    info!("project_url: {project_url}");

    info!("log world");
    let request = client.request(Method::Get, &project_url, &headers)?;
    info!("requesting...");
    let mut response = request.submit()?;
    let response_string = read_response_string(&mut response)?;
    println!("response_string: '{response_string}'");
    let project_response: ProjectBody = serde_json::from_str(&response_string)?;
    println!("serde_json::from_str finished");
    println!("project_response: {project_response}");
    info!("project_response: {project_response}");

    info!("responded");

    let release_url = format!(
        "{}/{}/{}/latest",
        config::BOTIFACTORY_URL,
        config::BOTIFACTORY_PROJECT_NAME,
        config::BOTIFACTORY_CHANNEL_NAME
    );
    info!("release_url: {release_url}");

    info!("log world");
    let request = client.request(Method::Get, &release_url, &headers)?;
    info!("requesting...");
    let mut response = request.submit()?;
    info!("responded");
    let status = response.status();
    println!("status: {status}");
    let response_string = read_response_string(&mut response)?;
    println!("response_string: ({response_string})");
    let release_response: ReleaseBody = serde_json::from_str(&response_string)?;
    println!("release_response: {release_response}");
    let latest_version = Versioning::new(release_response.release.version).ok_or(NetworkError::BadVersion)?;
    println!("latest version: {latest_version}");
    info!("latest version: {latest_version}");

    println!("latest_version: is_ideal: {}", latest_version.is_ideal());

    let binary_version = Versioning::new(config::RELEASE_VERSION).ok_or(NetworkError::BadVersion)?;
    println!("binary version: {binary_version}");
    info!("binary version: {binary_version}");
    println!("binary_version: is_ideal: {}", binary_version.is_ideal());

    if latest_version > binary_version {
        println!("creating ota_man");
        let ota_man = ota::EspOta::new()?;
        println!("created ota_man");
        let slot = ota_man.get_boot_slot()?;
        println!("got slot");
        println!("boot slot: {slot:#?}");

        let slot = ota_man.get_running_slot()?;
        println!("running slot: {slot:#?}");

        let slot = ota_man.get_update_slot()?;
        println!("update slot: {slot:#?}");
    }
    else {
        println!("running the current version");
        info!("running the current version");
    }

    println!("Hello world");
    //use tokio::time::{sleep, Duration};
    //sleep(Duration::from_secs(1)).await;
    println!("still here world 🥺");
    info!("still here world 🥺");
    Ok(())
}

esp_app_desc!();
fn main() -> anyhow::Result<()> {
    esp_idf_sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    // eventfd is needed by our mio poll implementation.  Note you should set max_fds
    // higher if you have other code that may need eventfd.

    info!("setting up eventfd...");
    let eventd_config = esp_idf_sys::esp_vfs_eventfd_config_t {
        max_fds: 1,
        ..Default::default()
    };
    esp! { unsafe { esp_idf_sys::esp_vfs_eventfd_register(&eventd_config) } }?;

    info!("Setting up board...");
    let peripherals = Peripherals::take().unwrap();
    let sysloop = EspSystemEventLoop::take()?;
    let timer = EspTaskTimerService::new()?;
    let nvs = EspDefaultNvsPartition::take()?;

    info!("Initializing Wi-Fi...");
    let wifi = AsyncWifi::wrap(
        EspWifi::new(peripherals.modem, sysloop.clone(), Some(nvs))?,
        sysloop,
        timer.clone(),
    )?;

    info!("Starting async run loop");
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async move {
            let mut wifi_loop = WifiLoop { wifi };
            wifi_loop.configure().await?;
            wifi_loop.initial_connect().await?;

            //use tokio::task::JoinSet;
            //let mut set = JoinSet::new();
            info!("Preparing to do something???");
            //set.spawn(test());
            match test() {
                Ok(_) => {
                    println!("test success");
                }
                Err(error) => {
                    println!("error: {error}");
                }
            };
            wifi_loop.stay_connected().await
        })?;


    Ok(())
}

pub struct WifiLoop<'a> {
    wifi: AsyncWifi<EspWifi<'a>>,
}

impl<'a> WifiLoop<'a> {
    pub async fn configure(&mut self) -> Result<(), BBBotError> {
        info!("Setting Wi-Fi credentials...");
        use heapless::String as HeaplessString;

        println!("ssid: {}", config::WIFI_SSID);
        println!("pass: {}", config::WIFI_PASS);

        let mut ssid_heapless: HeaplessString<32> = HeaplessString::new();
        ssid_heapless.push_str(config::WIFI_SSID)?;

        let mut pass_heapless: HeaplessString<64> = HeaplessString::new();
        pass_heapless.push_str(config::WIFI_PASS)?;

        self.wifi
            .set_configuration(&WifiConfiguration::Client(ClientConfiguration {
                ssid: ssid_heapless,
                password: pass_heapless,
                ..Default::default()
            }))?;

        info!("Starting Wi-Fi driver...");
        self.wifi.start().await?;
        Ok(())
    }

    pub async fn initial_connect(&mut self) -> Result<(), BBBotError> {
        info!("initial connect");
        self.do_connect_loop(true).await
    }
    pub async fn stay_connected(&mut self) -> Result<(), BBBotError> {
        info!("stay connected");
        self.do_connect_loop(false).await
    }

    async fn do_connect_loop(&mut self, exit_after_first_connect: bool) -> Result<(), BBBotError> {
        let wifi = &mut self.wifi;
        loop {
            // Wait for disconnect before trying to connect again.  This loop ensures
            // we stay connected and is commonly missing from trivial examples as it's
            // way too difficult to showcase the core logic of an example and have
            // a proper Wi-Fi event loop without a robust async runtime.  Fortunately, we can do it
            // now!
            wifi.wifi_wait(|this| this.is_up(), None).await?;

            info!("Connecting to Wi-Fi...");
            wifi.connect().await?;

            info!("Waiting for association...");
            wifi.ip_wait_while(
                |this| {
                    println!("{:#?}", this.is_up());
                    this.is_up().map(|s| !s)
                },
                None,
            )
            .await?;

            if exit_after_first_connect {
                return Ok(());
            }
        }
    }
}
