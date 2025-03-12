use headers::{half_u32, parse_header_common, parse_header_type0};
use io::{pci_config_modify, pci_config_read_u32, pci_config_read_word, pci_config_write_u32};
use pluggable_interrupt_os::println;

mod headers;
mod io;

/*
Everything here makes extensive reference of: https://wiki.osdev.org/PCI
And some: https://wiki.osdev.org/AC97
As well as other linked references
*/

pub fn init_pci() {
    check_all();

    init_audio();
}

#[derive(Debug)]
struct AudioAc97 {
    bus: u8,
    slot: u8,
    bar0: u32,
    bar1: u32,
}

fn init_audio() -> Option<AudioAc97> {
    let mut audio = None;

    for bus in 0..=255 {
        for device in 0..32 {
            let vendor = pci_config_read_word(bus, device, 0, 0);
            if vendor != 0xFFFF {
                let headhead = parse_header_common(bus, device, 0);
                debug_assert!(vendor == headhead.vendor_id);

                if headhead.header_type == 0x0
                    && headhead.class_code == 0x04
                    && headhead.subclass == 0x01
                {
                    // because header_type == 0, we know that this is a single function device
                    // (otherwise bit 7 would be set)
                    let full_header = parse_header_type0(bus, device, 0, headhead);

                    #[cfg(debug_assertions)]
                    if audio.is_some() {
                        println!("Warning, found multiple AC97 devices!");
                    }

                    audio = Some(AudioAc97 {
                        bus,
                        slot: device,
                        bar0: full_header.base_addresses[0],
                        bar1: full_header.base_addresses[0],
                    });
                }
            }
        }
    }

    audio
}

struct Vendor(u16);
impl Vendor {
    fn id(&self) -> Option<u16> {
        if self.0 == 0xFFFF {
            None
        } else {
            Some(self.0)
        }
    }
}
fn pci_check_vendor(bus: u8, slot: u8) -> Vendor {
    Vendor(pci_config_read_word(bus, slot, 0, 0))
}

fn check_all() {
    for bus in 0..=255 {
        for device in 0..32 {
            let v = pci_check_vendor(bus, device);
            if let Some(id) = v.id() {
                let h = parse_header_common(bus, device, 0);
                debug_assert!(id == h.vendor_id);

                println!(
                    "BUS{bus}   DEVICE{device}   V:D {:#06X}:{:#06X}    CLS:SUBCLS {:#04X}:{:#04X}   HTY{:#04X}",
                    h.vendor_id, h.device_id, h.class_code, h.subclass, h.header_type
                );

                if h.header_type == 0 {
                    let h = parse_header_type0(bus, device, 0, h);

                    if h.headhead.class_code == 0x4 {
                        // println!("{h:#X?}");
                        println!("{:#013b}", h.headhead.command);

                        // https://wiki.osdev.org/AC97#Detecting_AC97_sound_card
                        pci_config_modify(bus, device, 0, 0x1, |x| x | 0b101);

                        let (_, command) = half_u32(pci_config_read_u32(bus, device, 0, 1));
                        println!("{:#013b}", command);
                    }
                }
            }
        }
    }
}
