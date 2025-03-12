use super::io::pci_config_read_u32;

#[derive(Debug)]
pub struct PciHeaderCommon {
    pub device_id: u16,
    pub vendor_id: u16,

    pub status: u16,
    pub command: u16,

    pub class_code: u8,
    pub subclass: u8,
    pub prog_if: u8,
    pub revision_id: u8,

    pub built_in_self_test: u8,
    pub header_type: u8,
    pub latency_timer: u8,
    pub cache_line_size: u8,
}

#[derive(Debug)]
pub struct PciHeaderType0 {
    pub headhead: PciHeaderCommon,

    pub base_addresses: [u32; 6],

    pub cardbus_cis_pointer: u32,

    pub subsystem_id: u16,
    pub subsystem_vendor_id: u16,

    pub expansion_rom_base_address: u32,

    /* some reserved space */
    pub capabilites_pointer: u8,
    /* some reserved space */
    pub max_latency: u8,
    pub min_grant: u8,
    pub interrupt_pin: u8,
    pub interrupt_line: u8,
}

// Here we return the the tuple
// as if we split the binary representation of
// the number in half:
// high place values on left, low place values on right
pub fn half_u32(x: u32) -> (u16, u16) {
    let low = x & 0xFFFF;
    let high = (x >> 16) & 0xFFFF;
    (high as u16, low as u16)
}

pub fn half_u16(x: u16) -> (u8, u8) {
    let low = x & 0xFF;
    let high = (x >> 8) & 0xFF;
    (high as u8, low as u8)
}

pub fn quarter_u32(mut x: u32) -> (u8, u8, u8, u8) {
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

pub fn parse_header_common(bus: u8, slot: u8, func: u8) -> PciHeaderCommon {
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

pub fn parse_header_type0(
    bus: u8,
    slot: u8,
    func: u8,
    headhead: PciHeaderCommon,
) -> PciHeaderType0 {
    let base_addresses = [
        pci_config_read_u32(bus, slot, func, 0x4),
        pci_config_read_u32(bus, slot, func, 0x5),
        pci_config_read_u32(bus, slot, func, 0x6),
        pci_config_read_u32(bus, slot, func, 0x7),
        pci_config_read_u32(bus, slot, func, 0x8),
        pci_config_read_u32(bus, slot, func, 0x9),
    ];
    let cardbus_cis_pointer = pci_config_read_u32(bus, slot, func, 0xA);
    let (subsystem_id, subsystem_vendor_id) = half_u32(pci_config_read_u32(bus, slot, func, 0xB));
    let expansion_rom_base_address = pci_config_read_u32(bus, slot, func, 0xC);

    let (_, _, _, capabilites_pointer) = quarter_u32(pci_config_read_u32(bus, slot, func, 0xD));

    let (max_latency, min_grant, interrupt_pin, interrupt_line) =
        quarter_u32(pci_config_read_u32(bus, slot, func, 0xF));

    PciHeaderType0 {
        headhead,
        base_addresses,
        cardbus_cis_pointer,
        subsystem_id,
        subsystem_vendor_id,
        expansion_rom_base_address,
        capabilites_pointer,
        max_latency,
        min_grant,
        interrupt_pin,
        interrupt_line,
    }
}
