#![no_std]
#![no_main]

mod pci;
mod phys_alloc;
mod spacefox;

use bootloader::BootInfo;
use crossbeam::atomic::AtomicCell;
use pc_keyboard::DecodedKey;
use pci::scan_pci_devices;
use phys_alloc::PhysAllocator;
use pluggable_interrupt_os::{vga_buffer::clear_screen, HandlerTable};
use spacefox::Game;

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

    let devs = scan_pci_devices();
    let ac97 = devs.ac97.unwrap();

    let mut game = Game::new(&mut phys_alloc, ac97);

    loop {
        if let Ok(_) = TICKED.compare_exchange(true, false) {
            game.tick()
        }

        if let Ok(k) = LAST_KEY.fetch_update(|k| if k.is_some() { Some(None) } else { None }) {
            if let Some(k) = k {
                game.key(k);
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
