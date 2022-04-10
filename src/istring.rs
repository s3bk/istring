use core::{fmt, slice, str, convert, mem, cmp, ptr};
use core::ptr::copy_nonoverlapping;
use core::clone::Clone;
use core::iter::{FromIterator, IntoIterator, Extend};
use core::ops::{self, Index, Add, AddAssign};
use core::hash;
use core::ptr::NonNull;
use core::borrow::Borrow;
use alloc::{string::String, vec::Vec};
use alloc::borrow::Cow;
use alloc::string::FromUtf8Error;

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

pub union IStringUnion {
    inline: Inline,
    heap:   Heap
}
pub struct IString {
    union: IStringUnion,
}

#[test]
fn test_layout() {
    let s = IStringUnion { inline: Inline { data: [0; INLINE_CAPACITY], len: IS_INLINE } };
    let heap = unsafe { s.heap };
    assert_eq!(heap.len, MAX_CAPACITY + 1);
}

#[inline]
fn string_into_raw_parts(mut s: String) -> (*mut u8, usize, usize) {
    let len = s.len();
    let cap = s.capacity();
    let ptr = s.as_mut_ptr();
    mem::forget(s);
    (ptr, len, cap)
}

unsafe impl Send for IString {}
unsafe impl Sync for IString {}
    
impl IString {
    #[inline]
    pub fn new() -> IString {
        unsafe {
            IString {
                union: IStringUnion {
                    inline: Inline { data: [0; INLINE_CAPACITY], len: IS_INLINE }
                },
            }
        }
    }
    #[inline]
    pub fn with_capacity(capacity: usize) -> IString {
        assert!(capacity < MAX_CAPACITY);
        
        if capacity > INLINE_CAPACITY {
            IString{
                union: unsafe {
                    let (ptr, len, cap) = string_into_raw_parts(String::with_capacity(capacity));
                    
                    IStringUnion {
                        heap: Heap {
                            ptr,
                            len,
                            cap
                        }
                    }
                },
            }
        } else {
            IString {
                union: IStringUnion {
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
                let (ptr, _, cap) = string_into_raw_parts(String::with_capacity(cap));
                copy_nonoverlapping(self.union.inline.data.as_ptr(), ptr, len);
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
                copy_nonoverlapping(heap.ptr, self.union.inline.data.as_mut_ptr(), len);
                String::from_raw_parts(heap.ptr, len, heap.cap);
            }
        } else {
            self.resize(len);
        }
    }
    
    fn resize(&mut self, new_cap: usize) {
        assert_eq!(self.is_inline(), false);
        assert!(new_cap >= self.len());
        
        unsafe {
            let len = self.len();
            let mut string = String::from_raw_parts(self.union.heap.ptr, len, self.union.heap.cap);
            self.union.heap.ptr = ptr::null_mut();

            string.reserve(new_cap - len);
            let (ptr, _, cap) = string_into_raw_parts(string);
            self.union.heap.ptr = ptr;
            self.union.heap.cap = cap;
        }
    }

    #[inline]
    pub fn push_str(&mut self, s: &str) {
        let old_len = self.len();
        let new_len = old_len + s.len();
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
            self.set_len(new_len);
            self.as_bytes_mut()[old_len..new_len].copy_from_slice(s.as_bytes());
        }
    }
    
    #[inline(always)]
    pub unsafe fn from_raw_parts(buf: *mut u8, length: usize, capacity: usize) -> IString {
        String::from_raw_parts(buf, length, capacity).into()
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
    pub fn push(&mut self, ch: char) {
        let mut buf = [0; 4];
        self.push_str(ch.encode_utf8(&mut buf));
    }
    
    #[inline]
    pub fn truncate(&mut self, new_len: usize) {
        if new_len < self.len() {
            unsafe { self.set_len(new_len) }
        }
    }
}
impl Drop for IString {
    #[inline]
    fn drop(&mut self) {
        if !self.is_inline() {
            unsafe {
                let len = self.len();
                String::from_raw_parts(self.union.heap.ptr, len, self.union.heap.cap);
            }
        }
    }
}
impl<'a> convert::From<&'a str> for IString {
    #[inline]
    fn from(s: &'a str) -> IString {
        let mut istring = IString::with_capacity(s.len());
        istring.push_str(s);
        istring
    }
}
impl convert::From<String> for IString {
    #[inline]
    fn from(s: String) -> IString {
        if s.capacity() != 0 {
            let (ptr, len, cap) = string_into_raw_parts(s);
            let heap = Heap {
                ptr,
                len,
                cap,
            };

            IString {
                union: IStringUnion { heap: heap },
            }
        } else {
            IString::new()
        }
    }
}
impl<'a> convert::From<Cow<'a, str>> for IString {
    #[inline]
    fn from(s: Cow<'a, str>) -> IString {
        match s {
            Cow::Borrowed(s) => IString::from(s),
            Cow::Owned(s) => IString::from(s)
        }
    }
}
impl convert::Into<String> for IString {
    #[inline]
    fn into(mut self) -> String {
        if self.is_inline() {
            let len = self.len();
            self.move_to_heap(len);
        }
        
        unsafe {
            let s = String::from_raw_parts(self.union.heap.ptr, self.union.heap.len, self.union.heap.cap);

            // the IString must not drop
            mem::forget(self);
            s
        }
    }
}

impl Clone for IString {
    #[inline]
    fn clone(&self) -> IString {
        if self.is_inline() {
            // simple case
            IString {
                union: IStringUnion { inline: unsafe { self.union.inline } },
            }
        } else {
            let mut s = IString::with_capacity(self.len());
            s.push_str(self);
            s
        }
    }
}

impl fmt::Write for IString {
    #[inline(always)]
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.push_str(s);
        Ok(())
    }
}

impl Extend<char> for IString {
    #[inline]
    fn extend<I: IntoIterator<Item = char>>(&mut self, iter: I) {
        let iterator = iter.into_iter();
        let (lower_bound, _) = iterator.size_hint();
        self.reserve(lower_bound);
        for ch in iterator {
            self.push(ch)
        }
    }
}
impl<'a> Extend<&'a char> for IString {
    #[inline(always)]
    fn extend<I: IntoIterator<Item = &'a char>>(&mut self, iter: I) {
        self.extend(iter.into_iter().cloned());
    }
}
impl<'a> Extend<&'a str> for IString {
    #[inline(always)]
    fn extend<I: IntoIterator<Item = &'a str>>(&mut self, iter: I) {
        for s in iter {
            self.push_str(s)
        }
    }
}
impl<'a> Extend<Cow<'a, str>> for IString {
    #[inline(always)]
    fn extend<I: IntoIterator<Item = Cow<'a, str>>>(&mut self, iter: I) {
        for s in iter {
            self.push_str(&s)
        }
    }
}

impl Default for IString {
    #[inline(always)]
    fn default() -> IString {
        IString::new()
    }
}

impl<'a> Add<&'a str> for IString {
    type Output = IString;

    #[inline(always)]
    fn add(mut self, other: &str) -> IString {
        self.push_str(other);
        self
    }
}
impl<'a> AddAssign<&'a str> for IString {
    #[inline]
    fn add_assign(&mut self, other: &str) {
        self.push_str(other);
    }
}

impl FromIterator<char> for IString {
    fn from_iter<T>(iter: T) -> Self where T: IntoIterator<Item=char> {
        let mut s = IString::new();
        s.extend(iter);
        s
    }
}
impl<'a> FromIterator<&'a str> for IString {
    fn from_iter<T>(iter: T) -> Self where T: IntoIterator<Item=&'a str> {
        let mut s = IString::new();
        s.extend(iter);
        s
    }
}

define_common!(IString, IStringUnion);
