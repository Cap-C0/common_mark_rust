use crate::peekable_char_indices::*;
use std::fmt::Debug;
use std::ops::{Add, Range};

pub trait LeafContainerInline: Default + Debug {
    type Offset: Add<usize, Output = Self::Offset> + Ord + Clone + Debug + Copy;
    type Chars<'a>: PeekableCharIndices<Offset = Self::Offset> + Debug
    where
        Self: 'a;

    // fn to_html(&self, string_builder: &mut String, lrd_table: &LRDTable);
    fn is_empty(&self) -> bool;
    fn char_iter(&self) -> impl Iterator<Item = char>;
    fn as_pci(&self) -> impl PeekableCharIndices<Offset = Self::Offset>;
    fn chars_in_range<'a>(&'a self, range: Range<Self::Offset>) -> Self::Chars<'a>;
    fn add_extra_space_and_line(&mut self, extra_space: usize, line: &str);
    fn from_extra_space_and_line(extra_space: usize, line: &str) -> Self {
        let mut out = Self::default();
        out.add_extra_space_and_line(extra_space, line);
        out
    }
    ///
    fn split(self, offset: Self::Offset) -> (Self, Self);
    /// This is needed for when setext inlining
    fn remove_final_spaces(&mut self);
    fn append_other(&mut self, other: Self);
    fn remove_up_to(&mut self, offset: Self::Offset);
}

impl LeafContainerInline for String {
    type Offset = usize;
    type Chars<'a>
        = BorrowedStringPCI<'a>
    where
        Self: 'a;
    // fn to_html(&self, string_builder: &mut String, lrd_table: &LRDTable) {
    //     for ic in parse_inline(self, lrd_table) {
    //         ic.to_html(self, string_builder, lrd_table);
    //     }
    // }
    fn is_empty(&self) -> bool {
        self.is_empty()
    }
    fn char_iter(&self) -> impl Iterator<Item = char> {
        self.chars()
    }

    fn as_pci(&self) -> impl PeekableCharIndices<Offset = Self::Offset> {
        BorrowedStringPCI::new(self)
    }

    fn chars_in_range<'a>(&'a self, range: Range<Self::Offset>) -> Self::Chars<'a> {
        BorrowedStringPCI::new(&self[range])
    }
    fn add_extra_space_and_line(&mut self, extra_space: usize, line: &str) {
        if !self.is_empty() {
            self.push('\n');
        }
        for _ in 0..extra_space {
            self.push(' ');
        }
        self.push_str(line);
    }

    fn remove_final_spaces(&mut self) {
        while self.ends_with(' ') || self.ends_with('\t') {
            self.pop();
        }
    }
    fn append_other(&mut self, other: Self) {
        self.push_str(&other);
    }
    fn remove_up_to(&mut self, offset: Self::Offset) {
        *self = self[offset..].into();
    }

    fn split(self, offset: Self::Offset) -> (Self, Self) {
        (self[0..offset].into(), self[offset..].into())
    }
}
