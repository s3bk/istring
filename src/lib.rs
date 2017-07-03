#![feature(untagged_unions, alloc, heap_api, str_mut_extras)]

/*!
I String type hat has the same size as `String`,
but allows the whole size, except for one byte to inline the data.

That means on 32bit machines: size_of::<IString>() == 12 bytes, inline capacity: 11 bytes
on 64bit machines: size_of::<IString>() == 24 bytes, inline capacity: 23 bytes
*/

extern crate alloc;
use std::{ops, fmt, slice, str, convert, mem};
use std::ptr::copy_nonoverlapping;
use std::string::FromUtf8Error;

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
#[repr(C)]
struct Inline {
    data:   [u8; INLINE_CAPACITY],
    len:    u8
}
#[cfg(target_endian = "little")]
#[derive(Copy, Clone)]
#[repr(C)]
struct Heap {
    ptr:    *mut u8,
    cap:    usize,
    len:    usize
}

#[cfg(target_endian = "big")]
#[repr(C)]
struct Inline {
    len:    u8,
    data:   [u8; INLINE_CAPACITY],
}

#[cfg(target_endian = "big")]
#[repr(C)]
struct Heap {
    len:    usize,
    ptr:    *mut u8,
    cap:    usize
}

pub union IString {
    inline: Inline,
    heap:   Heap
}

#[test]
fn test_layout() {
    let s = IString { inline: Inline { data: [0; INLINE_CAPACITY], len: IS_INLINE } };
    let heap = unsafe { s.heap };
    assert_eq!(heap.len, MAX_CAPACITY + 1);
}

impl IString {
    pub fn new() -> IString {
        IString {
            inline: Inline { data: [0; INLINE_CAPACITY], len: IS_INLINE }
        }
    }
    pub fn with_capacity(capacity: usize) -> IString {
        assert!(capacity < MAX_CAPACITY);
        let mut s = IString::new();
        
        if capacity > INLINE_CAPACITY {
            s.move_to_heap(capacity)
        }
        s
    }
    
    #[inline(always)]
    pub fn is_inline(&self) -> bool {
        unsafe {
            (self.inline.len & IS_INLINE) != 0
        }
    }
    
    #[inline(always)]
    pub fn len(&self) -> usize {
        unsafe {
            if self.is_inline() {
                (self.inline.len & LEN_MASK) as usize
            } else {
                self.heap.len
            }
        }
    }
    #[inline(always)]
    pub unsafe fn set_len(&mut self, new_len: usize) {
        assert!(new_len <= self.capacity());
        if self.is_inline() {
            self.inline.len = new_len as u8 | IS_INLINE;
        } else {
            self.heap.len = new_len;
        }
    }
    
    #[inline(always)]
    pub fn capacity(&self) -> usize {
        if self.is_inline() {
            INLINE_CAPACITY
        } else {
            unsafe { self.heap.cap }
        }
    }
    
    /// un-inline the string and expand the capacity to `cap`
    /// does nothing if it isn't inlined.
    /// panics, if `cap` < `self.len()`
    pub fn move_to_heap(&mut self, cap: usize) {
        if self.is_inline() {
            // keep check here. the heap-bit is known to be zero, which makes len() trivial
            assert!(cap >= self.len());
            
            unsafe {
                let len = self.len();
                let ptr = alloc::heap::allocate(cap, 1);
                copy_nonoverlapping(self.inline.data.as_ptr(), ptr, len);
                self.heap = Heap { ptr: ptr, len: len, cap: cap };
            }
        }
    }
    
    /// if the strings fits inline, make it inline,
    /// otherwhise shrink the capacity to the `self.len()`.
    pub fn shrink(&mut self) {
        let len = self.len();
        if len <= INLINE_CAPACITY {
            unsafe {
                let heap = self.heap;
                self.inline.len = len as u8 | IS_INLINE;
                copy_nonoverlapping(heap.ptr, self.inline.data.as_mut_ptr(), len);
                alloc::heap::deallocate(heap.ptr, heap.cap, 1);
            }
        } else {
            self.resize(len);
        }
    }
    
    #[inline(always)]
    pub fn as_bytes(&self) -> &[u8] {
        let len = self.len();
        unsafe {
            if self.is_inline() {
                &self.inline.data[.. len]
            } else {
                slice::from_raw_parts(self.heap.ptr, len)
            }
        }
    }
    
    #[inline(always)]
    unsafe fn as_bytes_mut(&mut self) -> &mut [u8] {
        let len = self.len();
        if self.is_inline() {
            &mut self.inline.data[.. len]
        } else {
            slice::from_raw_parts_mut(self.heap.ptr, len)
        }
    }
    
    fn resize(&mut self, new_cap: usize) {
        assert_eq!(self.is_inline(), false);
        assert!(new_cap >= self.len());
        
        unsafe {
            let ptr = alloc::heap::reallocate(self.heap.ptr, self.heap.cap, new_cap, 1);
            self.heap.ptr = ptr;
            self.heap.cap = new_cap;
        }
    }
    
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
        println!("len = {:?}", self.len());
    }
    
    #[inline(always)]
    pub fn from_utf8(vec: Vec<u8>) -> Result<IString, FromUtf8Error> {
        String::from_utf8(vec).map(IString::from)
    }
    
    #[inline(always)]
    pub unsafe fn from_raw_parts(buf: *mut u8, length: usize, capacity: usize) -> IString {
        String::from_raw_parts(buf, length, capacity).into()
    }
    
    #[inline(always)]
    pub unsafe fn from_utf8_unchecked(bytes: Vec<u8>) -> String {
        String::from_utf8_unchecked(bytes).into()
    }
    
    #[inline(always)]
    pub fn into_bytes(self) -> Vec<u8> {
        let s: String = self.into();
        s.into_bytes()
    }
    
    #[inline(always)]
    pub fn as_str(&self) -> &str {
        unsafe {
            str::from_utf8_unchecked(self.as_bytes())
        }
    }
    
    #[inline(always)]
    pub fn as_mut_str(&mut self) -> &mut str {
        unsafe {
            str::from_utf8_unchecked_mut(self.as_bytes_mut())
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
    fn drop(&mut self) {
        if !self.is_inline() {
            unsafe {
                alloc::heap::deallocate(self.heap.ptr, self.heap.cap, 1);
            }
        }
    }
}
impl ops::Deref for IString {
    type Target = str;
    
    #[inline(always)]
    fn deref(&self) -> &str {
        self.as_str()
    }
}
impl fmt::Debug for IString {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        <str as fmt::Debug>::fmt(&*self, f)
    }
}
impl fmt::Display for IString {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        <str as fmt::Display>::fmt(&*self, f)
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
        let mut s = s.into_bytes();
        let heap = Heap {
            ptr:    s.as_mut_ptr(),
            len:    s.len(),
            cap:    s.capacity()
        };
        mem::forget(s);
        
        IString { heap: heap }
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
            String::from_raw_parts(self.heap.ptr, self.heap.len, self.heap.cap)
        }
    }
}

#[test]
fn main() {
    let mut s1 = IString::from("Hello World!");
    println!("s1: {:?}", s1);
    let s2 = IString::from("Hello World! .........xyz");
    println!("s2: {:?}", s2);
    s1.push_str("_.........xyz");
    println!("s1: {:?}", s1);
}
