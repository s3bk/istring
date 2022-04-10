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

#[macro_use]
mod common;

pub mod istring;
pub mod small;

pub use crate::istring::{IString};
pub use crate::small::SmallString;

#[cfg(feature="size")]
impl datasize::DataSize for IString {
    const IS_DYNAMIC: bool = true;
    const STATIC_HEAP_SIZE: usize = core::mem::size_of::<IString>();

    fn estimate_heap_size(&self) -> usize {
        if self.is_inline() {
            core::mem::size_of::<IString>()
        } else {
            core::mem::size_of::<IString>() + self.capacity()
        }
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
