use volatile::Volatile;

use crate::pci::io::{io_space_bar_read, io_space_bar_write, pci_config_modify};

mod music_loop;

// I reffered heavily to https://wiki.osdev.org/AC97
// and peeked a few times at the refernced BleskOS driver.
// See init() for details.

// https://larsimmisch.github.io/pyalsaaudio/terminology.html
// Fixed by AC97 card:
const NUM_BUFFERS: usize = 32;
const MAX_SAMPLES_PER_BUF: u16 = 0xFFFE;
// Defaults that we won't change:
const SAMPLE_SIZE: usize = size_of::<i16>();
const NUM_CHANNELS: usize = 2;
// Good to know:
const SAMPLES_PER_BUF: u16 = MAX_SAMPLES_PER_BUF;
const BYTES_PER_BUF: u32 = SAMPLES_PER_BUF as u32 * SAMPLE_SIZE as u32;
// const SAMPLES_PER_FRAME: usize = NUM_CHANNELS;
// const FRAMES_PER_BUF: usize = SAMPLES_PER_BUF as usize / SAMPLES_PER_FRAME;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[repr(packed)]
struct BufferDescriptor {
    physical_addr: u32,
    num_samples: u16,
    // From https://wiki.osdev.org/AC97#Buffer%20Descriptor%20List
    // Bit 15=Interrupt fired when data from this entry is transferred
    // Bit 14=Last entry of buffer, stop playing
    // Other bits=Reserved
    control: u16,
}
type BufferDescriptorList = [Volatile<BufferDescriptor>; NUM_BUFFERS];

#[derive(Debug)]
pub struct AudioAc97 {
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
        // while the card is resetting , which would
        // be annoying and hard to reproduce.
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
    // bdl_phys_addr should be the physical address (aligned to 4 bytes)
    // of a BufferDescriptorList you have already set up.
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
