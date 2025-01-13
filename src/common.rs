macro_rules! define_common_bytes {
    ($name:ident, $union:ident) => {
impl $name {
    /// view as Inline.
    ///
    /// Panics if the string isn't inlined
    #[inline(always)]
    pub unsafe fn as_inline(&mut self) -> &mut Inline {
        debug_assert!(self.is_inline());
        &mut self.union.inline
    }

    /// view as Heap.
    ///
    /// Panics if the string isn't on the Heap
    #[inline(always)]
    pub unsafe fn as_heap(&mut self) -> &mut Heap {
        debug_assert!(!self.is_inline());
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
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        unsafe {
            if self.is_inline() {
                &mut self.union.inline.data as *mut u8
            } else {
                self.union.heap.ptr
            }
        }
    }
    #[inline(always)]
    pub fn as_slice(&self) -> &[u8] {
        let len = self.len();
        unsafe {
            if self.is_inline() {
                &self.union.inline.data[.. len]
            } else {
                slice::from_raw_parts(self.union.heap.ptr, len)
            }
        }
    }
    
    #[inline(always)]
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe {
            let len = self.len();
            if self.is_inline() {
                &mut self.union.inline.data[.. len]
            } else {
                slice::from_raw_parts_mut(self.union.heap.ptr, len)
            }
        }
    }
    /// Deconstruct into the Inline part and the allocator
    ///
    /// Assumes the string is inlined and panics otherwhise.
    #[inline(always)]
    pub fn to_inline(self) -> Inline {
        assert_eq!(self.is_inline(), true);
        unsafe {
            let mut inline = self.union.inline;
            mem::forget(self);
            
            inline.len &= !IS_INLINE; // clear the bit
            inline
        }
    }
    pub unsafe fn from_heap(heap: Heap) -> Self {
        let union = $union { heap: heap };
        assert_eq!(union.inline.len & IS_INLINE, 0);
        $name { union: union }
    }
    pub unsafe fn from_inline(mut inline: Inline) -> Self {
        assert!(inline.len as usize <= INLINE_CAPACITY);
        inline.len |= IS_INLINE; // set inline bit
        $name {
            union: $union { inline: inline },
        }
    }
    /// Deconstruct into the Heap part and the allocator
    ///
    /// Assumes it is heap-state, panics otherwhise. (you may want to call move_to_heap before this.)
    /// The caller is responsible to adequatly dispose the owned memory. (for example by calling $name::from_heap)
    #[inline(always)]
    pub fn to_heap(self) -> Heap {
        assert_eq!(self.is_inline(), false);
        unsafe {
            let heap = self.union.heap;
            mem::forget(self);
            
            heap
        }
    }
}
impl ops::Deref for $name {
    type Target = [u8];
    
    #[inline(always)]
    fn deref(&self) -> &[u8] {
        self.as_slice()
    }
}
impl ops::DerefMut for $name {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut [u8] {
        self.as_mut_slice()
    }
}
impl fmt::Debug for $name {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        <[u8] as fmt::Debug>::fmt(&*self, f)
    }
}
impl PartialEq<[u8]> for $name {
    #[inline(always)]
    fn eq(&self, rhs: &[u8]) -> bool {
        self.as_slice() == rhs
    }
}
impl PartialEq for $name {
    fn eq(&self, rhs: &Self) -> bool {
        self.as_slice().eq(rhs.as_slice())
    }
}
impl Eq for $name {}
impl core::hash::Hash for $name {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.as_slice().hash(state);
    }
}
impl cmp::PartialOrd for $name {
    #[inline(always)]
    fn partial_cmp(&self, rhs: &Self) -> Option<cmp::Ordering> {
        self.as_slice().partial_cmp(rhs.as_slice())
    }
    #[inline(always)]
    fn lt(&self, rhs: &Self) -> bool {
        self.as_slice().lt(rhs.as_slice())
    }
    #[inline(always)]
    fn le(&self, rhs: &Self) -> bool {
        self.as_slice().le(rhs.as_slice())
    }
    #[inline(always)]
    fn gt(&self, rhs: &Self) -> bool {
        self.as_slice().gt(rhs.as_slice())
    }
    #[inline(always)]
    fn ge(&self, rhs: &Self) -> bool {
        self.as_slice().ge(rhs.as_slice())
    }
}
impl cmp::Ord for $name {
    #[inline(always)]
    fn cmp(&self, other: &$name) -> cmp::Ordering {
        self.as_slice().cmp(other.as_slice())
    }
}
impl ops::Index<ops::Range<usize>> for $name {
    type Output = [u8];

    #[inline]
    fn index(&self, index: ops::Range<usize>) -> &[u8] {
        &self[..][index]
    }
}
impl ops::Index<ops::RangeTo<usize>> for $name {
    type Output = [u8];

    #[inline]
    fn index(&self, index: ops::RangeTo<usize>) -> &[u8] {
        &self[..][index]
    }
}
impl ops::Index<ops::RangeFrom<usize>> for $name {
    type Output = [u8];

    #[inline]
    fn index(&self, index: ops::RangeFrom<usize>) -> &[u8] {
        &self[..][index]
    }
}
impl ops::Index<ops::RangeFull> for $name {
    type Output = [u8];

    #[inline]
    fn index(&self, _index: ops::RangeFull) -> &[u8] {
        self.as_slice()
    }
}
impl ops::Index<ops::RangeInclusive<usize>> for $name {
    type Output = [u8];

    #[inline]
    fn index(&self, index: ops::RangeInclusive<usize>) -> &[u8] {
        Index::index(&**self, index)
    }
}
impl ops::Index<ops::RangeToInclusive<usize>> for $name {
    type Output = [u8];

    #[inline]
    fn index(&self, index: ops::RangeToInclusive<usize>) -> &[u8] {
        Index::index(&**self, index)
    }
}

impl Borrow<[u8]> for $name {
    fn borrow(&self) -> &[u8] {
        self.as_slice()
    }
}
    }
}

macro_rules! define_common_string {
    ($name:ident, $union:ident) => {
impl $name {
    #[inline(always)]
    pub fn as_str(&self) -> &str {
        unsafe {
            str::from_utf8_unchecked(self.bytes.as_slice())
        }
    }
    
    #[inline(always)]
    pub fn as_mut_str(&mut self) -> &mut str {
        unsafe {
            str::from_utf8_unchecked_mut(self.bytes.as_mut_slice())
        }
    }
    
    
}
impl $name {
    #[inline(always)]
    pub fn into_bytes(self) -> Vec<u8> {
        let s: String = self.into();
        s.into_bytes()
    }
}

impl<'a> Into<String> for &'a $name {
    #[inline(always)]
    fn into(self) -> String {
        String::from(self.as_str())
    }
}
impl ops::Deref for $name {
    type Target = str;
    
    #[inline(always)]
    fn deref(&self) -> &str {
        self.as_str()
    }
}

#[cfg(feature="std")]
impl AsRef<str> for $name {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
#[cfg(feature="std")]
impl AsRef<[u8]> for $name {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}
impl fmt::Debug for $name {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        <str as fmt::Debug>::fmt(&*self, f)
    }
}
impl fmt::Display for $name {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        <str as fmt::Display>::fmt(&*self, f)
    }
}

impl PartialEq<str> for $name {
    #[inline(always)]
    fn eq(&self, rhs: &str) -> bool {
        self.as_str() == rhs
    }
}
impl<'a> PartialEq<&'a str> for $name {
    #[inline(always)]
    fn eq(&self, rhs: &&'a str) -> bool {
        self.as_str() == *rhs
    }
}
impl PartialEq<String> for $name {
    #[inline(always)]
    fn eq(&self, rhs: &String) -> bool {
        self.as_str() == rhs
    }
}
impl PartialEq for $name {
    fn eq(&self, rhs: &Self) -> bool {
        self.as_str().eq(rhs.as_str())
    }
}
impl Eq for $name {}
impl core::hash::Hash for $name {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}
impl core::cmp::PartialOrd for $name {
    #[inline(always)]
    fn partial_cmp(&self, rhs: &Self) -> Option<core::cmp::Ordering> {
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
impl core::cmp::Ord for $name {
    #[inline(always)]
    fn cmp(&self, other: &$name) -> core::cmp::Ordering {
        self.as_str().cmp(other.as_str())
    }
}
impl ops::Index<ops::Range<usize>> for $name {
    type Output = str;

    #[inline]
    fn index(&self, index: ops::Range<usize>) -> &str {
        &self[..][index]
    }
}
impl ops::Index<ops::RangeTo<usize>> for $name {
    type Output = str;

    #[inline]
    fn index(&self, index: ops::RangeTo<usize>) -> &str {
        &self[..][index]
    }
}
impl ops::Index<ops::RangeFrom<usize>> for $name {
    type Output = str;

    #[inline]
    fn index(&self, index: ops::RangeFrom<usize>) -> &str {
        &self[..][index]
    }
}
impl ops::Index<ops::RangeFull> for $name {
    type Output = str;

    #[inline]
    fn index(&self, _index: ops::RangeFull) -> &str {
        self.as_str()
    }
}
impl ops::Index<ops::RangeInclusive<usize>> for $name {
    type Output = str;

    #[inline]
    fn index(&self, index: ops::RangeInclusive<usize>) -> &str {
        Index::index(&**self, index)
    }
}
impl ops::Index<ops::RangeToInclusive<usize>> for $name {
    type Output = str;

    #[inline]
    fn index(&self, index: ops::RangeToInclusive<usize>) -> &str {
        Index::index(&**self, index)
    }
}

impl Borrow<str> for $name {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

    }
}
