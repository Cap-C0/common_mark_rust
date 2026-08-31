use std::cmp::Ordering;
use std::cmp::Ordering::*;

use crate::peekable_char_indices::{BorrowedStringPCI, PeekableCharIndices};

#[derive(PartialEq, Eq)]
struct IndexPosition {
    row: usize,
    offset: Option<usize>,
}

impl From<(usize, Option<usize>)> for IndexPosition {
    fn from(tup: (usize, Option<usize>)) -> Self {
        Self {
            row: tup.0,
            offset: tup.1,
        }
    }
}

impl Ord for IndexPosition {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.row.cmp(&other.row) {
            Equal => match (self.offset, self.offset) {
                (Some(_), None) => Less,
                (None, None) => Equal,
                (None, Some(_)) => Greater,
                (Some(x), Some(y)) => x.cmp(&y),
            },
            o => o,
        }
    }
}

impl PartialOrd for IndexPosition {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

pub struct FakeStringCollection<'a> {
    lines: Vec<&'a str>,
}

impl<'a> FakeStringCollection<'a> {
    pub fn to_peekable_iter(&self) -> FakeStringIterator<'_> {
        FakeStringIterator {
            current_iter: self.lines[0].into(),
            base_collection: self,
            peeked: None,
            current_row: 0,
        }
    }
}

#[derive(Clone)]
pub struct FakeStringIterator<'a> {
    current_iter: BorrowedStringPCI<'a>,
    base_collection: &'a FakeStringCollection<'a>,
    peeked: Option<Option<(usize, char)>>,
    current_row: usize,
    // upper_bound: Option<IndexPosition>,
}

impl<'a> PeekableCharIndices<IndexPosition> for FakeStringIterator<'a> {
    fn next_and_index(&mut self) -> Option<(IndexPosition, char)> {
        match self.peeked {
            Some(child_op) => {
                self.peeked.take();
                match child_op {
                    Some((row_offset, c)) => Some(((self.current_row, Some(row_offset)).into(), c)),
                    None => {
                        if self.current_row < self.base_collection.lines.len() - 1 {
                            let row_out = self.current_row;
                            self.current_row += 1;
                            self.current_iter = self.base_collection.lines[self.current_row].into();
                            Some(((row_out, None).into(), '\n'))
                        } else {
                            None
                        }
                    }
                }
            }
            None => match self.current_iter.next_and_index() {
                Some((row_offset, c)) => Some(((self.current_row, Some(row_offset)).into(), c)),
                None => {
                    if self.current_row < self.base_collection.lines.len() - 1 {
                        let row_out = self.current_row;
                        self.current_row += 1;
                        self.current_iter = self.base_collection.lines[self.current_row].into();
                        Some(((row_out, None).into(), '\n'))
                    } else {
                        None
                    }
                }
            },
        }
    }

    fn peek_and_index(&mut self) -> Option<(IndexPosition, char)> {
        if self.peeked.is_none() {
            self.peeked = Some(self.current_iter.next_and_index());
        }
        match self.peeked.unwrap() {
            Some((row_offset, c)) => Some(((self.current_row, Some(row_offset)).into(), c)),
            None => {
                if self.current_row < self.base_collection.lines.len() - 1 {
                    Some(((self.current_row, None).into(), '\n'))
                } else {
                    None
                }
            }
        }
    }

    fn offset(&self) -> IndexPosition {
        (
            self.current_row,
            self.peeked
                .map_or(Some(self.current_iter.offset()), |peeked_op| {
                    peeked_op.map(|(row_offset, _c)| row_offset)
                }),
        )
            .into()
    }
}

impl<'a> Iterator for FakeStringIterator<'a> {
    type Item = char;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_and_index().map(|t| t.1)
    }
}
