#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::usb::Driver;
use embassy_stm32::{bind_interrupts, peripherals, usb, Config};
use embassy_time::Timer;
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use embassy_usb::Builder;
use editline::{AsyncLineEditor, AsyncTerminal, terminals::EmbassyUsbTerminal};
use {defmt_rtt as _, panic_probe as _};

extern crate alloc;
use alloc_cortex_m::CortexMHeap;

#[global_allocator]
static ALLOCATOR: CortexMHeap = CortexMHeap::empty();

defmt::timestamp!("{=u64:us}", {
    embassy_time::Instant::now().as_micros()
});

bind_interrupts!(struct Irqs {
    OTG_FS => usb::InterruptHandler<peripherals::USB_OTG_FS>;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    // Initialize the allocator
    {
        use core::mem::MaybeUninit;
        const HEAP_SIZE: usize = 32768;
        static mut HEAP: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
        unsafe { ALLOCATOR.init(HEAP.as_ptr() as usize, HEAP_SIZE) }
    }

    let mut config = Config::default();
    {
        use embassy_stm32::rcc::*;
        config.rcc.hse = Some(Hse {
            freq: embassy_stm32::time::Hertz(8_000_000),
            mode: HseMode::Bypass,
        });
        config.rcc.hsi48 = Some(Hsi48Config { sync_from_usb: true });
        config.rcc.pll1 = Some(Pll {
            source: PllSource::HSE,
            prediv: PllPreDiv::DIV4,
            mul: PllMul::MUL240,
            divp: Some(PllDiv::DIV2),
            divq: Some(PllDiv::DIV8),
            divr: None,
        });
        config.rcc.sys = Sysclk::PLL1_P;
        config.rcc.ahb_pre = AHBPrescaler::DIV2;
        config.rcc.apb1_pre = APBPrescaler::DIV2;
        config.rcc.apb2_pre = APBPrescaler::DIV2;
        config.rcc.apb3_pre = APBPrescaler::DIV2;
        config.rcc.apb4_pre = APBPrescaler::DIV2;
        config.rcc.voltage_scale = VoltageScale::Scale1;
        config.rcc.mux.usbsel = mux::Usbsel::HSI48;
    }

    let p = embassy_stm32::init(config);

    defmt::info!("STM32H753ZI editline async REPL example");

    // Create USB driver
    let mut usb_config = usb::Config::default();
    usb_config.vbus_detection = false;

    let mut ep_out_buffer = [0u8; 256];
    let driver = Driver::new_fs(p.USB_OTG_FS, Irqs, p.PA12, p.PA11, &mut ep_out_buffer, usb_config);

    // Create USB device config
    let mut config_descriptor = [0u8; 256];
    let mut bos_descriptor = [0u8; 256];
    let mut control_buf = [0u8; 64];

    let mut usb_config = embassy_usb::Config::new(0xc0de, 0xcafe);
    usb_config.manufacturer = Some("editline");
    usb_config.product = Some("STM32H753 Async REPL");
    usb_config.serial_number = Some("12345678");
    usb_config.max_power = 100;
    usb_config.max_packet_size_0 = 64;

    // Create CDC ACM state
    let mut state = State::new();

    let mut builder = Builder::new(
        driver,
        usb_config,
        &mut config_descriptor,
        &mut bos_descriptor,
        &mut [],
        &mut control_buf,
    );

    // Create CDC ACM class
    let class = CdcAcmClass::new(&mut builder, &mut state, 64);

    // Build USB device
    let mut usb = builder.build();

    defmt::info!("USB device initialized");

    // Turn on green LED to indicate we're ready
    let mut _led = Output::new(p.PB0, Level::High, Speed::Low);

    // Run USB device and REPL concurrently
    let usb_fut = usb.run();

    let repl_fut = async {
        // Create terminal and editor
        let mut terminal = EmbassyUsbTerminal::new(class);
        let mut editor = AsyncLineEditor::new(256, 10);

        defmt::info!("Waiting for terminal connection (DTR)...");
        terminal.wait_connection().await;
        defmt::info!("Terminal connected!");

        // Send banner
        let _ = terminal.write(b"Welcome to STM32H753ZI async REPL!\r\n").await;
        let _ = terminal.write(b"Type 'help' for commands, 'exit' to quit\r\n\r\n").await;
        let _ = terminal.flush().await;

        loop {
            // Show prompt
            let _ = terminal.write(b"> ").await;
            let _ = terminal.flush().await;

            // Read line with full editing support
            match editor.read_line(&mut terminal).await {
                Ok(line) => {
                    defmt::info!("Got command: {}", line.as_str());

                    // Process command
                    if line == "exit" {
                        let _ = terminal.write(b"Goodbye!\r\n").await;
                        break;
                    } else if line == "help" {
                        let _ = terminal.write(b"Available commands:\r\n").await;
                        let _ = terminal.write(b"  help  - Show this help\r\n").await;
                        let _ = terminal.write(b"  hello - Say hello\r\n").await;
                        let _ = terminal.write(b"  exit  - Exit the REPL\r\n").await;
                    } else if line == "hello" {
                        let _ = terminal.write(b"Hello from STM32H753ZI!\r\n").await;
                    } else if line.is_empty() {
                        // Just show prompt again
                        continue;
                    } else {
                        let _ = terminal.write(b"Unknown command: ").await;
                        let _ = terminal.write(line.as_bytes()).await;
                        let _ = terminal.write(b"\r\n").await;
                        let _ = terminal.write(b"Type 'help' for available commands\r\n").await;
                    }
                    let _ = terminal.flush().await;
                }
                Err(_e) => {
                    defmt::error!("Error reading line");
                    break;
                }
            }

            // Check if still connected
            if !terminal.dtr() {
                defmt::info!("Terminal disconnected");
                break;
            }
        }

        defmt::info!("REPL exited");
    };

    join(usb_fut, repl_fut).await;
}
