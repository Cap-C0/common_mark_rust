use crate::inline_str_collection::LeafContainerInline;
use std::collections::HashMap;
use std::ops::Range;

pub type LRDTable<'a, T: 'a + LeafContainerInline> =
    HashMap<String, (T, Range<T::Offset>, Option<Range<T::Offset>>)>;
