use alloc::{boxed::Box, vec::Vec};
use linked_list_allocator::LockedHeap;
use x86_64::VirtAddr;
use x86_64::structures::paging::mapper::MapToError;
use x86_64::structures::paging::{
    FrameAllocator, Mapper, OffsetPageTable, Page, PageTableFlags, Size4KiB,
};

use crate::memory::BootInfoFrameAllocator;

pub const HEAP_START: u64 = 0x0000_4444_4444_0000;
pub const HEAP_SIZE: usize = 100 * 1024;

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

pub fn init_heap(
    mapper: &mut OffsetPageTable<'static>,
    frame_allocator: &mut BootInfoFrameAllocator,
) -> Result<(), MapToError<Size4KiB>> {
    let heap_start = VirtAddr::new(HEAP_START);
    let heap_end = heap_start + (HEAP_SIZE - 1) as u64;
    let first_page = Page::containing_address(heap_start);
    let last_page = Page::containing_address(heap_end);
    let page_range = Page::range_inclusive(first_page, last_page);

    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        let mapping = unsafe { mapper.map_to(page, frame, flags, frame_allocator)? };
        mapping.flush();
    }

    unsafe {
        ALLOCATOR.lock().init(HEAP_START as *mut u8, HEAP_SIZE);
    }

    Ok(())
}

pub fn probe_allocations() -> (u64, u64) {
    let boxed = Box::new(0xA1B2_C3D4_E5F6_1234_u64);
    let mut values = Vec::new();
    for value in 1_u64..=16 {
        values.push(value);
    }
    let sum = values.iter().copied().sum::<u64>();
    (*boxed, sum)
}
