use core::{mem::transmute, ptr::slice_from_raw_parts};

use headers::{half_u32, parse_header_common, parse_header_type0, quarter_u32};
use io::{
    io_space_bar_read, io_space_bar_write, pci_config_modify, pci_config_read_u32,
    pci_config_read_word,
};
use lazy_static::lazy_static;
use pluggable_interrupt_os::{print, println};
use volatile::Volatile;

use crate::PhysAllocator;

mod headers;
mod io;

static WAV_DATA: &[u8] = include_bytes!("../../../../../../Documents/something_like_megaman2.raw");
// static WAV_DATA: &[u8] = include_bytes!("../../../../../../Documents/snippet.raw");

/*
Everything here makes extensive reference of: https://wiki.osdev.org/PCI
And some: https://wiki.osdev.org/AC97
As well as other linked references
*/

pub fn init_pci(phys_alloc: &mut PhysAllocator) {
    let l = WAV_DATA.len();
    println!("WOW! {} mebibytes!", l / 1024 / 1024);
    debug_assert!(l % 2 == 0);
    let wav_raw = slice_from_raw_parts::<i16>(WAV_DATA.as_ptr() as *const i16, l / 2);
    let wav = unsafe { &*wav_raw };
    debug_assert!(
        WAV_DATA.len() == wav.len() * 2,
        "{} != {}",
        WAV_DATA.len(),
        wav.len() * 2
    );
    debug_assert!({
        // relies on little endian
        let thing = (WAV_DATA[1] as u16) << 8 | WAV_DATA[0] as u16;
        wav[0] == unsafe { transmute(thing) }
    });

    type BDL = [Volatile<BufferDescriptor>; 32];
    let bdl = phys_alloc.alloc32::<BDL>();

    // https://larsimmisch.github.io/pyalsaaudio/terminology.html
    // Fixed by AC97 card
    const NUM_BUFFERS: usize = 32;
    const MAX_SAMPLES_PER_BUF: u16 = 0xFFFE;
    // Defaults that we won't change
    const SAMPLE_SIZE: usize = size_of::<i16>();
    const NUM_CHANNELS: usize = 2;

    // calculuation
    const SAMPLES_PER_BUF: u16 = MAX_SAMPLES_PER_BUF;
    const BYTES_PER_BUF: u32 = SAMPLES_PER_BUF as u32 * SAMPLE_SIZE as u32;
    // const SAMPLES_PER_FRAME: usize = NUM_CHANNELS;
    // const FRAMES_PER_BUF: usize = SAMPLES_PER_BUF as usize / SAMPLES_PER_FRAME;

    const SAMPLES_IN_BLOB: usize = SAMPLES_PER_BUF as usize * NUM_BUFFERS;
    type SamplesBlob = [Volatile<i16>; SAMPLES_IN_BLOB];

    let samples_blob = phys_alloc.alloc32::<SamplesBlob>();

    for i in 0..samples_blob.rw_virt.len() {
        if i >= wav.len() {
            break;
        }
        samples_blob.rw_virt[i] = Volatile::new(wav[i]);
    }
    for i in 0..NUM_BUFFERS {
        bdl.rw_virt[i] = Volatile::new(BufferDescriptor {
            physical_addr: samples_blob.r_phys + BYTES_PER_BUF * i as u32,
            num_samples: SAMPLES_PER_BUF as u16,
            control: 0,
        })
    }

    // check_all();
    let a = init_audio(phys_alloc).unwrap();
    println!("FREE: {}", phys_alloc.kb_free());

    a.play(bdl.r_phys);
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
    fn play(&self, bdl_phys_loc: u32) {
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

        io_space_bar_write(self.bar1 + 0x10, bdl_phys_loc);

        println!("Writing number of last valid buffer");
        let address_last_valid_idx = self.bar1 + 0x10 + 0x05;
        io_space_bar_write::<u8>(address_last_valid_idx, 10);

        io_space_bar_write::<u16>(self.bar1 + 0x16, 0x1C);

        // IMPORTANT:
        // This is the line that gives Qemu a "volume meter" in pavucontrol
        // before this, there is no volume indicator, but after this there is!
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
                if w > 1 {
                    // break;
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

fn init_audio(phys_alloc: &mut PhysAllocator) -> Option<AudioAc97> {
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
