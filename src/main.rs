#![no_std]
#![no_main]

mod pci;

use bootloader::BootInfo;
use crossbeam::atomic::AtomicCell;
use pc_keyboard::DecodedKey;
use pci::init_pci;
use pluggable_interrupt_os::{println, vga_buffer::clear_screen, HandlerTable};
use pluggable_interrupt_template::LetterMover;

#[no_mangle]
pub extern "C" fn _start(boot_info: &'static BootInfo) -> ! {
    BOOT_INFO.store(Some(boot_info));
    HandlerTable::new()
        .keyboard(key)
        .timer(tick)
        .startup(startup)
        .cpu_loop(cpu_loop)
        .start()
}

static LAST_KEY: AtomicCell<Option<DecodedKey>> = AtomicCell::new(None);
static TICKED: AtomicCell<bool> = AtomicCell::new(false);
static BOOT_INFO: AtomicCell<Option<&'static BootInfo>> = AtomicCell::new(None);

fn cpu_loop() -> ! {
    let b = BOOT_INFO.load().unwrap();
    /*
    > 0x23AB88 - 0x1BB88
      2337672 − 113544 = 2224128
    > base 16
      2224128 = 0x0021F000

    > base 16
    > 0x239e88 - 0x1be88
    2334344 − 114312 = 0x0021E000
    */
    // println!("{:?}", b.kernal_image_offset);
    // loop {}
    init_pci(b);
    loop {
        if let Ok(_) = TICKED.compare_exchange(true, false) {
            // println!("Ticked!")
        }
    }
}

fn cpu_loop2() -> ! {
    let mut kernel = LetterMover::default();
    loop {
        if let Ok(_) = TICKED.compare_exchange(true, false) {
            kernel.tick();
        }

        if let Ok(k) = LAST_KEY.fetch_update(|k| if k.is_some() { Some(None) } else { None }) {
            if let Some(k) = k {
                kernel.key(k);
            }
        }
    }
}

fn key(key: DecodedKey) {
    LAST_KEY.store(Some(key));
}

fn tick() {
    TICKED.store(true);
}

fn startup() {
    // clear_screen();
}
