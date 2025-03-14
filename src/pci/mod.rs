use core::{mem::transmute, ptr::slice_from_raw_parts};

use headers::{parse_header_common, parse_header_type0, quarter_u32};
use io::{
    io_space_bar_read, io_space_bar_write, pci_config_modify, pci_config_read_u32,
    pci_config_read_word,
};
use pluggable_interrupt_os::{print, println};
use volatile::Volatile;

use crate::{phys_alloc::DualPtr32, PhysAllocator};

mod headers;
mod io;

static WAV_DATA: &[u8] = include_bytes!("../../../../../../Documents/something_like_megaman2.raw");
// static WAV_DATA: &[u8] = include_bytes!("../../../../../../Documents/snippet.raw");

// https://larsimmisch.github.io/pyalsaaudio/terminology.html
// Fixed by AC97 card
const NUM_BUFFERS: usize = 32;
const MAX_SAMPLES_PER_BUF: u16 = 0xFFFE;
// Defaults that we won't change
const SAMPLE_SIZE: usize = size_of::<i16>();
const NUM_CHANNELS: usize = 2;
// good to know
const SAMPLES_PER_BUF: u16 = MAX_SAMPLES_PER_BUF;
const BYTES_PER_BUF: u32 = SAMPLES_PER_BUF as u32 * SAMPLE_SIZE as u32;
// const SAMPLES_PER_FRAME: usize = NUM_CHANNELS;
// const FRAMES_PER_BUF: usize = SAMPLES_PER_BUF as usize / SAMPLES_PER_FRAME;

#[repr(packed)]
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
type BDL = [Volatile<BufferDescriptor>; NUM_BUFFERS];

const SAMPLES_IN_BLOB: usize = SAMPLES_PER_BUF as usize * NUM_BUFFERS;
type SamplesBlob = [Volatile<i16>; SAMPLES_IN_BLOB];
struct MusicLoop<'a> {
    ac97: AudioAc97,
    music_data: &'a [i16],
    music_data_read_head: usize,
    samples_blob: DualPtr32<'a, SamplesBlob>,
    buffer_descriptor_list: DualPtr32<'a, BDL>,
    last_buffer_filled: u8,
}

impl<'a> MusicLoop<'a> {
    fn play(&mut self) {
        self.fill_sound_blob();

        self.ac97.init();
        self.ac97
            .begin_transfer(self.buffer_descriptor_list.r_phys, NUM_BUFFERS as u8 - 1);
    }

    // should be done before the transfer is started
    fn fill_sound_blob(&mut self) {
        for i in 0..self.samples_blob.rw_virt.len() {
            self.samples_blob.rw_virt[i] =
                Volatile::new(self.music_data[i % self.music_data.len()]);
        }
    }

    // must be called repeatedly after the transfer is started
    // to continue to supply audio frames
    fn wind(&mut self) {
        debug_assert!(NUM_BUFFERS == 32); // if this changes, the bit mask won't work;
        const MOD32_MASK: u8 = 0b11111;
        let buf: u8 = self.ac97.get_current_buffer();

        let fill_to = buf.wrapping_sub(1) & MOD32_MASK;

        let mut i = (self.last_buffer_filled + 1) & MOD32_MASK;
        while i != fill_to {
            let mut buf_write_head = 0;
            while buf_write_head < SAMPLES_PER_BUF {
                let write_pos = i as usize * BYTES_PER_BUF as usize + buf_write_head as usize;
                self.samples_blob.rw_virt[write_pos] =
                    Volatile::new(self.music_data[self.music_data_read_head]);
                buf_write_head += 1;
                self.music_data_read_head += 1;
                if self.music_data_read_head >= self.music_data.len() {
                    self.music_data_read_head = 0;
                }
            }

            i = (i + 1) & MOD32_MASK;
        }

        self.ac97.set_filled_up_to(fill_to);
    }
}

#[derive(Debug)]
struct AudioAc97 {
    bus: u8,
    slot: u8,

    // Native Audio Mixer registers
    // reset, device selection, volume control
    mixer_port_base: u16,

    // Native Audio Bus Master registers
    // manages the ring buffer
    buffer_port_base: u16,
}

impl AudioAc97 {
    // buffer_port_base / nabm offsets
    const GLOBAL_CONTROL: u16 = 0x2C;
    const PCM_OUT: u16 = 0x10;
    const LAST_VALID_ENTRY_OFFSET: u16 = 0x05;
    const CURRENT_PROCESSED_ENTRY_OFFSET: u16 = 0x04;
    const TRANSFER_CONTROL_OFFSET: u16 = 0x0B;

    fn set_filled_up_to(&self, buf: u8) {
        debug_assert!((buf as usize) < NUM_BUFFERS);

        let last_valid_entry =
            self.buffer_port_base + Self::PCM_OUT + Self::LAST_VALID_ENTRY_OFFSET;
        io_space_bar_write::<u8>(last_valid_entry, buf);
    }

    fn get_current_buffer(&self) -> u8 {
        let buf = io_space_bar_read::<u8>(
            self.buffer_port_base + Self::PCM_OUT + Self::CURRENT_PROCESSED_ENTRY_OFFSET,
        );
        debug_assert!((buf as usize) < NUM_BUFFERS);
        buf
    }

    // I reffered heavily to https://wiki.osdev.org/AC97
    // and peeked a few times at the refernced BleskOS driver.
    // I'm not sure how to properly cite BleskOS, as I didn't directly copy code
    // (it is in C, so I quite literally could not have direcly copied anyhing),
    // but I did copy some of the order and wait timings of initialization
    // and cleared up some, in my opinion, misleading things on the osdev wiki
    fn init(&self) {
        // Blesk inserts several 'wait's in its code.
        // This makes me worry that if I don't do the same,
        // things may randomly fail if I happen to write too fast
        // while the card is resetting something.
        // So I attempt to wait in a few places.
        fn wait() {
            //TODO(colin), maybe set this to tick boundaries?
        }

        // https://wiki.osdev.org/AC97#Detecting_AC97_sound_card
        // the wiki says we have to write these bits to the AC97 pci control register before
        // anything else
        pci_config_modify(self.bus, self.slot, 0, 0x1, |x| x | 0b101);

        // Blesk does this in a different spot, osdev doesn't say to do it at all
        io_space_bar_write::<u16>(self.mixer_port_base + 0x00, 0xFFFF);

        // This is not from the wiki, but Blesk does this first
        const RESUME_OPERATION: u32 = 1 << 1;
        io_space_bar_write::<u32>(
            self.buffer_port_base + Self::GLOBAL_CONTROL,
            RESUME_OPERATION,
        );

        // Blesk waits after it writes to global control
        wait();

        // osdev.org says to set our volumes now, 0x0 is full volume
        // TODO(colin): clean up magic constants here
        io_space_bar_write::<u16>(self.mixer_port_base + 0x18, 0x0); //PCM
        io_space_bar_write::<u16>(self.mixer_port_base + 0x02, 0x0); //Master
        io_space_bar_write::<u16>(self.mixer_port_base + 0x04, 0x0); //Aux output

        // osdev.org says: "Set reset bit of output channel
        // (NABM register 0x1B, value 0x2) and wait for card to clear it""
        let pcm_out_transfer =
            self.buffer_port_base + Self::PCM_OUT + Self::TRANSFER_CONTROL_OFFSET;
        io_space_bar_write::<u8>(pcm_out_transfer, 0b10);
        while io_space_bar_read::<u8>(pcm_out_transfer) & 0b10 != 0 {
            wait();
        }

        // to start playing a sound osdev.org says we still have to do:
        // - Write physical position of BDL to Buffer Descriptor Base Address register (NABM register 0x10)
        // - Write number of last valid buffer entry to Last Valid Entry register (NABM register 0x15)
        // - Set bit for transfering data (NABM register 0x1B, value 0x1)
        // but here I move that to the begin_transfer function
    }

    // init() must be called first!
    fn begin_transfer(&self, bdl_phys_addr: u32, initial_valid_bufs: u8) {
        debug_assert!((initial_valid_bufs as usize) < NUM_BUFFERS);
        // to start playing a sound osdev.org says we still have to:
        // - Write physical position of BDL to Buffer Descriptor Base Address register (NABM register 0x10)
        // - Write number of last valid buffer entry to Last Valid Entry register (NABM register 0x15)
        // - Set bit for transfering data (NABM register 0x1B, value 0x1)

        io_space_bar_write(self.buffer_port_base + 0x10, bdl_phys_addr);

        let last_valid_entry =
            self.buffer_port_base + Self::PCM_OUT + Self::LAST_VALID_ENTRY_OFFSET;
        io_space_bar_write::<u8>(last_valid_entry, initial_valid_bufs);

        // This is the line that gives Qemu a "volume meter" in pavucontrol
        // before this, there is no volume indicator, but after this there is!
        // If the BDL or the data that any entry in it points to is
        // set up incorrectly, the volume indicator for Qemu should show up,
        // but not show any activity.
        const TRANSFER_SOUND_DATA: u8 = 1 << 0;
        let pcm_out_transfer =
            self.buffer_port_base + Self::PCM_OUT + Self::TRANSFER_CONTROL_OFFSET;
        io_space_bar_write::<u8>(pcm_out_transfer, TRANSFER_SOUND_DATA);
    }
}

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
    let a = init_audio().unwrap();
    println!("FREE: {}", phys_alloc.kb_free());

    a.play(bdl.r_phys);
}

impl AudioAc97 {
    fn play(&self, bdl_phys_loc: u32) {
        println!("{self:#X?}");

        pci_config_modify(self.bus, self.slot, 0, 0x1, |x| x | 0b101);
        // pci_config_modify(self.bus, self.slot, 0, 0xF, |x| {
        //     x | 0b00000000_00000000_00000011_00000011
        // });

        io_space_bar_write::<u32>(self.buffer_port_base + 0x2C, 0x2);

        for i in 0..100_000 {
            print!("");
        }

        io_space_bar_write::<u16>(self.mixer_port_base + 0x00, 0xFF);

        let samp_front = io_space_bar_read::<u16>(self.mixer_port_base + 0x2C);
        let samp_surr = io_space_bar_read::<u16>(self.mixer_port_base + 0x2E);
        let samp_lfe = io_space_bar_read::<u16>(self.mixer_port_base + 0x30);
        let samp_lr = io_space_bar_read::<u16>(self.mixer_port_base + 0x32);
        println!("samp {samp_front} {samp_surr} {samp_lfe} {samp_lr}");

        {
            let (pcm, master, aux) = (
                io_space_bar_read::<u16>(self.mixer_port_base + 0x18), //PCM
                io_space_bar_read::<u16>(self.mixer_port_base + 0x02), //Master
                io_space_bar_read::<u16>(self.mixer_port_base + 0x04), //Aux output
            );

            println!("PCM,MASTER,AUX {pcm:#X} {master:#X} {aux:#X} {pcm:#b} {master:#b} {aux:#b}")
        }
        {
            let glob = io_space_bar_read::<u32>(self.buffer_port_base + 0x2C);
            // note that this is just 0. Does that mean device is is reset?
            println!("global control {glob:#b}");
        }

        println!("Setting reset bit of audio...");
        let address_reset = self.buffer_port_base + 0x1B;
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
        io_space_bar_write::<u16>(self.mixer_port_base + 0x18, 0x0); //PCM
        io_space_bar_write::<u16>(self.mixer_port_base + 0x02, 0x0); //Master

        io_space_bar_write::<u16>(self.mixer_port_base + 0x04, 0x2020); //Aux output
        for i in 0..100_000 {
            print!("");
        }

        io_space_bar_write::<u8>(address_reset, 0x0);

        io_space_bar_write(self.buffer_port_base + 0x10, bdl_phys_loc);

        println!("Writing number of last valid buffer");
        let address_last_valid_idx = self.buffer_port_base + 0x10 + 0x05;
        io_space_bar_write::<u8>(address_last_valid_idx, 10);

        // io_space_bar_write::<u16>(self.buffer_port_base + 0x16, 0x1C);

        // IMPORTANT:
        // This is the line that gives Qemu a "volume meter" in pavucontrol
        // before this, there is no volume indicator, but after this there is!
        println!("Setting bit for transferring data");
        io_space_bar_write::<u8>(address_reset, 0b1);

        let mut w = 0;
        loop {
            let y = io_space_bar_read::<u8>(self.buffer_port_base + 0x14);
            io_space_bar_write::<u8>(address_last_valid_idx, y.wrapping_sub(1) & 0b11111);
            let x = io_space_bar_read::<u16>(self.buffer_port_base + 0x16);

            if x & 2 == 1 || x & 1 == 1 {
                println!("F done {x:#b}");
                break;
            } else {
                w += 1;
                if w > 1 {
                    // break;
                }
                let y = io_space_bar_read::<u8>(self.buffer_port_base + 0x14);
                let z = io_space_bar_read::<u16>(self.buffer_port_base + 0x18);

                let next = io_space_bar_read::<u8>(self.buffer_port_base + 0x1A);
                let control = io_space_bar_read::<u8>(self.buffer_port_base + 0x1B);

                println!("F {x:#b} {y} ({z:#06X}/) {next} {control:#010b}");
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
                        mixer_port_base: (full_header.base_addresses[0] & 0xFFFFFFFC) as u16,
                        buffer_port_base: (full_header.base_addresses[1] & 0xFFFFFFFC) as u16,
                    });
                }
            }
        }
    }

    audio
}

// struct Vendor(u16);
// impl Vendor {
//     fn id(&self) -> Option<u16> {
//         if self.0 == 0xFFFF {
//             None
//         } else {
//             Some(self.0)
//         }
//     }
// }
// fn pci_check_vendor(bus: u8, slot: u8) -> Vendor {
//     Vendor(pci_config_read_word(bus, slot, 0, 0))
// }
//
// fn check_all() {
//     for bus in 0..=255 {
//         for device in 0..32 {
//             let v = pci_check_vendor(bus, device);
//             if let Some(id) = v.id() {
//                 let h = parse_header_common(bus, device, 0);
//                 debug_assert!(id == h.vendor_id);
//
//                 println!(
//                     "BUS{bus}   DEVICE{device}   V:D {:#06X}:{:#06X}    CLS:SUBCLS {:#04X}:{:#04X}   HTY{:#04X}",
//                     h.vendor_id, h.device_id, h.class_code, h.subclass, h.header_type
//                 );
//
//                 if h.header_type == 1 {
//                     println!("{h:#X?}");
//                     return;
//                 }
//
//                 if false && h.header_type == 0 {
//                     let h = parse_header_type0(bus, device, 0, h);
//
//                     if h.headhead.class_code == 0x4 {
//                         println!("{h:#X?}");
//                         println!("{:#013b}", h.headhead.command);
//
//                         // https://wiki.osdev.org/AC97#Detecting_AC97_sound_card
//                         pci_config_modify(bus, device, 0, 0x1, |x| x | 0b101);
//
//                         let (_, command) = half_u32(pci_config_read_u32(bus, device, 0, 1));
//                         println!("{:#013b}", command);
//                     }
//                 }
//             }
//         }
//     }
// }
