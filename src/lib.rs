// Copyright 2017 Sebastian Köln

// Licensed under the MIT license
// <LICENSE or http://opensource.org/licenses/MIT>

// The trait impls contains large chunks from alloc/string.rs,
// with the following copyright notice:

// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(untagged_unions, alloc, str_mut_extras, inclusive_range, allocator_api, unique)]
#![no_std]

/*!
A replacement for String that allows storing strings of length up to sizeof<String>() - 1 without a heap allocation

That means on 32bit machines: size_of::<IString>() == 12 bytes, inline capacity: 11 bytes
on 64bit machines: size_of::<IString>() == 24 bytes, inline capacity: 23 bytes
*/

extern crate alloc;

use core::{fmt, slice, str, convert, mem, cmp, ptr};
use core::ptr::copy_nonoverlapping;
use core::clone::Clone;
use core::iter::Extend;
use core::ops::{self, Index, Add, AddAssign};
use core::hash;
use core::ptr::Unique;
use alloc::{String, Vec, heap};
use alloc::borrow::Cow;
use alloc::string::FromUtf8Error;
use alloc::allocator::{Alloc, Layout};
use heap::Heap as HeapAlloc;

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
    pub ptr:    Unique<u8>,
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
    pub ptr:    Unique<u8>,
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
pub struct IString<A: Alloc=HeapAlloc> {
    union: IStringUnion,
    alloc: A
}

#[test]
fn test_layout() {
    let s = IStringUnion { inline: Inline { data: [0; INLINE_CAPACITY], len: IS_INLINE } };
    let heap = unsafe { s.heap };
    assert_eq!(heap.len, MAX_CAPACITY + 1);
}

impl IString {
    #[inline(always)]
    pub fn new() -> IString {
        IString::new_in(HeapAlloc)
    }
    #[inline(always)]
    pub fn with_capacity(capacity: usize) -> IString {
        IString::with_capacity_in(capacity, HeapAlloc)
    }
}
    
impl<A: Alloc> IString<A> {
    #[inline(always)]
    pub fn new_in(a: A) -> IString<A> {
        IString {
            union: IStringUnion {
                inline: Inline { data: [0; INLINE_CAPACITY], len: IS_INLINE },
            },
            alloc: a
        }
    }
    #[inline]
    pub fn with_capacity_in(capacity: usize, mut a: A) -> IString<A> {
        assert!(capacity < MAX_CAPACITY);
        
        if capacity > INLINE_CAPACITY {
            IString{
                union: unsafe {
                    let ptr = a.alloc(Layout::from_size_align_unchecked(capacity, 1)).unwrap();
                    IStringUnion { heap: Heap { ptr: Unique::new(ptr), len: 0, cap: capacity } }
                },
                alloc: a
            }
        } else {
            IString {
                union: IStringUnion {
                    inline: Inline { data: [0; INLINE_CAPACITY], len: IS_INLINE }
                },
                alloc: a
            }
        }
    }

    /// view as Inline.
    ///
    /// Panics if the string isn't inlined
    #[inline(always)]
    pub unsafe fn as_inline(&mut self) -> &mut Inline {
        assert!(self.is_inline());
        &mut self.union.inline
    }

    /// view as Heap.
    ///
    /// Panics if the string isn't on the Heap
    #[inline(always)]
    pub unsafe fn as_heap(&mut self) -> &mut Heap {
        assert!(!self.is_inline());
        &mut self.union.heap
    }

    //#[inline]
    //pub fn as_inline_or_heap(self) 
    
    #[inline(always)]
    pub fn is_inline(&self) -> bool {
        unsafe {
            (self.union.inline.len & IS_INLINE) != 0
        }
    }
    
    #[inline(always)]
    pub fn len(&self) -> usize {
        unsafe {
            if self.is_inline() {
                (self.union.inline.len & LEN_MASK) as usize
            } else {
                self.union.heap.len
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
                let ptr = HeapAlloc.alloc(Layout::from_size_align_unchecked(cap, 1)).unwrap();
                copy_nonoverlapping(self.union.inline.data.as_ptr(), ptr, len);
                self.union.heap = Heap { ptr: Unique::new(ptr), len: len, cap: cap };
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
                copy_nonoverlapping(heap.ptr.as_ptr(), self.union.inline.data.as_mut_ptr(), len);
                HeapAlloc.dealloc(heap.ptr.as_ptr(), Layout::from_size_align_unchecked(heap.cap, 1));
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
                &self.union.inline.data[.. len]
            } else {
                slice::from_raw_parts(self.union.heap.ptr.as_ptr(), len)
            }
        }
    }
    
    #[inline(always)]
    unsafe fn as_bytes_mut(&mut self) -> &mut [u8] {
        let len = self.len();
        if self.is_inline() {
            &mut self.union.inline.data[.. len]
        } else {
            slice::from_raw_parts_mut(self.union.heap.ptr.as_ptr(), len)
        }
    }
    
    fn resize(&mut self, new_cap: usize) {
        assert_eq!(self.is_inline(), false);
        assert!(new_cap >= self.len());
        
        unsafe {
            let ptr = HeapAlloc.realloc(
                self.union.heap.ptr.as_ptr(),
                Layout::from_size_align_unchecked(self.union.heap.cap, 1),
                Layout::from_size_align_unchecked(new_cap, 1)
            ).unwrap();
            self.union.heap.ptr = Unique::new(ptr);
            self.union.heap.cap = new_cap;
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

    /// Deconstruct into the Heap part and the allocator
    ///
    /// Assumes it is heap-state, panics otherwhise. (you may want to call move_to_heap before this.)
    /// The caller is responsible to adequatly dispose the owned memory. (for example by calling IString::from_heap)
    #[inline(always)]
    pub fn to_heap(self) -> (Heap, A) {
        assert_eq!(self.is_inline(), false);
        unsafe {
            let heap = self.union.heap;
            let alloc = ptr::read(&self.alloc);
            mem::forget(self);
            
            (heap, alloc)
        }
    }
    
    /// Deconstruct into the Inline part and the allocator
    ///
    /// Assumes the string is inlined and panics otherwhise.
    #[inline(always)]
    pub fn to_inline(self) -> (Inline, A) {
        assert_eq!(self.is_inline(), true);
        unsafe {
            let mut inline = self.union.inline;
            let alloc = ptr::read(&self.alloc);
            mem::forget(self);
            
            inline.len &= !IS_INLINE; // clear the bit
            (inline, alloc)
        }
    }
    pub unsafe fn from_heap(heap: Heap, alloc: A) -> Self {
        let union = IStringUnion { heap: heap };
        assert_eq!(union.inline.len & IS_INLINE, 0);
        IString { union: union, alloc: alloc }
    }
    pub unsafe fn from_inline(mut inline: Inline, alloc: A) -> Self {
        assert!(inline.len as usize <= INLINE_CAPACITY);
        inline.len |= IS_INLINE; // set inline bit
        IString {
            union: IStringUnion { inline: inline },
            alloc: alloc
        }
    }
}
impl<A: Alloc> Drop for IString<A> {
    #[inline]
    fn drop(&mut self) {
        if !self.is_inline() {
            unsafe {
                HeapAlloc.dealloc(self.union.heap.ptr.as_ptr(), Layout::from_size_align_unchecked(self.union.heap.cap, 1));
            }
        }
    }
}
impl IString {        
    #[inline(always)]
    pub fn into_bytes(self) -> Vec<u8> {
        let s: String = self.into();
        s.into_bytes()
    }
}
impl<A: Alloc> ops::Deref for IString<A> {
    type Target = str;
    
    #[inline(always)]
    fn deref(&self) -> &str {
        self.as_str()
    }
}
impl<A: Alloc> fmt::Debug for IString<A> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        <str as fmt::Debug>::fmt(&*self, f)
    }
}
impl<A: Alloc> fmt::Display for IString<A> {
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
            ptr:    unsafe { Unique::new(s.as_mut_ptr()) },
            len:    s.len(),
            cap:    s.capacity()
        };
        mem::forget(s);
        
        IString {
            union: IStringUnion { heap: heap },
            alloc: HeapAlloc
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
            String::from_raw_parts(self.union.heap.ptr.as_ptr(), self.union.heap.len, self.union.heap.cap)
        }
    }
}

impl<A: Alloc+Clone> Clone for IString<A> {
    #[inline]
    fn clone(&self) -> IString<A> {
        if self.is_inline() {
            // simple case
            IString {
                union: IStringUnion { inline: unsafe { self.union.inline } },
                alloc: self.alloc.clone()
            }
        } else {
            let mut s = IString::with_capacity_in(self.len(), self.alloc.clone());
            s.push_str(self);
            s
        }
    }
}


impl<A: Alloc> PartialEq<str> for IString<A> {
    #[inline(always)]
    fn eq(&self, rhs: &str) -> bool {
        self.as_str() == rhs
    }
}
impl<'a, A: Alloc> PartialEq<&'a str> for IString<A> {
    #[inline(always)]
    fn eq(&self, rhs: &&'a str) -> bool {
        self.as_str() == *rhs
    }
}
impl<A: Alloc> PartialEq<String> for IString<A> {
    #[inline(always)]
    fn eq(&self, rhs: &String) -> bool {
        self.as_str() == rhs
    }
}
impl<A: Alloc, B: Alloc> PartialEq<IString<B>> for IString<A> {
    #[inline(always)]
    fn eq(&self, rhs: &IString<B>) -> bool {
        self.as_str() == rhs.as_str()
    }
}
impl<A: Alloc> Eq for IString<A> {}
impl<A: Alloc> cmp::PartialOrd for IString<A> {
    #[inline(always)]
    fn partial_cmp(&self, rhs: &Self) -> Option<cmp::Ordering> {
        self.as_str().partial_cmp(rhs.as_str())
    }
    #[inline(always)]
    fn lt(&self, rhs: &Self) -> bool {
        self.as_str().lt(rhs.as_str())
    }
    #[inline(always)]
    fn le(&self, rhs: &Self) -> bool {
        self.as_str().le(rhs.as_str())
    }
    #[inline(always)]
    fn gt(&self, rhs: &Self) -> bool {
        self.as_str().gt(rhs.as_str())
    }
    #[inline(always)]
    fn ge(&self, rhs: &Self) -> bool {
        self.as_str().ge(rhs.as_str())
    }
}
impl<A: Alloc> cmp::Ord for IString<A> {
    #[inline(always)]
    fn cmp(&self, other: &IString<A>) -> cmp::Ordering {
        self.as_str().cmp(other.as_str())
    }
}
impl<A: Alloc> fmt::Write for IString<A> {
    #[inline(always)]
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.push_str(s);
        Ok(())
    }
}

impl<A: Alloc> Extend<char> for IString<A> {
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
impl<'a, A: Alloc> Extend<&'a char> for IString<A> {
    #[inline(always)]
    fn extend<I: IntoIterator<Item = &'a char>>(&mut self, iter: I) {
        self.extend(iter.into_iter().cloned());
    }
}
impl<'a, A: Alloc> Extend<&'a str> for IString<A> {
    #[inline(always)]
    fn extend<I: IntoIterator<Item = &'a str>>(&mut self, iter: I) {
        for s in iter {
            self.push_str(s)
        }
    }
}
impl<'a, A: Alloc> Extend<Cow<'a, str>> for IString<A> {
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

impl<A: Alloc> hash::Hash for IString<A> {
    #[inline(always)]
    fn hash<H: hash::Hasher>(&self, hasher: &mut H) {
        (**self).hash(hasher)
    }
}

impl<'a, A: Alloc> Add<&'a str> for IString<A> {
    type Output = IString<A>;

    #[inline(always)]
    fn add(mut self, other: &str) -> IString<A> {
        self.push_str(other);
        self
    }
}
impl<'a, A: Alloc> AddAssign<&'a str> for IString<A> {
    #[inline]
    fn add_assign(&mut self, other: &str) {
        self.push_str(other);
    }
}

impl<A: Alloc> ops::Index<ops::Range<usize>> for IString<A> {
    type Output = str;

    #[inline]
    fn index(&self, index: ops::Range<usize>) -> &str {
        &self[..][index]
    }
}
impl<A: Alloc> ops::Index<ops::RangeTo<usize>> for IString<A> {
    type Output = str;

    #[inline]
    fn index(&self, index: ops::RangeTo<usize>) -> &str {
        &self[..][index]
    }
}
impl<A: Alloc> ops::Index<ops::RangeFrom<usize>> for IString<A> {
    type Output = str;

    #[inline]
    fn index(&self, index: ops::RangeFrom<usize>) -> &str {
        &self[..][index]
    }
}
impl<A: Alloc> ops::Index<ops::RangeFull> for IString<A> {
    type Output = str;

    #[inline]
    fn index(&self, _index: ops::RangeFull) -> &str {
        self.as_str()
    }
}
impl<A: Alloc> ops::Index<ops::RangeInclusive<usize>> for IString<A> {
    type Output = str;

    #[inline]
    fn index(&self, index: ops::RangeInclusive<usize>) -> &str {
        Index::index(&**self, index)
    }
}
impl<A: Alloc> ops::Index<ops::RangeToInclusive<usize>> for IString<A> {
    type Output = str;

    #[inline]
    fn index(&self, index: ops::RangeToInclusive<usize>) -> &str {
        Index::index(&**self, index)
    }
}

#[test]
fn main() {
    let p1 = "Hello World!";
    let p2 = "Hello World! .........xyz";
    let p3 = " .........xyz";
    
    let s1 = IString::from(p1);
    assert_eq!(s1, p1);
    
    let s2 = IString::from(p2);
    assert_eq!(s2, p2);
    
    let mut s3 = s1.clone();
    s3.push_str(p3);
    assert_eq!(s3, p2);
}
