//use esp_backtrace as _;
use esp_println::println;

use esp_idf_hal::gpio;
use esp_idf_hal::prelude::Peripherals;
use esp_idf_svc:: {
    eventloop::EspSystemEventLoop,
    hal::peripheral,
    nvs,
    timer::EspTaskTimerService,
    wifi::{AuthMethod, BlockingWifi, ClientConfiguration, Configuration, EspWifi},
};
use std::thread::sleep;

mod wifi;
mod config;

use crate::config::Config;
use crate::wifi::WifiConnection;
use anyhow::Result;


fn main() {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();
    esp_idf_svc::io::vfs::initialize_eventfd(1).expect("Failed to initialize eventfd");
    //patch_eventfd();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to build tokio runtime");

    match rt.block_on(async { async_main().await}) {
        Ok(()) => log::info!("main() finished reboot."),
        Err(err) => {
            log::error!("{err:?}");
            sleep(std::time::Duration::from_secs(3));
        },
    }
    log::info!("Hello, world!");
    println!("print world!");

    esp_idf_hal::reset::restart();
}

/*
pub async fn run_hello(wifi_state: Arc<WifiState>) -> anyhow::Result<()> {
    let state = Arc::new(SharedState {
        counter: AtomicIsize::new(0),
        wifi_state,
    });
}
*/

async fn async_main() -> Result<()> {
    log::info!("Starting async_main.");

    let config = Config::load()?;
    //log::info!("Configuration:\n{config:#?}");

    let event_loop = EspSystemEventLoop::take()?;
    let timer = EspTaskTimerService::new()?;
    let peripherals = Peripherals::take()?;
    let nvs_default_partition = nvs::EspDefaultNvsPartition::take()?;

    let mut wifi_connection = WifiConnection::new(
        peripherals.modem,
        event_loop,
        timer,
        Some(nvs_default_partition),
        &config,
    )
    .await?;

    //tokio::try_join!(
        //run_server(wifi_connection.state.clone()),
        wifi_connection.connect().await?;
    //)?;

    log::info!("Wifi connection successful");
    Ok(())
}
