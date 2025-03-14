use core::mem::transmute;

use bootloader::BootInfo;
use headers::{half_u32, parse_header_common, parse_header_type0, quarter_u32};
use io::{
    io_space_bar_read, io_space_bar_write, pci_config_modify, pci_config_read_u32,
    pci_config_read_word, pci_config_write_u32,
};
use lazy_static::lazy_static;
use pluggable_interrupt_os::{print, println};
use spin::Mutex;
use volatile::Volatile;
use x86_64::instructions::port::Port;

mod headers;
mod io;

// const MAGICAL_SUBRACT_ME_FOR_PHYS_ADDR: u64 = 0x0021F000;
const MAGICAL_SUBRACT_ME_FOR_PHYS_ADDR: u64 = 0x0021E000;

/*
Everything here makes extensive reference of: https://wiki.osdev.org/PCI
And some: https://wiki.osdev.org/AC97
As well as other linked references
*/

pub fn init_pci(boot_info: &'static BootInfo) {
    // check_all();
    let a = init_audio(boot_info).unwrap();
    a.play();
}

const WAVSIZE: usize = 0x1000;

lazy_static! {
    static ref SAMPLE: [Volatile<i16>; WAVSIZE * 2] = {
        let mut x = [0i16; WAVSIZE * 2];
        for i in 0..x.len() {
            x[i] = (i as i16).wrapping_mul(79);
        }
        x.map(Volatile::new)
    };
}

// #[repr(packed)]
#[repr(align(4))]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
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
        let raw_addr: *const [Volatile<i16>; WAVSIZE * 2] = &raw const *SAMPLE;

        let phys_raw_addr = raw_addr as u32 - MAGICAL_SUBRACT_ME_FOR_PHYS_ADDR as u32;

        // debug_assert!(raw_addr as u64 <= u32::MAX as u64);
        // let addr_truncated = raw_addr as u32;
        //
        // debug_assert!(raw_addr == unsafe { transmute(addr_truncated as usize) });

        BufferDescriptor {
            physical_addr: phys_raw_addr,
            num_samples: WAVSIZE as u16,
            control: 0,
        }
    }
}

lazy_static! {
    static ref BUFFER_DESCRIPTOR_LIST: Mutex<[Volatile<BufferDescriptor>; 32]> =
        Mutex::new([BufferDescriptor::square(); 32].map(Volatile::new));
}

#[derive(Debug)]
struct AudioAc97 {
    phys_mem_offset: u64,
    bus: u8,
    slot: u8,
    // reset, device selection, volume control
    bar0: u16,
    // audio data
    bar1: u16,
}

impl AudioAc97 {
    fn play(&self) {
        println!("{self:#X?}");

        pci_config_modify(self.bus, self.slot, 0, 0x1, |x| x | 0b101);
        // pci_config_modify(self.bus, self.slot, 0, 0xF, |x| {
        //     x | 0b00000000_00000000_00000011_00000011
        // });

        io_space_bar_write::<u32>(self.bar1 + 0x2C, 0x2);

        for i in 0..100_000 {
            print!("");
        }

        io_space_bar_write::<u16>(self.bar0 + 0x00, 0xFF);

        for i in 0..100_000 {
            print!("");
        }

        let samp_front = io_space_bar_read::<u16>(self.bar0 + 0x2C);
        let samp_surr = io_space_bar_read::<u16>(self.bar0 + 0x2E);
        let samp_lfe = io_space_bar_read::<u16>(self.bar0 + 0x30);
        let samp_lr = io_space_bar_read::<u16>(self.bar0 + 0x32);
        println!("samp {samp_front} {samp_surr} {samp_lfe} {samp_lr}");

        {
            let (pcm, master, aux) = (
                io_space_bar_read::<u16>(self.bar0 + 0x18), //PCM
                io_space_bar_read::<u16>(self.bar0 + 0x02), //Master
                io_space_bar_read::<u16>(self.bar0 + 0x04), //Aux output
            );

            println!("PCM,MASTER,AUX {pcm:#X} {master:#X} {aux:#X} {pcm:#b} {master:#b} {aux:#b}")
        }
        {
            let glob = io_space_bar_read::<u32>(self.bar1 + 0x2C);
            // note that this is just 0. Does that mean device is is reset?
            println!("global control {glob:#b}");
        }

        println!("Setting reset bit of audio...");
        let address_reset = self.bar1 + 0x1B;
        io_space_bar_write::<u8>(address_reset, 2);
        // pci_config_modify(self.bus, self.slot, 0, 0x1, |x| x & !0b11);
        // let b = io_space_bar_read::<u8>(address_reset);
        // print!("[{b}");
        for i in 0..100_000 {
            print!("");
        }

        loop {
            let b = io_space_bar_read::<u8>(address_reset);
            println!("[{b}");
            if io_space_bar_read::<u8>(address_reset) & 0x2 != 0x2 {
                println!("Bit was cleared!");
                break;
            }
        }
        // pci_config_modify(self.bus, self.slot, 0, 0x1, |x| x | 0b101);

        // set volumes
        io_space_bar_write::<u16>(self.bar0 + 0x18, 0x0); //PCM
        io_space_bar_write::<u16>(self.bar0 + 0x02, 0x0); //Master

        io_space_bar_write::<u16>(self.bar0 + 0x04, 0x2020); //Aux output
        for i in 0..100_000 {
            print!("");
        }

        io_space_bar_write::<u8>(address_reset, 0x0);

        println!("Writing BDL pos");
        let address = self.bar1 + 0x10 + 0x0;
        let mut l = BUFFER_DESCRIPTOR_LIST.lock();
        // {
        //     let j = l[1].physical_addr;
        //     for i in 0..100 {
        //         let b = (j + i * 2) as *const u16;
        //         print!("[{:#X}]", unsafe { *b });
        //     }
        // }
        let raw_addr: *mut [Volatile<BufferDescriptor>; 32] = &raw mut *l;
        println!("RAW      {:#018X}", raw_addr as u64);
        println!("RAW PHYS {:#018X}", raw_addr as u64 + self.phys_mem_offset);
        let raw_phys_addr = raw_addr as u64 - MAGICAL_SUBRACT_ME_FOR_PHYS_ADDR;
        debug_assert!(raw_phys_addr as usize <= u32::MAX as usize);
        let addr_truncated = raw_phys_addr as u32;
        println!("BDL^{:#X}", addr_truncated);
        // for i in 0..64 {
        //     let h = (addr_truncated + i * 2) as *const u16;
        //     print!("<{:04X}>", unsafe { *h });
        // }
        // debug_assert!(raw_addr == unsafe { transmute(addr_truncated as usize) });

        io_space_bar_write(address, addr_truncated);

        println!("Writing number of last valid buffer");
        let address_last_valid_idx = self.bar1 + 0x10 + 0x05;
        io_space_bar_write::<u8>(address_last_valid_idx, 10);

        io_space_bar_write::<u16>(self.bar1 + 0x16, 0x1C);

        // loop {}

        // IMPORTANT:
        // This is the line that gives Qemu a "volume meter" in pavucontrol
        // before this, there is no volume indicator, but after this there is!
        // unfortunately it does not move at all
        println!("Setting bit for transferring data");
        io_space_bar_write::<u8>(address_reset, 0b1);

        let mut w = 0;
        loop {
            let y = io_space_bar_read::<u8>(self.bar1 + 0x14);
            io_space_bar_write::<u8>(address_last_valid_idx, y.wrapping_sub(1) & 0b11111);
            let x = io_space_bar_read::<u16>(self.bar1 + 0x16);

            if x & 2 == 1 || x & 1 == 1 {
                println!("F done {x:#b}");
                break;
            } else {
                w += 1;
                if w > 4 {
                    break;
                }
                let y = io_space_bar_read::<u8>(self.bar1 + 0x14);
                let z = io_space_bar_read::<u16>(self.bar1 + 0x18);

                let next = io_space_bar_read::<u8>(self.bar1 + 0x1A);
                let control = io_space_bar_read::<u8>(self.bar1 + 0x1B);

                println!("F {x:#b} {y} ({z:#06X}/) {next} {control:#010b}");
            }
        }
    }
}

fn init_audio(boot_info: &'static BootInfo) -> Option<AudioAc97> {
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
                        "I {} {} {:#018b} {:#018b}",
                        full_header.interrupt_pin,
                        full_header.interrupt_line,
                        full_header.headhead.command,
                        full_header.headhead.status
                    );

                    pci_config_modify(bus, device, 0, 0xF, |x| {
                        let tmp = x & !0xFF_FF;
                        let tmp = tmp | 0x00_5C;

                        println!("modifying {x:#b} to {tmp:#b}");

                        tmp
                    });

                    {
                        let (_, _, pin, line) =
                            quarter_u32(pci_config_read_u32(bus, device, 0, 0xF));
                        println!("I {} {}", pin, line,);
                    }

                    // let x = boot_info.physical_memory_offset;
                    audio = Some(AudioAc97 {
                        phys_mem_offset: boot_info.physical_memory_offset,
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
