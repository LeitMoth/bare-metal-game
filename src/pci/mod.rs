use headers::{half_u32, parse_header_common, parse_header_type0};
use io::{
    io_space_bar_read, io_space_bar_write, pci_config_modify, pci_config_read_u32,
    pci_config_read_word, pci_config_write_u32,
};
use lazy_static::lazy_static;
use pluggable_interrupt_os::{print, println};
use spin::Mutex;
use x86_64::instructions::port::Port;

mod headers;
mod io;

/*
Everything here makes extensive reference of: https://wiki.osdev.org/PCI
And some: https://wiki.osdev.org/AC97
As well as other linked references
*/

pub fn init_pci() {
    // check_all();
    let a = init_audio().unwrap();
    a.play();
}

const WAVSIZE: usize = 0x1000;

lazy_static! {
    static ref SAMPLE: [i16; WAVSIZE * 2] = {
        let mut x = [0i16; WAVSIZE * 2];
        for i in 0..x.len() {
            x[i] = ((i * 100) % (i16::MAX as usize)) as i16;
        }
        x
    };
}

#[repr(packed)]
#[derive(Debug, Default, Clone, Copy)]
struct BufferDescriptor {
    physical_addr: u32,
    num_samples: u16,
    // From https://wiki.osdev.org/AC97#Buffer%20Descriptor%20List
    // Bit 15=Interrupt fired when data from this entry is transferred
    // Bit 14=Last entry of buffer, stop playing
    // Other bits=Reserved
    control: u16,
}

impl BufferDescriptor {
    fn square() -> Self {
        let raw_addr: *const [i16; WAVSIZE * 2] = &raw const *SAMPLE;

        debug_assert!(raw_addr as u64 <= u32::MAX as u64);
        let addr_truncated = raw_addr as u32;

        BufferDescriptor {
            physical_addr: addr_truncated,
            num_samples: WAVSIZE as u16,
            control: 1 << 15,
        }
    }
}

lazy_static! {
    static ref BUFFER_DESCRIPTOR_LIST: Mutex<[BufferDescriptor; 32]> =
        Mutex::new([BufferDescriptor::square(); 32]);
}

#[derive(Debug)]
struct AudioAc97 {
    bus: u8,
    slot: u8,
    // reset, device selection, volume control
    bar0: u16,
    // audio data
    bar1: u16,
}

impl AudioAc97 {
    fn play(&self) {
        pci_config_modify(self.bus, self.slot, 0, 0x1, |x| x | 0b101);
        // pci_config_modify(self.bus, self.slot, 0, 0xF, |x| {
        //     x | 0b00000000_00000000_00000011_00000011
        // });

        io_space_bar_write::<u16>(self.bar0 + 0x00, 0x0);

        // set volumes
        io_space_bar_write::<u16>(self.bar0 + 0x02, 0x0);
        io_space_bar_write::<u16>(self.bar0 + 0x04, 0x0);

        io_space_bar_write::<u16>(self.bar0 + 0x18, 0x0);

        let samp_front = io_space_bar_read::<u16>(self.bar0 + 0x2C);
        let samp_surr = io_space_bar_read::<u16>(self.bar0 + 0x2E);
        let samp_lfe = io_space_bar_read::<u16>(self.bar0 + 0x30);
        let samp_lr = io_space_bar_read::<u16>(self.bar0 + 0x32);
        println!("samp {samp_front} {samp_surr} {samp_lfe} {samp_lr}");

        io_space_bar_write::<u16>(self.bar1 + 0x2C, 0b0111);
        let mut v = 10;
        for i in 0..1000000 {
            v -= 1;
            print!("");
        }
        let x = io_space_bar_read::<u16>(self.bar0 + 0x18);
        println!("pcm {x:#X}");

        // pci_config_modify(self.bus, self.slot, 0, 0x1, |x| x | 0b101);
        println!("{self:#X?}");
        // TODO(colin): figure out what is happening here
        // I am going off of the bottom of this page
        // https://wiki.osdev.org/AC97
        // but I can't seem to write anything properly,
        // when I read back b I just get 255
        println!("Setting reset bit of audio...");
        let address_reset = self.bar1 + 0x1B;
        io_space_bar_write::<u8>(address_reset, 2);
        // pci_config_modify(self.bus, self.slot, 0, 0x1, |x| x & !0b11);
        let b = io_space_bar_read::<u8>(address_reset);
        print!("[{b}");

        loop {
            let b = io_space_bar_read::<u8>(address_reset);
            println!("[{b}");
            if io_space_bar_read::<u8>(address_reset) & 0b10 == 0 {
                // println!("Bit was cleared!");
                break;
            }
        }
        // pci_config_modify(self.bus, self.slot, 0, 0x1, |x| x | 0b101);

        println!("Writing BDL pos");
        let address = self.bar1 + 0x10 + 0x0;
        let mut l = BUFFER_DESCRIPTOR_LIST.lock();
        {
            let j = l[1].physical_addr;
            for i in 0..100 {
                let b = (j + i * 2) as *const u16;
                print!("[{:#X}]", unsafe { *b });
            }
        }
        let raw_addr: *mut [BufferDescriptor; 32] = &raw mut *l;
        debug_assert!(raw_addr as u64 <= u32::MAX as u64);
        let addr_truncated = raw_addr as u32;
        println!("BDL^{:#X}", addr_truncated);
        for i in 0..64 {
            let h = (addr_truncated + i * 2) as *const u16;
            // print!("<{:04X}>", unsafe { *h });
        }
        io_space_bar_write(address, addr_truncated);

        println!("Writing number of last valid buffer");
        let address_last_valid_idx = self.bar1 + 0x10 + 0x05;
        io_space_bar_write::<u8>(address_last_valid_idx, 22);

        println!("Setting bit for transferring data");
        io_space_bar_write::<u8>(address_reset, 1);

        let mut w = 0;
        loop {
            io_space_bar_write::<u8>(address_last_valid_idx, 25);
            let x = io_space_bar_read::<u16>(self.bar1 + 0x16);
            if x & 2 == 1 || x & 1 == 1 {
                println!("F done {x:#b}");
                break;
            } else {
                w += 1;
                if w > 20 {
                    // break;
                }
                let y = io_space_bar_read::<u8>(self.bar1 + 0x14);
                let z = io_space_bar_read::<u16>(self.bar1 + 0x18);
                println!("F {x:#b} {y} ({z}/{})", SAMPLE.len() / 2);
            }
        }
    }
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

                    println!(
                        "I {} {}",
                        full_header.interrupt_pin, full_header.interrupt_line
                    );

                    audio = Some(AudioAc97 {
                        bus,
                        slot: device,
                        bar0: (full_header.base_addresses[0] & 0xFFFFFFFC) as u16,
                        bar1: (full_header.base_addresses[1] & 0xFFFFFFFC) as u16,
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

                if h.header_type == 1 {
                    println!("{h:#X?}");
                    return;
                }

                if false && h.header_type == 0 {
                    let h = parse_header_type0(bus, device, 0, h);

                    if h.headhead.class_code == 0x4 {
                        println!("{h:#X?}");
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
