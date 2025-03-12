use x86_64::instructions::port::Port;

// slot and device seem to be used interchangably here
pub fn pci_config_read_word(bus: u8, slot: u8, func: u8, offset: u8) -> u16 {
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

pub fn pci_config_read_u32(bus: u8, slot: u8, func: u8, register: u8) -> u32 {
    debug_assert!(slot <= 0b00011111);
    debug_assert!(func <= 0b00000111);
    debug_assert!(register <= (256 / 4) as u8);

    let lbus = bus as u32;
    let lslot = slot as u32;
    let lfunc = func as u32;
    let loffset = (register as u32) * 4;

    let address = (1 << 31) | (lbus << 16) | (lslot << 11) | (lfunc << 8) | loffset;

    let mut outl = Port::new(0xCF8);
    unsafe { outl.write(address) };

    let mut inl = Port::new(0xCFC);
    unsafe { inl.read() }
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

    let mut outl = Port::new(0xCF8);
    unsafe { outl.write(address) };

    let mut inl = Port::new(0xCFC);
    unsafe { inl.write(value) };
}
