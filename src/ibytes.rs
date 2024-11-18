use alloc::vec::Vec;
use core::{ptr, mem, slice, convert, ops, cmp, fmt};
use core::ops::{Index};
use core::borrow::Borrow;

#[cfg(feature="ts")]
use alloc::{borrow::ToOwned, string::String, format};

const IS_INLINE: u8 = 1 << 7;
const LEN_MASK: u8 = !IS_INLINE;

#[cfg(target_pointer_width="64")]
const INLINE_CAPACITY: usize = 23;
#[cfg(target_pointer_width="32")]
const INLINE_CAPACITY: usize = 11;

#[cfg(target_pointer_width="64")]
const MAX_CAPACITY: usize = (1 << 63) - 1;
#[cfg(target_pointer_width="32")]
const MAX_CAPACITY: usize = (1 << 31) - 1;

// use the MSG of heap.len to encode the variant
// which is also MSB of inline.len
#[cfg(target_endian = "little")]
#[derive(Copy, Clone)]
#[repr(C)]
pub struct Inline {
    pub data:   [u8; INLINE_CAPACITY],
    pub len:    u8
}
#[cfg(target_endian = "little")]
#[derive(Copy, Clone)]
#[repr(C)]
pub struct Heap {
    pub ptr:    *mut u8,
    pub cap:    usize,
    pub len:    usize
}

#[cfg(target_endian = "big")]
#[derive(Copy, Clone)]
#[repr(C)]
pub struct Inline {
    pub len:    u8,
    pub data:   [u8; INLINE_CAPACITY],
}

#[cfg(target_endian = "big")]
#[derive(Copy, Clone)]
#[repr(C)]
pub struct Heap {
    pub len:    usize,
    pub ptr:    *mut u8,
    pub cap:    usize
}

pub enum InlineOrHeap {
    Inline(Inline),
    Heap(Heap)
}

pub union IBytesUnion {
    inline: Inline,
    heap:   Heap
}

#[cfg_attr(feature="ts", derive(ts_rs::TS), ts(type="Vec<u8>"))]
pub struct IBytes {
    union: IBytesUnion,
}

unsafe impl Send for IBytes {}
unsafe impl Sync for IBytes {}

#[test]
fn test_layout() {
    let s = IBytesUnion { inline: Inline { data: [0; INLINE_CAPACITY], len: IS_INLINE } };
    let heap = unsafe { s.heap };
    assert_eq!(heap.len, MAX_CAPACITY + 1);
}

#[inline]
fn vec_into_raw_parts(mut s: Vec<u8>) -> (*mut u8, usize, usize) {
    let len = s.len();
    let cap = s.capacity();
    let ptr = s.as_mut_ptr();
    mem::forget(s);
    (ptr, len, cap)
}

define_common_bytes!(IBytes, IBytesUnion);

impl IBytes {
    #[inline]
    pub fn new() -> IBytes {
        IBytes {
            union: IBytesUnion {
                inline: Inline { data: [0; INLINE_CAPACITY], len: IS_INLINE }
            },
        }
    }
    #[inline]
    pub fn with_capacity(capacity: usize) -> IBytes {
        assert!(capacity < MAX_CAPACITY);
        
        if capacity > INLINE_CAPACITY {
            let (ptr, len, cap) = vec_into_raw_parts(Vec::with_capacity(capacity));
            IBytes {
                union: IBytesUnion {
                    heap: Heap {
                        ptr,
                        len,
                        cap
                    }
                }
            }
        } else {
            IBytes {
                union: IBytesUnion {
                    inline: Inline { data: [0; INLINE_CAPACITY], len: IS_INLINE }
                },
            }
        }
    }
    #[inline(always)]
    pub unsafe fn set_len(&mut self, new_len: usize) {
        assert!(new_len <= self.capacity());
        if self.is_inline() {
            self.union.inline.len = new_len as u8 | IS_INLINE;
        } else {
            self.union.heap.len = new_len;
        }
    }
    #[inline(always)]
    pub fn capacity(&self) -> usize {
        if self.is_inline() {
            INLINE_CAPACITY
        } else {
            unsafe { self.union.heap.cap }
        }
    }
    /// un-inline the string and expand the capacity to `cap`.
    ///
    /// does nothing if it isn't inlined.
    /// panics, if `cap` < `self.len()`
    pub fn move_to_heap(&mut self, cap: usize) {
        if self.is_inline() {
            // keep check here. the heap-bit is known to be zero, which makes len() trivial
            assert!(cap >= self.len());
            
            unsafe {
                let len = self.len();
                let (ptr, _, cap) = vec_into_raw_parts(Vec::with_capacity(cap));
                ptr::copy_nonoverlapping(self.union.inline.data.as_ptr(), ptr, len);
                self.union.heap = Heap {
                    ptr,
                    len,
                    cap
                };
            }
        }
    }
    /// if the strings fits inline, make it inline,
    /// otherwhise shrink the capacity to the `self.len()`.
    pub fn shrink(&mut self) {
        let len = self.len();
        if len <= INLINE_CAPACITY {
            unsafe {
                let heap = self.union.heap;
                self.union.inline.len = len as u8 | IS_INLINE;
                ptr::copy_nonoverlapping(heap.ptr, self.union.inline.data.as_mut_ptr(), len);
                Vec::from_raw_parts(heap.ptr, len, heap.cap);
            }
        } else {
            self.resize(len);
        }
    }
    pub (crate) fn resize(&mut self, new_cap: usize) {
        assert_eq!(self.is_inline(), false);
        assert!(new_cap >= self.len());
        
        unsafe {
            let len = self.len();
            let mut data = Vec::from_raw_parts(self.union.heap.ptr, len, self.union.heap.cap);
            self.union.heap.ptr = ptr::null_mut();

            data.reserve(new_cap - len);
            let (ptr, _, cap) = vec_into_raw_parts(data);
            self.union.heap.ptr = ptr;
            self.union.heap.cap = cap;
        }
    }
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        let new_cap = self.capacity() + additional;
        if self.is_inline() {
            if new_cap > INLINE_CAPACITY {
                self.move_to_heap(new_cap);
            }
        } else {
            self.resize(new_cap);
        }
    }
    
    #[inline]
    pub fn reserve_exact(&mut self, additional: usize) {
        let new_cap = self.capacity() + additional;
        if self.is_inline() {
            self.move_to_heap(new_cap);
        } else {
            self.resize(new_cap);
        }
    }
    #[inline]
    pub fn push(&mut self, byte: u8) {
        self.extend_from_slice(&[byte]);
    }
    pub fn extend_from_slice(&mut self, bytes: &[u8]) {
        let old_len = self.len();
        let new_len = old_len + bytes.len();
        if self.is_inline() {
            if new_len > INLINE_CAPACITY {
                self.move_to_heap(new_len.next_power_of_two());
            }
        } else {
            if new_len > self.capacity() {
                self.resize(new_len.next_power_of_two());
            }
        }

        unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr(), self.as_mut_ptr().offset(old_len as isize), bytes.len());
            self.set_len(new_len);
        }
    }
}

impl Drop for IBytes {
    #[inline]
    fn drop(&mut self) {
        if !self.is_inline() {
            unsafe {
                let len = self.len();
                Vec::from_raw_parts(self.union.heap.ptr, len, self.union.heap.cap);
            }
        }
    }
}
impl<'a> convert::From<&'a [u8]> for IBytes {
    #[inline]
    fn from(s: &'a [u8]) -> IBytes {
        if s.len() > INLINE_CAPACITY {
            let (ptr, len, cap) = vec_into_raw_parts(Vec::from(s));
            let heap = Heap {
                ptr,
                len,
                cap,
            };
            IBytes {
                union: IBytesUnion { heap: heap },
            }
        } else {
            unsafe {
                let mut data = [0; INLINE_CAPACITY];
                data[..s.len()].copy_from_slice(s);
                IBytes::from_inline(Inline { data, len: s.len() as u8 })
            }
        }
    }
}
impl<'a> convert::From<&'a str> for IBytes {
    #[inline]
    fn from(s: &'a str) -> IBytes {
        IBytes::from(s.as_bytes())
    }
}
impl convert::From<Vec<u8>> for IBytes {
    #[inline]
    fn from(s: Vec<u8>) -> IBytes {
        if s.capacity() != 0 {
            let (ptr, len, cap) = vec_into_raw_parts(s);
            let heap = Heap {
                ptr,
                len,
                cap,
            };

            IBytes {
                union: IBytesUnion { heap: heap },
            }
        } else {
            IBytes::new()
        }
    }
}
impl convert::From<alloc::string::String> for IBytes {
    #[inline]
    fn from(s: alloc::string::String) -> IBytes {
        IBytes::from(s.into_bytes())
    }
}
impl convert::Into<Vec<u8>> for IBytes {
    #[inline]
    fn into(mut self) -> Vec<u8> {
        if self.is_inline() {
            let len = self.len();
            self.move_to_heap(len);
        }
        
        unsafe {
            let s = Vec::from_raw_parts(self.union.heap.ptr, self.union.heap.len, self.union.heap.cap);

            // the IBytes must not drop
            mem::forget(self);
            s
        }
    }
}

impl Clone for IBytes {
    #[inline]
    fn clone(&self) -> IBytes {
        unsafe {
            if self.is_inline() {
                // simple case
                IBytes {
                    union: IBytesUnion { inline: self.union.inline },
                }
            } else {
                let len = self.len();
                let mut s = IBytes::with_capacity(len);
                s.extend_from_slice(slice::from_raw_parts(self.union.heap.ptr, len));
                s
            }
        }
    }
}

#[cfg(feature="size")]
impl datasize::DataSize for IBytes {
    const IS_DYNAMIC: bool = true;
    const STATIC_HEAP_SIZE: usize = core::mem::size_of::<Self>();

    fn estimate_heap_size(&self) -> usize {
        if self.is_inline() {
            Self::STATIC_HEAP_SIZE
        } else {
            Self::STATIC_HEAP_SIZE + self.capacity()
        }
    }
}
