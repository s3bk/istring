use core::{fmt, slice, str, convert, mem, cmp};
use core::clone::Clone;
use core::ops::{self, Index};
use core::borrow::Borrow;
use alloc::{string::String, vec::Vec};
use alloc::boxed::Box;
use crate::FromUtf8Error;

#[cfg(feature="ts")]
use alloc::{borrow::ToOwned, format};

const IS_INLINE: u8 = 1 << 7;
const LEN_MASK: u8 = !IS_INLINE;

#[cfg(target_pointer_width="64")]
const INLINE_CAPACITY: usize = 15;
#[cfg(target_pointer_width="32")]
const INLINE_CAPACITY: usize = 7;

#[allow(unused)]
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

union SmallBytesUnion {
    inline: Inline,
    heap:   Heap
}

#[cfg_attr(feature="ts", derive(ts_rs::TS), ts(type="Vec<u8>"))]
pub struct SmallBytes {
    union: SmallBytesUnion,
}
unsafe impl Send for SmallBytes {}
unsafe impl Sync for SmallBytes {}

#[derive(Clone)]
#[cfg_attr(feature="size", derive(datasize::DataSize))]
#[cfg_attr(feature="ts", derive(ts_rs::TS), ts(type="String"))]

pub struct SmallString {
    bytes: SmallBytes,
}

#[cfg(feature="rkyv")]
mod rkyv_impl {
    use rkyv::{
        string::ArchivedString,
        Archive, Deserialize, DeserializeUnsized, Serialize, SerializeUnsized, Place
    };
    use rancor::{Fallible, Source};
    use super::SmallString;

    impl Archive for SmallString {
        type Archived = rkyv::string::ArchivedString;
        type Resolver = rkyv::string::StringResolver;

        #[inline]
        fn resolve(&self, resolver: Self::Resolver, out: Place<Self::Archived>) {
            rkyv::string::ArchivedString::resolve_from_str(self.as_str(), resolver, out);
        }
    }

    #[cfg(feature="rkyv")]
    impl<S: Fallible + ?Sized> Serialize<S> for SmallString
    where
        str: SerializeUnsized<S>,
        S::Error: Source
    {
        #[inline]
        fn serialize(&self, serializer: &mut S) -> Result<Self::Resolver, S::Error> {
            ArchivedString::serialize_from_str(self.as_str(), serializer)
        }
    }
    impl<D: Fallible + ?Sized> Deserialize<SmallString, D> for ArchivedString
    where
        str: DeserializeUnsized<str, D>,
    {
        #[inline]
        fn deserialize(&self, _: &mut D) -> Result<SmallString, D::Error> {
            Ok(self.as_str().into())
        }
    }
    impl PartialEq<SmallString> for ArchivedString {
        #[inline]
        fn eq(&self, other: &SmallString) -> bool {
            PartialEq::eq(self.as_str(), other.as_str())
        }
    }
    
    impl PartialEq<ArchivedString> for SmallString {
        #[inline]
        fn eq(&self, other: &ArchivedString) -> bool {
            PartialEq::eq(other.as_str(), self.as_str())
        }
    }
}

#[test]
fn test_layout() {
    let s = SmallBytesUnion { inline: Inline { data: [0; INLINE_CAPACITY], len: IS_INLINE } };
    let heap = unsafe { s.heap };
    assert_eq!(heap.len, MAX_CAPACITY + 1);
}

#[inline(always)]
fn box_slice(s: &[u8]) -> Box<[u8]> {
    Box::from(s)
}
#[inline(always)]
fn box_slice_into_raw_parts(mut s: Box<[u8]>) -> (*mut u8, usize) {
    let len = s.len();
    let ptr = s.as_mut_ptr();
    mem::forget(s);
    (ptr, len)
}
#[inline(always)]
unsafe fn box_slice_from_raw_parts(ptr: *mut u8, len: usize) -> Box<[u8]> {
    let ptr = slice::from_raw_parts_mut(ptr, len) as *mut [u8];
    Box::from_raw(ptr)
}

impl SmallBytes {
    #[inline(always)]
    pub fn new() -> SmallBytes {
        unsafe {
            SmallBytes::from_inline(
                Inline { data: [0; INLINE_CAPACITY], len: 0 },
            )
        }
    }
}
impl<'a> From<&'a [u8]> for SmallBytes {
    #[inline]
    fn from(s: &[u8]) -> SmallBytes {
        let len = s.len();
        unsafe {
            if len > INLINE_CAPACITY {
                let s = box_slice(s);
                let (ptr, len) = box_slice_into_raw_parts(s);
                SmallBytes::from_heap(
                    Heap {
                        ptr,
                        len
                    },
                )
            } else {
                let mut data = [0; INLINE_CAPACITY];
                data[.. len].copy_from_slice(s);
                SmallBytes::from_inline(
                    Inline { data, len: len as u8 },
                )
            }
        }
    }
}

impl SmallString {
    #[inline(always)]
    pub fn new() -> SmallString {
        SmallString {
            bytes: SmallBytes::new()
        }
    }
    pub fn from_utf8(bytes: SmallBytes) -> Result<SmallString, FromUtf8Error<SmallBytes>> {
        match str::from_utf8(bytes.as_slice()) {
            Ok(_) => Ok(SmallString { bytes }),
            Err(error) => Err(FromUtf8Error {
                bytes,
                error
            })
        }
    }
}
impl Drop for SmallBytes {
    #[inline]
    fn drop(&mut self) {
        if !self.is_inline() {
            unsafe {
                box_slice_from_raw_parts(self.union.heap.ptr, self.union.heap.len);
            }
        }
    }
}
impl<'a> convert::From<&'a str> for SmallString {
    #[inline]
    fn from(s: &'a str) -> SmallString {
        SmallString {
            bytes: SmallBytes::from(s.as_bytes())
        }
    }
}
impl convert::From<Vec<u8>> for SmallBytes {
    #[inline]
    fn from(s: Vec<u8>) -> SmallBytes {
        let len = s.len();
        if len <= INLINE_CAPACITY {
            return SmallBytes::from(s.as_slice());
        }

        unsafe {
            let s = s.into_boxed_slice();
            let (ptr, len) = box_slice_into_raw_parts(s);
            let heap = Heap {
                ptr,
                len,
            };

            SmallBytes::from_heap(
                heap,
            )
        }
    }
}
impl convert::From<String> for SmallString {
    #[inline]
    fn from(s: String) -> SmallString {
        SmallString {
            bytes: SmallBytes::from(s.into_bytes())
        }
    }
}
impl Into<Vec<u8>> for SmallBytes {
    #[inline]
    fn into(self) -> Vec<u8> {
        let len = self.len();
        if self.is_inline() {
            self.as_slice().into()
        } else {
            unsafe {
                let s = box_slice_from_raw_parts(self.union.heap.ptr, len);
                // the SmallString must not drop
                mem::forget(self);

                Vec::from(s)
            }
        }
    }
}
impl Into<String> for SmallString {
    #[inline]
    fn into(self) -> String {
        unsafe {
            String::from_utf8_unchecked(self.bytes.into())
        }
    }
}
impl Clone for SmallBytes {
    #[inline]
    fn clone(&self) -> SmallBytes {
        unsafe {
            if self.is_inline() {
                // simple case
                SmallBytes {
                    union: SmallBytesUnion { inline: self.union.inline },
                }
            } else {
                let len = self.len();
                let bytes = slice::from_raw_parts(self.union.heap.ptr, len);
                let (ptr, len) = box_slice_into_raw_parts(box_slice(bytes));
                SmallBytes::from_heap(
                    Heap {
                        ptr,
                        len
                    },
                )
            }
        }
    }
}
impl FromIterator<char> for SmallString {
    fn from_iter<T: IntoIterator<Item=char>>(iter: T) -> Self {
        let mut buf = [0; INLINE_CAPACITY];
        let mut pos = 0;
        let mut iter = iter.into_iter();
        while let Some(c) = iter.next() {
            if pos + c.len_utf8() > INLINE_CAPACITY {
                let mut s = String::with_capacity(32);
                s.push_str(unsafe { str::from_utf8_unchecked(&buf[..pos]) });
                s.push(c);
                s.extend(iter);
                return s.into();
            }
            pos += c.encode_utf8(&mut buf[pos..]).len();
        }
        let bytes = unsafe { SmallBytes::from_inline(
            Inline { data: buf, len: pos as u8 },
        ) };
        SmallString { bytes }
    }
}
impl From<char> for SmallString {
    fn from(c: char) -> SmallString {
        let mut buf = [0; INLINE_CAPACITY];
        let len = c.encode_utf8(&mut buf).len();
        let bytes = unsafe { SmallBytes::from_inline(
            Inline { data: buf, len: len as u8 },
        ) };
        SmallString { bytes }
    }
}


#[cfg(feature="size")]
impl datasize::DataSize for SmallBytes {
    const IS_DYNAMIC: bool = true;
    const STATIC_HEAP_SIZE: usize = core::mem::size_of::<Self>();

    fn estimate_heap_size(&self) -> usize {
        if self.is_inline() {
            Self::STATIC_HEAP_SIZE
        } else {
            Self::STATIC_HEAP_SIZE + self.len()
        }
    }
}

define_common_string!(SmallString, SmallStringUnion);
define_common_bytes!(SmallBytes, SmallBytesUnion);
