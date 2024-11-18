use core::{fmt, str, convert};
use core::clone::Clone;
use core::iter::{FromIterator, IntoIterator, Extend};
use core::ops::{self, Index, Add, AddAssign};
use core::borrow::Borrow;
use alloc::{string::String, vec::Vec};
use alloc::borrow::Cow;

#[cfg(feature="ts")]
use alloc::{borrow::ToOwned, format};

use crate::ibytes::IBytes;
use crate::FromUtf8Error;

#[derive(Clone)]
#[cfg_attr(feature="size", derive(datasize::DataSize))]
#[cfg_attr(feature="ts", derive(ts_rs::TS), ts(type="String"))]
pub struct IString {
    pub (crate) bytes: IBytes,
}


impl IString {
    #[inline]
    pub fn new() -> IString {
        IString {
            bytes: IBytes::new()
        }
    }
    #[inline]
    pub fn with_capacity(capacity: usize) -> IString {
        IString {
            bytes: IBytes::with_capacity(capacity)
        }
    }
    #[inline(always)]
    pub unsafe fn set_len(&mut self, new_len: usize) {
        self.bytes.set_len(new_len);
    }
    
    #[inline(always)]
    pub fn capacity(&self) -> usize {
        self.bytes.capacity()
    }
    
    /// un-inline the string and expand the capacity to `cap`.
    ///
    /// does nothing if it isn't inlined.
    /// panics, if `cap` < `self.len()`
    #[inline(always)]
    pub fn move_to_heap(&mut self, cap: usize) {
        self.bytes.move_to_heap(cap);
    }
    
    /// if the strings fits inline, make it inline,
    /// otherwhise shrink the capacity to the `self.len()`.
    #[inline(always)]
    pub fn shrink(&mut self) {
        self.bytes.shrink();
    }
    
    #[inline]
    pub fn push_str(&mut self, s: &str) {
        self.bytes.extend_from_slice(s.as_bytes());
    }
    
    #[inline(always)]
    pub unsafe fn from_raw_parts(buf: *mut u8, length: usize, capacity: usize) -> IString {
        String::from_raw_parts(buf, length, capacity).into()
    }
 
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.bytes.reserve(additional);
    }
    
    #[inline]
    pub fn reserve_exact(&mut self, additional: usize) {
        self.bytes.reserve_exact(additional);
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

    pub fn from_utf8(bytes: IBytes) -> Result<IString, FromUtf8Error<IBytes>> {
        match str::from_utf8(bytes.as_slice()) {
            Ok(_) => Ok(IString { bytes }),
            Err(error) => Err(FromUtf8Error {
                bytes,
                error
            })
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
        IString {
            bytes: IBytes::from(s.into_bytes())
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
    fn into(self) -> String {
        unsafe {
            String::from_utf8_unchecked(self.bytes.into())
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

define_common_string!(IString, IStringUnion);
