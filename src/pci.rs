// https://wiki.osdev.org/PCI
/*

uint16_t pciConfigReadWord(uint8_t bus, uint8_t slot, uint8_t func, uint8_t offset) {
    uint32_t address;
    uint32_t lbus  = (uint32_t)bus;
    uint32_t lslot = (uint32_t)slot;
    uint32_t lfunc = (uint32_t)func;
    uint16_t tmp = 0;

    // Create configuration address as per Figure 1
    address = (uint32_t)((lbus << 16) | (lslot << 11) |
              (lfunc << 8) | (offset & 0xFC) | ((uint32_t)0x80000000));

    // Write out the address
    outl(0xCF8, address);
    // Read in the data
    // (offset & 2) * 8) = 0 will choose the first word of the 32-bit register
    tmp = (uint16_t)((inl(0xCFC) >> ((offset & 2) * 8)) & 0xFFFF);
    return tmp;
}

*/

use pluggable_interrupt_os::{print, println};
use x86_64::instructions::port::Port;

// slot and device seem to be used interchangably here
fn pci_config_read_word(bus: u8, slot: u8, func: u8, offset: u8) -> u16 {
    debug_assert!(slot <= 0b00011111);
    debug_assert!(func <= 0b00000111);

    let lbus = bus as u32;
    let lslot = slot as u32;
    let lfunc = func as u32;
    let loffset = offset as u32;

    let address = (lbus << 16) | (lslot << 11) | (lfunc << 8) | (loffset & 0xFC) | (0x80000000u32);

    let mut outl = Port::new(0xCF8);
    unsafe { outl.write(address) };

    let mut inl = Port::new(0xCFC);
    let tmp: u32 = unsafe { inl.read() };
    let sel_hi_shift = (loffset & 2) * 8;

    // println!("=0x{:X} >> 0x{:X}", tmp, sel_hi_shift);

    ((tmp >> sel_hi_shift) & 0xFFFF) as u16
}

fn pci_config_read_u32(bus: u8, slot: u8, func: u8, u32_array_index: u8) -> u32 {
    debug_assert!(slot <= 0b00011111);
    debug_assert!(func <= 0b00000111);
    debug_assert!(u32_array_index <= (256 / 4) as u8);

    let lbus = bus as u32;
    let lslot = slot as u32;
    let lfunc = func as u32;
    let loffset = (u32_array_index as u32) * 4;

    let address = (1 << 31) | (lbus << 16) | (lslot << 11) | (lfunc << 8) | loffset;

    let mut outl = Port::new(0xCF8);
    unsafe { outl.write(address) };

    let mut inl = Port::new(0xCFC);
    unsafe { inl.read() }
}

/*
uint16_t pciCheckVendor(uint8_t bus, uint8_t slot) {
    uint16_t vendor, device;
    /* Try and read the first configuration register. Since there are no
     * vendors that == 0xFFFF, it must be a non-existent device. */
    if ((vendor = pciConfigReadWord(bus, slot, 0, 0)) != 0xFFFF) {
       device = pciConfigReadWord(bus, slot, 0, 2);
       . . .
    } return (vendor);
}
*/

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
    let vendor = pci_config_read_word(bus, slot, 0, 0);
    if vendor != 0xFFFF {
        let device = pci_config_read_word(bus, slot, 0, 2);
    } else {
    }

    Vendor(vendor)
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
                let h = parse_header(bus, device, 0);
                debug_assert!(id == h.vendor_id);

                println!(
                    "BUS{bus}   DEVICE{device}   V:D {:#X}:{:#X}    CLS:SUBCLS {:#X}:{:#X}",
                    h.vendor_id, h.device_id, h.class_code, h.subclass,
                );
            }
        }
    }
}

pub fn init_pci() {
    check_all();
}

#[derive(Debug)]
struct PciHeaderCommon {
    device_id: u16,
    vendor_id: u16,

    status: u16,
    command: u16,

    class_code: u8,
    subclass: u8,
    prog_if: u8,
    revision_id: u8,

    built_in_self_test: u8,
    header_type: u8,
    latency_timer: u8,
    cache_line_size: u8,
}

// Here we return the the tuple
// as if we split the binary representation of
// the number in half:
// high place values on left, low place values on right
fn half_u32(x: u32) -> (u16, u16) {
    let low = x & 0xFFFF;
    let high = (x >> 16) & 0xFFFF;
    (high as u16, low as u16)
}

fn half_u16(x: u16) -> (u8, u8) {
    let low = x & 0xFF;
    let high = (x >> 8) & 0xFF;
    (high as u8, low as u8)
}

fn quarter_u32(mut x: u32) -> (u8, u8, u8, u8) {
    let b0 = (x & 0xFF) as u8;
    x >>= 8;
    let b1 = (x & 0xFF) as u8;
    x >>= 8;
    let b2 = (x & 0xFF) as u8;
    x >>= 8;
    let b3 = (x & 0xFF) as u8;

    debug_assert!(x >> 8 == 0);

    (b3, b2, b1, b0)
}

fn parse_header(bus: u8, slot: u8, func: u8) -> PciHeaderCommon {
    let (device_id, vendor_id) = half_u32(pci_config_read_u32(bus, slot, func, 0));
    let (status, command) = half_u32(pci_config_read_u32(bus, slot, func, 1));
    let (class_code, subclass, prog_if, revision_id) =
        quarter_u32(pci_config_read_u32(bus, slot, func, 2));
    let (built_in_self_test, header_type, latency_timer, cache_line_size) =
        quarter_u32(pci_config_read_u32(bus, slot, func, 3));

    PciHeaderCommon {
        device_id,
        vendor_id,
        status,
        command,
        class_code,
        subclass,
        prog_if,
        revision_id,
        built_in_self_test,
        header_type,
        latency_timer,
        cache_line_size,
    }
}
