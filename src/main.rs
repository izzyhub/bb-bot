use core::str;
use embedded_svc::{
    http::client::{Client, Response as RequestResponse},
    io::Read,
    wifi::{ClientConfiguration, Configuration as WifiConfiguration},
};

use esp_idf_hal::reset;
use esp_idf_hal::gpio;
use esp_idf_hal::prelude::Peripherals;
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::{peripheral, task::{block_on}},
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
use log::error;
//use tokio::io::{AsyncReadExt, AsyncWriteExt};
//use tokio::net::{TcpListener, TcpStream};
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

use tokio::time::{sleep, Duration};

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
                error!("error: {error}");
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
    Ok(str::from_utf8(&valid_utf8)?.to_string())
}

fn check_for_ota() -> Result<()> {
    info!("test");
    let connection = EspHttpConnection::new(&HttpConfiguration::default())?;
    info!("created connection");
    let mut client = Client::wrap(connection);
    info!("created client");
    let headers = [("accept", "application/json")];

    let release_url = format!(
        "{}/{}/{}/latest",
        config::BOTIFACTORY_URL,
        config::BOTIFACTORY_PROJECT_NAME,
        config::BOTIFACTORY_CHANNEL_NAME
    );
    info!("release_url: {release_url}");

    let request = client.request(Method::Get, &release_url, &headers)?;
    let mut response = request.submit()?;
    let status = response.status();
    info!("status: {status}");
    let response_string = read_response_string(&mut response)?;
    let release_response: ReleaseBody = serde_json::from_str(&response_string)?;

    let latest_version = Versioning::new(release_response.release.version).ok_or(NetworkError::BadVersion)?;
    let binary_version = Versioning::new(config::RELEASE_VERSION).ok_or(NetworkError::BadVersion)?;

    info!("latest version: {latest_version}");
    info!("latest_version: is_ideal: {}", latest_version.is_ideal());

    info!("binary version: {binary_version}");
    info!("binary_version: is_ideal: {}", binary_version.is_ideal());

    if latest_version > binary_version {
        info!("creating ota_man");
        let mut ota_man = ota::EspOta::new()?;
        info!("created ota_man");
        let slot = ota_man.get_boot_slot()?;
        info!("got slot");
        info!("boot slot: {slot:#?}");

        let slot = ota_man.get_running_slot()?;
        println!("running slot: {slot:#?}");

        let slot = ota_man.get_update_slot()?;
        println!("update slot: {slot:#?}");

        let headers = [("accept", "application/octet-stream")];
        let mut update_process = ota_man.initiate_update()?;

        let request = client.request(Method::Get, &release_url, &headers)?;
        let mut response = request.submit()?;
        let status = response.status();
        info!("status: {status}");
        if status == 200 {
            let mut buf = [0_u8; 256];
            while let Ok(size) = Read::read(&mut response, &mut buf) {
                info!("size: {size}");
                if size == 0 {
                    // It might be nice to check if we have any left over bytes here (ie. the offset > 0)
                    // as this would mean that the response ended with an invalid UTF-8 sequence, but for the
                    // purposes of this training we are assuming that the full response will be valid UTF-8
                    println!("breaking");
                    break;
                }
                update_process.write(&buf)?;
            }
            print!("while over??");
            //let finished = update_process.finish()?;
            update_process.complete();
            info!("restarting to update");
            reset::restart();
        }

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

async fn wifi_setup<'a>(
    sysloop: EspSystemEventLoop,
    nvs: EspDefaultNvsPartition,
) -> Result<WifiLoop<'a>> {
    let peripherals = Peripherals::take()?;
    let timer = EspTaskTimerService::new()?;
    let wifi = AsyncWifi::wrap(
        EspWifi::new(peripherals.modem, sysloop.clone(), Some(nvs))?,
        sysloop,
        timer.clone(),
    )?;
    let mut wifi_loop = WifiLoop { wifi };
    wifi_loop.configure().await?;
    wifi_loop.initial_connect().await?;
    Ok(wifi_loop)
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
    //let peripherals = Peripherals::take()?;
    let sysloop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;


    info!("Starting async run loop");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    println!("creating runtime???");
    runtime.block_on(async move {
        println!("inside runtime???");
        let mut wifi_loop = wifi_setup(sysloop.clone(), nvs).await.expect("Failed to setup wifi");
        info!("Initializing Wi-Fi...");
        println!("setup wifi??");
        tokio::spawn(async move {
            loop {
                while let Err(error) = wifi_loop.stay_connected().await {
                    error!("connection error: {error}");
                    println!("stay connected returned an err???");
                }
            }
        });
        loop {
            println!("looping??");
            tokio::spawn(async {
                info!("Preparing to do something???");
                match check_for_ota() {
                    Ok(_) => {
                        println!("ota success");
                    }
                    Err(error) => {
                        println!("ota error: {error}");
                    }
                };
            });
            info!("napping");
            println!("napping");
            sleep(Duration::from_secs(10)).await;
            println!("woke");
            info!("woke");
        }
    });
    println!("end???");

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
