#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer, with_timeout};
use esp_hal::clock::CpuClock;
use esp_hal::timer::timg::TimerGroup;
use esp_hal::rng::Rng;
use esp_csi_rs::{collector::CSISniffer, config::CSIConfig};
use esp_wifi::{init, EspWifiController};

extern crate alloc;

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

macro_rules! mk_static {
    ($t:ty,$val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write(($val));
        x
    }};
}

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    // Configure System Clock
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    // Take Peripherals
    let peripherals = esp_hal::init(config);

    // Allocate some heap space
    esp_alloc::heap_allocator!(size: 72 * 1024);

    // Initialize Embassy - ESP32-C2 only has TIMG0, so use systimer for embassy
    let systimer = esp_hal::timer::systimer::SystemTimer::new(peripherals.SYSTIMER);
    esp_hal_embassy::init(systimer.alarm0);

    // Instantiate peripherals necessary to set up WiFi
    let timer1 = TimerGroup::new(peripherals.TIMG0);
    let wifi = peripherals.WIFI;
    let timer = timer1.timer0;
    let rng = Rng::new(peripherals.RNG);

    // Initialize WiFi Controller
    let init = &*mk_static!(EspWifiController<'static>, init(timer, rng).unwrap());
    // Instantiate WiFi controller and interfaces
    let (controller, interfaces) = esp_wifi::wifi::new(&init, wifi).unwrap();

    // Create a Collector Instance
    let mut csi_coll_snif = CSISniffer::new(CSIConfig::default(), controller).await;

    // Initialize CSI Collector
    csi_coll_snif.init(interfaces, &spawner).await.unwrap();

    // Start Collection
    csi_coll_snif.start_collection().await;

    // Collect for 2 Seconds
    with_timeout(Duration::from_secs(2), async {
        loop {
            csi_coll_snif.print_csi_w_metadata().await;
        }
    })
    .await
    .unwrap_err();

    // Stop Collection
    csi_coll_snif.stop_collection().await;

    // Keep running
    loop {
        Timer::after(Duration::from_secs(1)).await;
    }
}
