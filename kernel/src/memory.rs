use bootloader_api::info::{MemoryRegionKind, MemoryRegions};
use spin::Mutex;
use x86_64::registers::control::Cr3;
use x86_64::structures::paging::{
    FrameAllocator, Mapper, OffsetPageTable, Page, PageSize, PageTable, PageTableFlags, PhysFrame,
    Size4KiB, Translate,
};
use x86_64::{PhysAddr, VirtAddr};

const MIN_ALLOCATABLE_PHYS_ADDR: u64 = 0x10_0000;
pub const DEMO_PAGE_START: u64 = 0x0000_5555_5555_0000;
const DEMO_PAGE_VALUE: u64 = 0xC0DE_CAFE_BAAD_F00D;

#[derive(Debug, Clone, Copy)]
pub struct MemorySummary {
    pub total_regions: usize,
    pub usable_regions: usize,
    pub usable_bytes: u64,
    pub reserved_bytes: u64,
    pub largest_usable_region_bytes: u64,
    pub usable_frames_4k: usize,
}

static KERNEL_MEMORY_SUMMARY: Mutex<Option<MemorySummary>> = Mutex::new(None);

pub fn set_kernel_memory_summary(summary: MemorySummary) {
    *KERNEL_MEMORY_SUMMARY.lock() = Some(summary);
}

pub fn kernel_memory_summary() -> Option<MemorySummary> {
    *KERNEL_MEMORY_SUMMARY.lock()
}

pub fn summarize_memory(regions: &MemoryRegions) -> MemorySummary {
    let mut usable_regions = 0_usize;
    let mut usable_bytes = 0_u64;
    let mut reserved_bytes = 0_u64;
    let mut largest_usable_region_bytes = 0_u64;

    for region in regions.iter() {
        let size = region.end.saturating_sub(region.start);
        if region.kind == MemoryRegionKind::Usable {
            usable_regions += 1;
            usable_bytes = usable_bytes.saturating_add(size);
            largest_usable_region_bytes = largest_usable_region_bytes.max(size);
        } else {
            reserved_bytes = reserved_bytes.saturating_add(size);
        }
    }

    MemorySummary {
        total_regions: regions.len(),
        usable_regions,
        usable_bytes,
        reserved_bytes,
        largest_usable_region_bytes,
        usable_frames_4k: (usable_bytes / Size4KiB::SIZE as u64) as usize,
    }
}

pub fn memory_region_kind_name(kind: MemoryRegionKind) -> &'static str {
    match kind {
        MemoryRegionKind::Usable => "usable",
        MemoryRegionKind::Bootloader => "bootloader",
        MemoryRegionKind::UnknownUefi(_) => "unknown_uefi",
        MemoryRegionKind::UnknownBios(_) => "unknown_bios",
        _ => "other",
    }
}

#[derive(Debug, Clone, Copy)]
pub enum VmPageSize {
    Size4KiB,
    Size2MiB,
    Size1GiB,
}

#[derive(Debug, Clone, Copy)]
pub struct VmWalkInfo {
    pub virtual_address: u64,
    pub canonical: bool,
    pub p4_index: u16,
    pub p3_index: u16,
    pub p2_index: u16,
    pub p1_index: u16,
    pub page_offset: u16,
    pub p4_flags_bits: u64,
    pub p3_flags_bits: u64,
    pub p2_flags_bits: u64,
    pub p1_flags_bits: u64,
    pub physical_address: Option<u64>,
    pub page_size: Option<VmPageSize>,
}

impl VmWalkInfo {
    fn new(virtual_address: u64, canonical: bool, virtual_addr: VirtAddr) -> Self {
        Self {
            virtual_address,
            canonical,
            p4_index: virtual_addr.p4_index().into(),
            p3_index: virtual_addr.p3_index().into(),
            p2_index: virtual_addr.p2_index().into(),
            p1_index: virtual_addr.p1_index().into(),
            page_offset: virtual_addr.page_offset().into(),
            p4_flags_bits: 0,
            p3_flags_bits: 0,
            p2_flags_bits: 0,
            p1_flags_bits: 0,
            physical_address: None,
            page_size: None,
        }
    }
}

pub fn vm_page_size_name(page_size: VmPageSize) -> &'static str {
    match page_size {
        VmPageSize::Size4KiB => "4KiB",
        VmPageSize::Size2MiB => "2MiB",
        VmPageSize::Size1GiB => "1GiB",
    }
}

pub fn walk_virtual_address(physical_memory_offset: u64, virtual_address: u64) -> VmWalkInfo {
    let virtual_addr = VirtAddr::new_truncate(virtual_address);
    let canonical = VirtAddr::try_new(virtual_address).is_ok();
    let mut info = VmWalkInfo::new(virtual_address, canonical, virtual_addr);
    if !canonical {
        return info;
    }

    let physical_offset = VirtAddr::new(physical_memory_offset);
    let p4 = unsafe { active_level_4_table(physical_offset) };
    let p4_entry = &p4[virtual_addr.p4_index()];
    info.p4_flags_bits = p4_entry.flags().bits();
    if !p4_entry.flags().contains(PageTableFlags::PRESENT) {
        return info;
    }

    let p3 = unsafe { page_table_from_physical(p4_entry.addr(), physical_offset) };
    let p3_entry = &p3[virtual_addr.p3_index()];
    info.p3_flags_bits = p3_entry.flags().bits();
    if !p3_entry.flags().contains(PageTableFlags::PRESENT) {
        return info;
    }
    if p3_entry.flags().contains(PageTableFlags::HUGE_PAGE) {
        let page_offset_1g = virtual_addr.as_u64() & ((1_u64 << 30) - 1);
        info.physical_address = Some(p3_entry.addr().as_u64() + page_offset_1g);
        info.page_size = Some(VmPageSize::Size1GiB);
        return info;
    }

    let p2 = unsafe { page_table_from_physical(p3_entry.addr(), physical_offset) };
    let p2_entry = &p2[virtual_addr.p2_index()];
    info.p2_flags_bits = p2_entry.flags().bits();
    if !p2_entry.flags().contains(PageTableFlags::PRESENT) {
        return info;
    }
    if p2_entry.flags().contains(PageTableFlags::HUGE_PAGE) {
        let page_offset_2m = virtual_addr.as_u64() & ((1_u64 << 21) - 1);
        info.physical_address = Some(p2_entry.addr().as_u64() + page_offset_2m);
        info.page_size = Some(VmPageSize::Size2MiB);
        return info;
    }

    let p1 = unsafe { page_table_from_physical(p2_entry.addr(), physical_offset) };
    let p1_entry = &p1[virtual_addr.p1_index()];
    info.p1_flags_bits = p1_entry.flags().bits();
    if !p1_entry.flags().contains(PageTableFlags::PRESENT) {
        return info;
    }

    let page_offset_4k = virtual_addr.as_u64() & 0xfff;
    info.physical_address = Some(p1_entry.addr().as_u64() + page_offset_4k);
    info.page_size = Some(VmPageSize::Size4KiB);
    info
}

pub struct BootInfoFrameAllocator {
    memory_regions: &'static MemoryRegions,
    next: usize,
}

impl BootInfoFrameAllocator {
    pub unsafe fn init(memory_regions: &'static MemoryRegions) -> Self {
        Self {
            memory_regions,
            next: 0,
        }
    }

    pub fn allocated_frames(&self) -> usize {
        self.next
    }

    pub fn remaining_usable_frames_estimate(&self) -> usize {
        self.usable_frames().skip(self.next).count()
    }

    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> + '_ {
        let usable_regions = self
            .memory_regions
            .iter()
            .filter(|region| region.kind == MemoryRegionKind::Usable);
        let address_ranges = usable_regions.filter_map(|region| {
            let start = region.start.max(MIN_ALLOCATABLE_PHYS_ADDR);
            if start >= region.end {
                None
            } else {
                Some(start..region.end)
            }
        });
        let frame_addresses =
            address_ranges.flat_map(|range| range.step_by(Size4KiB::SIZE as usize));

        frame_addresses.map(|address| PhysFrame::containing_address(PhysAddr::new(address)))
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let frame = self.usable_frames().nth(self.next);
        self.next += 1;
        frame
    }
}

pub unsafe fn init_offset_page_table(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    let level_4_table = unsafe { active_level_4_table(physical_memory_offset) };
    unsafe { OffsetPageTable::new(level_4_table, physical_memory_offset) }
}

pub fn map_demo_page(
    mapper: &mut OffsetPageTable<'static>,
    frame_allocator: &mut BootInfoFrameAllocator,
) -> Result<u64, &'static str> {
    let page: Page<Size4KiB> = Page::containing_address(VirtAddr::new(DEMO_PAGE_START));

    if mapper.translate_addr(page.start_address()).is_some() {
        return Err("demo page already mapped");
    }

    let frame = frame_allocator
        .allocate_frame()
        .ok_or("no free frame for demo mapping")?;
    let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
    let mapping = unsafe { mapper.map_to(page, frame, flags, frame_allocator) }
        .map_err(|_| "map_to failed")?;
    mapping.flush();

    let ptr: *mut u64 = page.start_address().as_mut_ptr();
    unsafe {
        ptr.write_volatile(DEMO_PAGE_VALUE);
    }
    let value = unsafe { ptr.read_volatile() };
    Ok(value)
}

pub fn map_page_with_flags(
    mapper: &mut OffsetPageTable<'static>,
    frame_allocator: &mut BootInfoFrameAllocator,
    virtual_address: u64,
    flags: PageTableFlags,
) -> Result<(), &'static str> {
    let page: Page<Size4KiB> = Page::containing_address(VirtAddr::new(virtual_address));

    if mapper.translate_addr(page.start_address()).is_some() {
        return Err("page already mapped");
    }

    let frame = frame_allocator
        .allocate_frame()
        .ok_or("no free frame for page mapping")?;
    let mapping = unsafe { mapper.map_to(page, frame, flags, frame_allocator) }
        .map_err(|_| "map_to failed")?;
    mapping.flush();
    Ok(())
}

pub fn translate_virtual_address(physical_memory_offset: u64, virtual_address: u64) -> Option<u64> {
    let mapper = unsafe { init_offset_page_table(VirtAddr::new(physical_memory_offset)) };
    mapper
        .translate_addr(VirtAddr::new(virtual_address))
        .map(|phys| phys.as_u64())
}

unsafe fn page_table_from_physical(
    physical_address: PhysAddr,
    physical_memory_offset: VirtAddr,
) -> &'static PageTable {
    let virtual_address = physical_memory_offset + physical_address.as_u64();
    let page_table_ptr: *const PageTable = virtual_address.as_ptr();
    unsafe { &*page_table_ptr }
}

unsafe fn active_level_4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable {
    let (level_4_frame, _) = Cr3::read();
    let phys = level_4_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();
    unsafe { &mut *page_table_ptr }
}
