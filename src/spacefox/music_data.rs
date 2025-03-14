use core::slice::from_raw_parts;

use lazy_static::lazy_static;

// https://users.rust-lang.org/t/can-i-conveniently-compile-bytes-into-a-rust-program-with-a-specific-alignment/24049/2
// I found this really cool trick for aligning include_bytes correctly!
// I would have been totally lost without this, many thanks to ExpHP!!!

lazy_static! {
    pub static ref WAV_DATA_SAMPLES: &'static [i16] = {
        // This struct is generic in Bytes to admit unsizing coercions.
        #[repr(C)] // guarantee 'bytes' comes after '_align'
        struct AlignedTo<Align, Bytes: ?Sized> {
            _align: [Align; 0],
            bytes: Bytes,
        }

        // dummy static used to create aligned data
        static ALIGNED: &'static AlignedTo<i16, [u8]> = &AlignedTo {
            _align: [],
            bytes: *include_bytes!("../../../../../../Documents/snippet.raw"),
            // bytes: *include_bytes!("../../../../../../Documents/something_like_megaman2.raw"),
        };

        static ALIGNED_BYTES: &'static [u8] = &ALIGNED.bytes;

        unsafe {
            from_raw_parts(
                ALIGNED_BYTES.as_ptr() as *const i16,
                ALIGNED_BYTES.len() / 2,
            )
        }
    };
}
