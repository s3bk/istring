macro_rules! impl_bincode {
    ($bincode:ident, $mod:ident) => {

mod $mod {
    use $bincode::{
        de::{BorrowDecode, BorrowDecoder, Decode, Decoder, read::{Reader, BorrowReader}},
        enc::{Encode, Encoder},
        error::{DecodeError, EncodeError}
    };

    use crate::small::{SmallBytes, SmallString, INLINE_CAPACITY, Inline};
    use alloc::vec;

    impl<Context> Decode<Context> for SmallBytes {
        // Required method
        fn decode<D: Decoder<Context = Context>>(decoder: &mut D) -> Result<Self, DecodeError> {
            let v = u64::decode(decoder)?;
            let len: usize = v.try_into().map_err(|_| DecodeError::OutsideUsizeRange(v))?;
            if len <= INLINE_CAPACITY {
                let mut data = [0; INLINE_CAPACITY];
                decoder.reader().read(&mut data[..len])?;
                Ok(unsafe { SmallBytes::from_inline(
                    Inline { data, len: len as u8 },
                )})
            } else {
                let mut buf = vec![0; len];
                decoder.reader().read(&mut buf)?;
                Ok(buf.into())
            }
        }
    }
    impl<'de, Context> BorrowDecode<'de, Context> for SmallBytes {
        fn borrow_decode<D: BorrowDecoder<'de, Context = Context>>(
            decoder: &mut D,
        ) -> core::result::Result<Self, DecodeError> {
            let v = u64::decode(decoder)?;
            let len: usize = v.try_into().map_err(|_| DecodeError::OutsideUsizeRange(v))?;
            let bytes = decoder.borrow_reader().take_bytes(len)?;
            Ok(bytes.into())
        }
    }

    impl<Context> Decode<Context> for SmallString {
        fn decode<D: Decoder<Context = Context>>(decoder: &mut D) -> Result<Self, DecodeError> {
            let bytes = SmallBytes::decode(decoder)?;
            match core::str::from_utf8(&*bytes) {
                Ok(_) => Ok(SmallString { bytes }),
                Err(e) => Err(DecodeError::Utf8 {
                    inner: e,
                })
            }
        }
    }
    impl<'de, Context> BorrowDecode<'de, Context> for SmallString {
        fn borrow_decode<D: BorrowDecoder<'de, Context = Context>>(
            decoder: &mut D,
        ) -> core::result::Result<Self, DecodeError> {
            let v = u64::decode(decoder)?;
            let len: usize = v.try_into().map_err(|_| DecodeError::OutsideUsizeRange(v))?;
            let bytes = decoder.borrow_reader().take_bytes(len)?;
            let str = core::str::from_utf8(bytes).map_err(|e| DecodeError::Utf8 { inner: e })?;
            Ok(str.into())
        }
    }

    impl Encode for SmallBytes {
        fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
            self.as_slice().encode(encoder)
        }
    }
    impl Encode for SmallString {
        fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
            self.bytes.encode(encoder)
        }
    }
}

    };
}

#[cfg(feature="bincode")]
impl_bincode!(bincode, impl_bincode);

#[cfg(feature="bincode-next")]
impl_bincode!(bincode_next, impl_bincode_next);
