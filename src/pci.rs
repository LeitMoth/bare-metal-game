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
                print!("[{:X}]", id)
            }
        }
    }
}

pub fn init_pci() {
    check_all();
}

struct PciHeaderCommon {
    device_id: u16,
    vendor_id: u16,

    status: u16,
    command: u16, /* display binary */

    class_code: u8,
    subclass: u8,
    prog_if: u8,
    revision_id: u8,

    built_in_self_test: u8,
    header_type: u8,
    latency_timer: u8,
    cache_line_size: u8,
}

fn parse_header(bus: u8, slot: u8, func: u8) {
    let vendor_id = pci_config_read_word(bus, slot, func, 0x0);
    let device_id = pci_config_read_word(bus, slot, func, 0x2);
    let device_status = pci_config_read_word(bus, slot, func, 0x4);
    let command = pci_config_read_word(bus, slot, func, 0x6);
}
