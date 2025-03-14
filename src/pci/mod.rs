use core::{mem::transmute, ptr::slice_from_raw_parts};

use audio_ac97::{music_loop::MusicLoop, AudioAc97};
use headers::{parse_header_common, parse_header_type0};
use io::pci_config_read_word;
use pluggable_interrupt_os::println;

use crate::PhysAllocator;

mod audio_ac97;
mod headers;
mod io;

// Everything here makes extensive use of: https://wiki.osdev.org/PCI
// as well as some of the references from the bottom of that page.

static WAV_DATA: &[u8] = include_bytes!("../../../../../../Documents/something_like_megaman2.raw");
// static WAV_DATA: &[u8] = include_bytes!("../../../../../../Documents/snippet.raw");

struct PciDevices {
    ac97: Option<AudioAc97>,
    // We could add more devices here, if we wanted
}

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

    let ac97 = init_audio().unwrap();

    let mut music = MusicLoop::new(phys_alloc, wav, ac97);

    music.play();
    loop {
        music.wind();
    }
}

fn scan_pci() -> PciDevices {
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
                    audio = Some(AudioAc97::new(bus, device, full_header));

                    // println!(
                    //     "I {} {} {:#018b} {:#018b}",
                    //     full_header.interrupt_pin,
                    //     full_header.interrupt_line,
                    //     full_header.headhead.command,
                    //     full_header.headhead.status
                    // );
                    //
                    // // Here I was chainging the interupt line, I don't think I ever figure out how
                    // // to use interrupts though
                    // pci_config_modify(bus, device, 0, 0xF, |x| {
                    //     let tmp = x & !0xFF_FF;
                    //     let tmp = tmp | 0x00_5C;
                    //
                    //     println!("modifying {x:#b} to {tmp:#b}");
                    //
                    //     tmp
                    // });
                    //
                    // {
                    //     let (_, _, pin, line) =
                    //         quarter_u32(pci_config_read_u32(bus, device, 0, 0xF));
                    //     println!("I {} {}", pin, line,);
                    // }

                    // let x = boot_info.physical_memory_offset;
                    // audio = Some(AudioAc97 {
                    //     bus,
                    //     slot: device,
                    //     mixer_port_base: (full_header.base_addresses[0] & 0xFFFFFFFC) as u16,
                    //     buffer_port_base: (full_header.base_addresses[1] & 0xFFFFFFFC) as u16,
                    // });
                }
            }
        }
    }

    PciDevices { ac97: audio }
}
