use core::{borrow::Borrow, fmt::Debug, hash::Hash, ops::Deref};

#[cfg(feature="ts")]
use alloc::{borrow::ToOwned, string::String, format};

#[derive(Copy, Clone)]
#[cfg_attr(feature="ts", derive(ts_rs::TS), ts(type="Vec<u8>"))]
pub struct TinyBytes {
    len: u8,
    buf: [u8; 7]
}

#[derive(Copy, Clone)]
#[cfg_attr(feature="ts", derive(ts_rs::TS), ts(type="String"))]
pub struct TinyString(TinyBytes);

impl TinyBytes {
    #[inline]
    pub const fn new(s: &[u8]) -> Option<Self> {
        let len = s.len();
        if len >= 8 {
            return None;
        }
        let mut buf = [0; 7];
        let mut i = 0;
        while i < len {
            buf[i] = s[i];
            i += 1;
        }
        
        Some(TinyBytes {
            len: s.len() as u8,
            buf
        })
    }

    #[inline(always)]
    pub fn as_bytes(&self) -> &[u8] {
        &**self
    }
}
impl Deref for TinyBytes {
    type Target = [u8];

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.buf[.. self.len as usize]
    }
}
impl Deref for TinyString {
    type Target = str;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        unsafe {
            core::str::from_utf8_unchecked(&*self.0)
        }
    }
}
impl AsRef<[u8]> for TinyBytes {
    #[inline(always)]
    fn as_ref(&self) -> &[u8] {
        &**self
    }
}
impl AsRef<[u8]> for TinyString {
    #[inline(always)]
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}
impl AsRef<str> for TinyString {
    #[inline(always)]
    fn as_ref(&self) -> &str {
        &**self
    }
}

impl TinyString {
    pub const fn new(s: &str) -> Option<Self> {
        match TinyBytes::new(s.as_bytes()) {
            Some(b) => Some(TinyString(b)),
            None => None
        }
    }
    #[inline]
    pub fn as_str(&self) -> &str {
        &**self
    }
}

impl Debug for TinyBytes {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        (**self).fmt(f)
    }
}
impl Debug for TinyString {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        (**self).fmt(f)
    }
}

impl<T: AsRef<[u8]>> PartialEq<T> for TinyBytes {
    #[inline]
    fn eq(&self, other: &T) -> bool {
        self.as_bytes().eq(other.as_ref())
    }
}
impl Eq for TinyBytes {}

impl<T: AsRef<[u8]>> PartialOrd<T> for TinyBytes {
    #[inline]
    fn partial_cmp(&self, other: &T) -> Option<core::cmp::Ordering> {
        self.as_bytes().partial_cmp(other.as_ref())
    }
}
impl Ord for TinyBytes {
    #[inline]
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.as_bytes().cmp(other.as_bytes())
    }
}

impl<T: AsRef<str>> PartialEq<T> for TinyString {
    #[inline]
    fn eq(&self, other: &T) -> bool {
        self.as_str().eq(other.as_ref())
    }
}
impl Eq for TinyString {}

impl<T: AsRef<str>> PartialOrd<T> for TinyString {
    #[inline]
    fn partial_cmp(&self, other: &T) -> Option<core::cmp::Ordering> {
        self.as_str().partial_cmp(other.as_ref())
    }
}
impl Ord for TinyString {
    #[inline]
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.as_bytes().cmp(other.as_bytes())
    }
}
impl Hash for TinyBytes {
    #[inline]
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.as_bytes().hash(state)
    }
}
impl Hash for TinyString {
    #[inline]
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.as_str().hash(state)
    }
}
impl Borrow<[u8]> for TinyBytes {
    #[inline]
    fn borrow(&self) -> &[u8] {
        self.as_bytes()
    }
}
impl Borrow<str> for TinyString {
    #[inline]
    fn borrow(&self) -> &str {
        self.as_str()
    }
}
impl From<char> for TinyString {
    #[inline]
    fn from(value: char) -> Self {
        let mut buf = [0; 7];
        let len = value.encode_utf8(&mut buf).len() as u8;
        TinyString(TinyBytes { len, buf })
    }
}
