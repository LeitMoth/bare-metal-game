use bootloader::{
    bootinfo::{MemoryRegion, MemoryRegionType},
    BootInfo,
};

pub struct PhysAllocator {
    prime_region: MemoryRegion,
    physical_memory_offset: u64,
    offset: u64,
}

pub struct DualAddr {
    pub phys_addr: u64,
    pub virt_addr: u64,
}

pub struct DualPtr32<'a, T> {
    pub r_phys: u32,
    pub rw_virt: &'a mut T,
}

impl PhysAllocator {
    // Fails if there are no nonempty unused memory regions
    pub fn new(boot_info: &BootInfo) -> Option<Self> {
        let len = |m: &MemoryRegion| m.range.end_addr() - m.range.start_addr();
        let mut free_region = MemoryRegion::empty();

        for m in boot_info.memory_map.iter() {
            if let MemoryRegionType::Usable = m.region_type {
                if len(m) > len(&free_region) {
                    free_region = m.clone();
                }
            }
        }

        if len(&free_region) == 0 {
            None
        } else {
            Some(PhysAllocator {
                prime_region: free_region,
                physical_memory_offset: boot_info.physical_memory_offset,
                offset: 0,
            })
        }
    }

    // Aligns by 4
    pub fn get_hunk(&mut self, size: u64) -> DualAddr {
        let phys_mem_start = self.prime_region.range.start_addr();
        let phys_mem_end = self.prime_region.range.end_addr();

        let mut phys_start = phys_mem_start + self.offset;
        // align to 4 byte boundary
        while phys_start % 4 != 0 {
            phys_start += 1;
        }
        // end should be exclusive
        let phys_end = phys_start + size;

        if phys_end > phys_mem_end {
            panic!(
                "Failed to allocate, too large by {} bytes!",
                phys_end - phys_mem_end
            );
        } else {
            self.offset = phys_end;
            DualAddr {
                phys_addr: phys_start,
                virt_addr: phys_start + self.physical_memory_offset,
            }
        }
    }

    pub fn alloc32<'a, T>(&mut self) -> DualPtr32<'a, T> {
        let DualAddr {
            phys_addr,
            virt_addr,
        } = self.get_hunk(size_of::<T>() as u64);

        debug_assert!(phys_addr <= u32::MAX as u64);

        DualPtr32 {
            r_phys: phys_addr as u32,
            rw_virt: unsafe { &mut *(virt_addr as *mut T) },
        }
    }
}
