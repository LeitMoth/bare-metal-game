use pluggable_interrupt_os::println;
use x86_64::{
    instructions::port::Port,
    structures::port::{PortRead, PortWrite},
};

const CONFIG_ADDRESS: u16 = 0xCF8;
const CONFIG_DATA: u16 = 0xCFC;

// slot and device seem to be used interchangably here
pub fn pci_config_read_word(bus: u8, slot: u8, func: u8, offset: u8) -> u16 {
    debug_assert!(slot <= 0b00011111);
    debug_assert!(func <= 0b00000111);

    let lbus = bus as u32;
    let lslot = slot as u32;
    let lfunc = func as u32;
    let loffset = offset as u32;

    let address = (lbus << 16) | (lslot << 11) | (lfunc << 8) | (loffset & 0xFC) | (0x80000000u32);

    let mut config_address_port = Port::new(CONFIG_ADDRESS);
    unsafe { config_address_port.write(address) };

    let mut config_data_port = Port::new(CONFIG_DATA);
    let tmp: u32 = unsafe { config_data_port.read() };
    let sel_hi_shift = (loffset & 2) * 8;

    // println!("=0x{:X} >> 0x{:X}", tmp, sel_hi_shift);

    ((tmp >> sel_hi_shift) & 0xFFFF) as u16
}

pub fn pci_config_read_u32(bus: u8, slot: u8, func: u8, register: u8) -> u32 {
    debug_assert!(slot <= 0b00011111);
    debug_assert!(func <= 0b00000111);
    debug_assert!(register <= (256 / 4) as u8);

    let lbus = bus as u32;
    let lslot = slot as u32;
    let lfunc = func as u32;
    let loffset = (register as u32) * 4;

    let address = (1 << 31) | (lbus << 16) | (lslot << 11) | (lfunc << 8) | loffset;

    let mut config_address_port = Port::new(CONFIG_ADDRESS);
    unsafe { config_address_port.write(address) };

    let mut config_data_port = Port::new(CONFIG_DATA);
    unsafe { config_data_port.read() }
}

pub fn pci_config_write_u32(bus: u8, slot: u8, func: u8, register: u8, value: u32) {
    debug_assert!(slot <= 0b00011111);
    debug_assert!(func <= 0b00000111);
    debug_assert!(register <= (256 / 4) as u8);

    let lbus = bus as u32;
    let lslot = slot as u32;
    let lfunc = func as u32;
    let loffset = (register as u32) * 4;

    let address = (1 << 31) | (lbus << 16) | (lslot << 11) | (lfunc << 8) | loffset;

    let mut config_address_port = Port::new(CONFIG_ADDRESS);
    unsafe { config_address_port.write(address) };

    let mut config_data_port = Port::new(CONFIG_DATA);
    unsafe { config_data_port.write(value) };
}

pub fn pci_config_modify(bus: u8, slot: u8, func: u8, register: u8, f: impl Fn(u32) -> u32) {
    debug_assert!(slot <= 0b00011111);
    debug_assert!(func <= 0b00000111);
    debug_assert!(register <= (256 / 4) as u8);

    let lbus = bus as u32;
    let lslot = slot as u32;
    let lfunc = func as u32;
    let loffset = (register as u32) * 4;

    let address = (1 << 31) | (lbus << 16) | (lslot << 11) | (lfunc << 8) | loffset;

    let mut config_address_port = Port::new(CONFIG_ADDRESS);
    unsafe { config_address_port.write(address) };

    let mut config_data_port = Port::new(CONFIG_DATA);
    let mut tmp = unsafe { config_data_port.read() };

    tmp = f(tmp);

    unsafe { config_address_port.write(address) };
    unsafe { config_data_port.write(tmp) }
}

// The types are weird here:
// https://github.com/VendelinSlezak/BleskOS/blob/21b59a62438d8248935281348d899cbc648ffb27/source/drivers/system/buses/pci.c#L329
// I had to check how someone elses code read this in to unterstand
// OsDev wiki says this:
// "Typically, memory address BARs need to be located in physical ram while I/O space BARs
// can reside at any memory address (even beyond physical memory)."
// Which gives the impression that I/O space bars can be a lot larger than 16 bits.
// But when I caved in and look at how the referenced code stored these,
// it stores bars as word_t s!!!!!
// So I guess it is u16
pub fn io_space_bar_write<T: PortWrite>(port: u16, value: T) {
    // println!("W_{port:#X}");
    let mut config_address_port = Port::new(port);
    unsafe { config_address_port.write(value) };
}

pub fn io_space_bar_read<T: PortRead>(port: u16) -> T {
    // println!("R_{port:#X}");
    let mut config_data_port = Port::new(port);
    unsafe { config_data_port.read() }
}
