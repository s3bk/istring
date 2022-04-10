use core::{fmt, slice, str, convert, mem, cmp, ptr, hash};
use core::clone::Clone;
use core::ops::{self, Index};
use core::borrow::Borrow;
use alloc::{string::String, vec::Vec};
use alloc::string::FromUtf8Error;
use alloc::boxed::Box;

const IS_INLINE: u8 = 1 << 7;
const LEN_MASK: u8 = !IS_INLINE;

#[cfg(target_pointer_width="64")]
const INLINE_CAPACITY: usize = 15;
#[cfg(target_pointer_width="32")]
const INLINE_CAPACITY: usize = 7;

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
}

union SmallStringUnion {
    inline: Inline,
    heap:   Heap
}
pub struct SmallString {
    union: SmallStringUnion,
}

#[test]
fn test_layout() {
    let s = SmallStringUnion { inline: Inline { data: [0; INLINE_CAPACITY], len: IS_INLINE } };
    let heap = unsafe { s.heap };
    assert_eq!(heap.len, MAX_CAPACITY + 1);
}

#[inline(always)]
fn box_str(s: &str) -> Box<str> {
    Box::from(s)
}
#[inline(always)]
fn box_str_into_raw_parts(mut s: Box<str>) -> (*mut u8, usize) {
    let len = s.len();
    let ptr = s.as_mut_ptr();
    mem::forget(s);
    (ptr, len)
}
#[inline(always)]
unsafe fn box_str_from_raw_parts(ptr: *mut u8, len: usize) -> Box<str> {
    let ptr = slice::from_raw_parts_mut(ptr, len) as *mut [u8] as *mut str;
    Box::from_raw(ptr)
}

unsafe impl Send for SmallString {}

impl SmallString {
    #[inline(always)]
    pub fn new(s: &str) -> SmallString {
        let len = s.len();
        unsafe {
            if len > INLINE_CAPACITY {
                let s = box_str(s);
                let (ptr, len) = box_str_into_raw_parts(s);
                SmallString::from_heap(
                    Heap {
                        ptr,
                        len
                    },
                )
            } else {
                let mut data = [0; INLINE_CAPACITY];
                data[.. len].copy_from_slice(s.as_bytes());
                SmallString::from_inline(
                    Inline { data, len: len as u8 },
                )
            }
        }
    }
}
impl Drop for SmallString {
    #[inline]
    fn drop(&mut self) {
        if !self.is_inline() {
            unsafe {
                box_str_from_raw_parts(self.union.heap.ptr, self.union.heap.len);
            }
        }
    }
}
impl<'a> convert::From<&'a str> for SmallString {
    #[inline]
    fn from(s: &'a str) -> SmallString {
        SmallString::new(s)
    }
}
impl convert::From<String> for SmallString {
    #[inline]
    fn from(mut s: String) -> SmallString {
        let len = s.len();
        if len <= INLINE_CAPACITY {
            return SmallString::from(s.as_str());
        }

        unsafe {
            let s = s.into_boxed_str();
            let (ptr, len) = box_str_into_raw_parts(s);
            let heap = Heap {
                ptr,
                len,
            };

            SmallString::from_heap(
                heap,
            )
        }
    }
}
impl Into<String> for SmallString {
    fn into(self) -> String {
        let len = self.len();
        if self.is_inline() {
            self.as_str().into()
        } else {
            unsafe {
                let s = box_str_from_raw_parts(self.union.heap.ptr, len);
                // the SmallString must not drop
                mem::forget(self);

                String::from(s)
            }
        }
    }
}
impl Clone for SmallString {
    #[inline]
    fn clone(&self) -> SmallString {
        unsafe {
            if self.is_inline() {
                // simple case
                SmallString {
                    union: SmallStringUnion { inline: self.union.inline },
                }
            } else {
                let len = self.len();
                let bytes = slice::from_raw_parts(self.union.heap.ptr, len);
                let s = core::str::from_utf8_unchecked(bytes);
                let (ptr, len) = box_str_into_raw_parts(box_str(s));
                SmallString::from_heap(
                    Heap {
                        ptr,
                        len
                    },
                )
            }
        }
    }
}

define_common!(SmallString, SmallStringUnion);
