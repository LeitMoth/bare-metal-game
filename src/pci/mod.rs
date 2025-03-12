use headers::{half_u32, parse_header_common, parse_header_type0};
use io::{pci_config_read_u32, pci_config_read_word, pci_config_write_u32};
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

/*

 void checkAllBuses(void) {
     uint16_t bus;
     uint8_t device;

     for (bus = 0; bus < 256; bus++) {
         for (device = 0; device < 32; device++) {
             checkDevice(bus, device);
         }
     }
 }
*/

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
                        // We are supposed to enable
                        let mut line =
                            ((h.headhead.status as u32) << 16) | (h.headhead.command as u32);
                        line |= 0b101;

                        pci_config_write_u32(bus, device, 0, 0x1, line);

                        let (_, command) = half_u32(pci_config_read_u32(bus, device, 0, 1));

                        println!("{:#013b}", command);
                    }
                }
            }
        }
    }
}
