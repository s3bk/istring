// Copyright 2022 Sebastian Köln

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

#![no_std]

/*!
A replacement for String that allows storing strings of length up to sizeof<String>() - 1 without a heap allocation

That means on 32bit machines: size_of::<IString>() == 12 bytes, inline capacity: 11 bytes
on 64bit machines: size_of::<IString>() == 24 bytes, inline capacity: 23 bytes
*/

extern crate alloc;

#[cfg(feature="std")]
extern crate std;

#[macro_use]
mod common;

pub mod istring;
pub mod small;
pub mod ibytes;

pub use crate::istring::{IString};
pub use crate::ibytes::IBytes;
pub use crate::small::{SmallBytes, SmallString};

#[derive(Debug)]
pub struct FromUtf8Error<T> {
    bytes: T,
    error: core::str::Utf8Error,
}
impl<T: core::ops::Deref<Target=[u8]>> FromUtf8Error<T> {
    pub fn as_bytes(&self) -> &[u8] {
        &*self.bytes
    }
    pub fn into_bytes(self) -> T {
        self.bytes
    }
    pub fn utf8_error(&self) -> core::str::Utf8Error {
        self.error
    }
}
#[cfg(feature="std")]
impl<T: std::fmt::Debug> std::fmt::Display for FromUtf8Error<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        self.error.fmt(f)
    }
}
#[cfg(feature="std")]
impl<T: std::fmt::Debug> std::error::Error for FromUtf8Error<T> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.error.source()
    }
}


#[cfg(feature="serialize")]
use serde::{Serialize, Serializer, Deserialize, Deserializer};

#[cfg(feature="serialize")]
impl Serialize for IString {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error>
    {
        self.as_str().serialize(serializer)
    }
}

#[cfg(feature="serialize")]
impl<'de> Deserialize<'de> for IString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: Deserializer<'de> {
        let s = alloc::string::String::deserialize(deserializer)?;
        let mut s = IString::from(s);
        s.shrink();
        Ok(s)
    }
}

#[cfg(feature="serialize")]
impl Serialize for SmallString {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error>
    {
        self.as_str().serialize(serializer)
    }
}

#[cfg(feature="serialize")]
impl<'de> Deserialize<'de> for SmallString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: Deserializer<'de> {
        let s = alloc::string::String::deserialize(deserializer)?;
        Ok(SmallString::from(s))
    }
}
