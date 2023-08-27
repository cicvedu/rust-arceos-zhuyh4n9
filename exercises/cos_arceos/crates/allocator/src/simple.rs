//! Simple memory allocation.
//!
//! TODO: more efficient

use core::alloc::Layout;
use core::cmp::max;
use core::mem::size_of;
use core::num::NonZeroUsize;

use crate::{AllocResult, BaseAllocator, ByteAllocator};

const UNIT_SIZE : usize = 32;
const MIN_BITMAP_COUNT : usize  = 4096 * 6;

pub struct Bitmap {
    bitmap_addr: usize,
    bitmap_size: usize,
    bit_count : usize,
}

fn align_up(val : usize, align : usize) -> usize
{
    (val + align - 1) & (!(align - 1))
}

fn div_round_up(val : usize, align : usize) -> usize
{
    align_up(val, align) / align
}

impl Bitmap {
    pub fn new(_addr : usize, _size : usize, _count : usize) -> AllocResult<Self> {
        log::warn!("_count {}, size {}\n", _count, _size);
        if _count > _size * 8 {
            Err(crate::AllocError::InvalidParam)
        } else {
            Ok(Self {
                bitmap_addr : _addr,
                bitmap_size : _size,
                bit_count : _count,
            })
        }
    }
    unsafe fn check_free(self : &Self, bit : usize) -> bool
    {
        if bit > self.bit_count {
            return false;
        }
        let map : *mut u8 = (self.bitmap_addr + (bit / size_of::<u8>())) as *mut u8;
        let bytes_offset : usize = bit & size_of::<u8>();

        return (((*map) >> bytes_offset) & 0x1) == 0;
    }
    unsafe fn set_bit(self : &Self, bit : usize, set: bool) {
        if bit > self.bit_count {
            return;
        }
        let map : *mut u8 = (self.bitmap_addr + (bit / size_of::<u8>())) as *mut u8;
        let bytes_offset : usize = bit & size_of::<u8>();

        if set {
            (*map) |= 1 << bytes_offset;
        } else {
            (*map) &= !(1 << bytes_offset);
        }
    }
    unsafe fn set_bits(self : &Self, _start : usize, _end : usize, set: bool) {
        let mut cur_bit = _start;
        while cur_bit < _end {
            log::warn!("set bit {} to {}\n", cur_bit, set);
            self.set_bit(cur_bit, set);
            cur_bit += 1;
        }
    }
    pub unsafe fn alloc_contiguous(self : Self, alloc_cnt: usize) -> AllocResult<usize> {
        let mut bit_nr: usize = 0;
        let mut cnt : usize = 0;
        let mut bit_start: usize = 0;
        loop {
            if self.check_free(bit_nr) {
                if cnt == 0 {
                    bit_start = bit_nr;
                }
                cnt += 1;
            } else {
                bit_start = 0;
                cnt = 0;
            }
            if cnt == alloc_cnt {
                self.set_bits(bit_start, bit_start + cnt, true);
                log::warn!("alloc {} bit success at {}\n", cnt, bit_start);
                return Ok(bit_start);
            }
            if bit_nr >= self.bit_count {
                log::warn!("alloc {} bit failed!\n", cnt);
                return Err(crate::AllocError::NoMemory);
            }
            bit_nr += 1;
        }
    }
    pub unsafe fn dealloc(self : Self, _start: usize, _size: usize) -> AllocResult {
        let mut cur_bit = _start;
        let mut last_bit = _start + _size;
        while cur_bit < last_bit {
            if self.check_free(cur_bit) {
                return Err(crate::AllocError::MemoryOverlap);
            }
            cur_bit += 1;
        }
        self.set_bits(_start, last_bit, false);
        return Ok(())
    }
    pub fn grow_capacity(self : & mut Self, new_bits : usize) -> usize {
        let total_bits = self.bitmap_size * 8;
        let old = self.bit_count;
        if total_bits < new_bits || old >= new_bits{
            return 0;
        }
        self.bit_count = new_bits;
        new_bits - old
    }
}

impl Clone for Bitmap {
    fn clone(&self) -> Self {
        Self {
            bitmap_addr: self.bitmap_addr,
            bitmap_size: self.bitmap_size,
            bit_count : self.bit_count,
        }
    }
}
pub struct SimpleByteAllocator {
    slot_count : usize,
    free_unit : usize,
    real_size : usize,
    bitmap_addr : usize,
    data_addr : usize,
    bitmap : Option<Bitmap>,
}

impl SimpleByteAllocator {
    pub const fn new() -> Self {
        Self {
            slot_count : 0,
            free_unit : 0,
            real_size : 0,
            bitmap_addr : 0,
            data_addr : 0,
            bitmap : None,
        }
    }
}

impl BaseAllocator for SimpleByteAllocator {
    fn init(&mut self, _start: usize, _size: usize) {
        self.real_size = _size;
        if _size <= MIN_BITMAP_COUNT {
            return;
        }
        let bitmap_bytes = MIN_BITMAP_COUNT;
        let slot_count = (_size - bitmap_bytes) / UNIT_SIZE;
        if let Ok(bitmap) = Bitmap::new(_start, bitmap_bytes, slot_count) {
            self.slot_count = slot_count;
            self.free_unit = slot_count;
            self.real_size = _size;
            self.bitmap_addr = _start;
            self.data_addr = _start + bitmap_bytes;
            self.bitmap = Some(bitmap);
        } else {
            log::warn!("init fail, total size: {}, slots: {}\n", _size, slot_count);
        }
    }

    fn add_memory(&mut self, _start: usize, _size: usize) -> AllocResult {
        // grow memory
        if _start == (self.bitmap_addr + self.real_size) {
            let new_slots = (self.real_size + _size) / UNIT_SIZE;
            if let Some(mut bitmap) = self.bitmap.clone() {
                if bitmap.grow_capacity(new_slots) == 0 {
                    log::warn!("failed to grow bitmap!\n");
                    return Err(crate::AllocError::InvalidParam);
                }
                self.real_size += _size;
                self.free_unit += new_slots - self.slot_count;
                self.slot_count = new_slots;
                return Ok(());
            }
        }
        return Err(crate::AllocError::InvalidParam);
    }
}

impl ByteAllocator for SimpleByteAllocator {
    fn alloc(&mut self, _layout: Layout) -> AllocResult<NonZeroUsize> {
        let request_slots = align_up(_layout.align(), 32) / UNIT_SIZE;
        if let Some(mut bitmap) = self.bitmap.clone() {
            unsafe {
                let start_slot = bitmap.alloc_contiguous(request_slots)?;
                self.free_unit -= request_slots;
                if let Some(addr) = NonZeroUsize::new(start_slot * 32 + self.data_addr) {
                    log::warn!("alloc addr {}, start_slot {}\n", addr, start_slot);
                    return Ok(addr);
                }
            }
        }
        return Err(crate::AllocError::NoMemory);
    }

    fn dealloc(&mut self, _pos: NonZeroUsize, _layout: Layout) {
        log::warn!("dealloc at {}\n", _pos);
        let request_slots = align_up(_layout.align(), 32) / UNIT_SIZE;
        let start_slots = (usize::from(_pos) - self.data_addr) / UNIT_SIZE;
        if let Some(bitmap) = self.bitmap.clone() {
            unsafe {
                let _ = bitmap.dealloc(start_slots, request_slots);
            }
        }
    }

    fn total_bytes(&self) -> usize {
        log::warn!("total: {}", self.real_size);
        self.real_size
    }

    fn used_bytes(&self) -> usize {
        log::warn!("used slot: {}", (self.slot_count - self.free_unit));
        (self.slot_count - self.free_unit) * UNIT_SIZE 
    }

    fn available_bytes(&self) -> usize {
        log::warn!("slots: {}", self.slot_count);
        self.slot_count * UNIT_SIZE
    }
}
