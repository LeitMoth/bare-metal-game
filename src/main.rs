#![no_std]
#![no_main]

mod pci;
mod phys_alloc;

use bootloader::BootInfo;
use crossbeam::atomic::AtomicCell;
use pc_keyboard::DecodedKey;
use pci::init_pci;
use phys_alloc::PhysAllocator;
use pluggable_interrupt_os::{vga_buffer::clear_screen, HandlerTable};
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
    let info = BOOT_INFO.load().unwrap();
    let mut phys_alloc = PhysAllocator::new(info).unwrap();

    init_pci(&mut phys_alloc);

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
    clear_screen();
}
