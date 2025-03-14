use volatile::Volatile;

use crate::phys_alloc::{DualPtr32, PhysAllocator};

use super::{
    AudioAc97, BufferDescriptor, BufferDescriptorList, BYTES_PER_BUF, NUM_BUFFERS, SAMPLES_PER_BUF,
};

const SAMPLES_IN_BLOB: usize = SAMPLES_PER_BUF as usize * NUM_BUFFERS;
type SamplesBlob = [Volatile<i16>; SAMPLES_IN_BLOB];

pub struct MusicLoop<'a> {
    ac97: AudioAc97,
    music_data: &'a [i16],
    music_data_read_head: usize,
    samples_blob: DualPtr32<'a, SamplesBlob>,
    buffer_descriptor_list: DualPtr32<'a, BufferDescriptorList>,
    last_buffer_filled: u8,
}

impl<'a> MusicLoop<'a> {
    // Assumes audio is in 16 bit samples
    pub fn new(phys_alloc: &mut PhysAllocator, music_data: &'a [i16], ac97: AudioAc97) -> Self {
        let samples_blob = phys_alloc.alloc32::<SamplesBlob>();
        let buffer_descriptor_list = phys_alloc.alloc32::<BufferDescriptorList>();

        for i in 0..NUM_BUFFERS {
            buffer_descriptor_list.rw_virt[i] = Volatile::new(BufferDescriptor {
                physical_addr: samples_blob.r_phys + BYTES_PER_BUF * i as u32,
                num_samples: SAMPLES_PER_BUF as u16,
                control: 0, // no interrupt, no stopping
            })
        }

        let mut me = Self {
            ac97,
            music_data,
            music_data_read_head: 0,
            samples_blob,
            buffer_descriptor_list,
            last_buffer_filled: 0,
        };

        me.fill_sound_blob();

        me
    }

    // this is called in new, when any MusicLoop is created
    // because we have to ensure this happens before play
    fn fill_sound_blob(&mut self) {
        for i in 0..self.samples_blob.rw_virt.len() {
            self.samples_blob.rw_virt[i] =
                Volatile::new(self.music_data[i % self.music_data.len()]);

            self.music_data_read_head += 1;
            if self.music_data_read_head >= self.music_data.len() {
                self.music_data_read_head = 0;
            }
        }
        // samples_blob strecthes accross all buffers, so after the
        // for loop all buffers are valid
        self.last_buffer_filled = NUM_BUFFERS as u8 - 1;
    }

    // starts the loop
    pub fn play(&mut self) {
        self.ac97.init();
        self.ac97
            .begin_transfer(self.buffer_descriptor_list.r_phys, NUM_BUFFERS as u8 - 1);
    }

    // must be called repeatedly after the transfer is started
    // to continue to supply audio frames
    pub fn wind(&mut self) {
        debug_assert!(NUM_BUFFERS == 32); // if this changes, the bit mask won't work;
        const MOD32_MASK: u8 = 0b11111;
        let current_buf: u8 = self.ac97.get_current_buffer();

        // if buf != self.last_buffer_filled {
        //     println!("Bailing {buf}!");
        //     return;
        // }

        let fill_to = current_buf.wrapping_sub(1) & MOD32_MASK;

        // println!("Filling {current_buf}, {}", self.last_buffer_filled);

        let mut i = (self.last_buffer_filled + 1) & MOD32_MASK;
        while i != current_buf {
            // println!(
            //     "Filling from [{}..{}), dataon {}",
            //     (self.last_buffer_filled + 1) & MOD32_MASK,
            //     current_buf,
            //     self.music_data_read_head
            // );
            let mut buf_write_head = 0;
            while buf_write_head < SAMPLES_PER_BUF {
                let write_pos = i as usize * SAMPLES_PER_BUF as usize + buf_write_head as usize;
                // println!("w {}/{}", write_pos, self.samples_blob.rw_virt.len());
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
        self.last_buffer_filled = fill_to;
    }
}
