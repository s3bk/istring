
use core::marker::PhantomData;

use serde::{Serialize, Serializer, Deserialize, Deserializer, de::Visitor};
use alloc::string::String;

use crate::{IString, SmallString, TinyString};


struct StringVisitor<T>(PhantomData<T>);

impl<T> StringVisitor<T> {
    fn new() -> Self {
        StringVisitor(PhantomData)
    }
}

impl<'de, T> Visitor<'de> for StringVisitor<T> where T: for<'a> From<&'a str> + From<String> {
    type Value = T;

    fn expecting(&self, formatter: &mut alloc::fmt::Formatter) -> alloc::fmt::Result {
        write!(formatter, "a string")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error, {

        Ok(T::from(v))
    }
    fn visit_string<E>(self, v: alloc::string::String) -> Result<Self::Value, E>
        where
            E: serde::de::Error, {
        
        Ok(T::from(v))
    }
}

struct TinyStringVisitor;

impl<'de> Visitor<'de> for TinyStringVisitor {
    type Value = TinyString;

    fn expecting(&self, formatter: &mut alloc::fmt::Formatter) -> alloc::fmt::Result {
        write!(formatter, "a string")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error, {

        use serde::de::Error;
        TinyString::new(v).ok_or(Error::invalid_length(v.len(), &"less than 8 bytes"))
    }
}

impl Serialize for IString {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error>
    {
        self.as_str().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for IString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: Deserializer<'de> {
        deserializer.deserialize_string(StringVisitor::<IString>::new())
    }
}

impl Serialize for SmallString {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error>
    {
        self.as_str().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SmallString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: Deserializer<'de> {
        deserializer.deserialize_string(StringVisitor::<SmallString>::new())
    }
}

impl Serialize for TinyString {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error>
    {
        self.as_str().serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for TinyString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: Deserializer<'de> {
        deserializer.deserialize_str(TinyStringVisitor)
    }
}

