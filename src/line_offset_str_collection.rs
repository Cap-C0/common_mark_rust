use std::cmp::Ordering::*;
use std::{
    cmp::Ordering,
    ops::{Add, Range},
};

use crate::peekable_char_indices::{BorrowedStringPCI, PeekableCharIndices};

//TODO: I think the peeking offset can actually not use options?
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub struct IndexPosition {
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

// because these are meant to point inside of a string, but are ultimately derived from a
// char_indices. There are situations where the underlying char_indices returns the offset at the
// end of the string, but the "peek" sees an empty return (None), and so while we would ostensibly like to
// recognize these cases as "equal" it shouldn't actually matter for the case of recreating the
// string we "ate" as we go along, because the "next if" checks are based on the "peek" which will
// return the "stronger" None offset for the sake of our comparison.
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

impl Add<usize> for IndexPosition {
    type Output = Self;
    fn add(self, rhs: usize) -> Self::Output {
        Self {
            row: self.row,
            offset: self.offset.map(|o| o + rhs),
        }
    }
}

pub struct LineOffsetStrCollection<'a> {
    /// (extra spaces, line_content)
    lines: Vec<(usize, &'a str)>,
}

impl<'a> LineOffsetStrCollection<'a> {
    pub fn to_peekable_iter(&self) -> LineOffsetStrIterator<'_> {
        LineOffsetStrIterator {
            current_iter: self.lines[0].1.into(),
            base_collection: self,
            peeked: None,
            current_row: 0,
        }
    }

    // These should only be used in cases of inline, in which case the extra spaces will be by
    // definition 0.
    pub fn chars_in_range(
        &self,
        range: Range<IndexPosition>,
    ) -> Box<dyn Iterator<Item = char> + '_> {
        let (start_row, start_offset) = range
            .start
            .offset
            .map_or((range.start.row + 1, 0), |o| (range.start.row, o));

        let iter_out: Box<dyn Iterator<Item = char>> = if start_row < self.lines.len() {
            Box::new(
                if start_row > range.start.row {
                    "\n".chars()
                } else {
                    "".chars()
                }
                .chain(CharsInMultiLineRange {
                    line_offset_str_iter: LineOffsetStrIterator {
                        current_iter: self.lines[start_row].1[start_offset..].into(),
                        base_collection: self,
                        peeked: None,
                        current_row: start_row,
                    },
                    upper_bound: range.end,
                }),
            )
        } else {
            Box::new("".chars())
        };
        iter_out
    }
}

pub struct CharsInMultiLineRange<'a> {
    line_offset_str_iter: LineOffsetStrIterator<'a>,
    upper_bound: IndexPosition,
}

impl<'a> Iterator for CharsInMultiLineRange<'a> {
    type Item = char;

    fn next(&mut self) -> Option<char> {
        if self.line_offset_str_iter.offset() < self.upper_bound {
            self.line_offset_str_iter.next()
        } else {
            None
        }
    }
}

#[derive(Clone)]
pub struct LineOffsetStrIterator<'a> {
    current_iter: BorrowedStringPCI<'a>,
    base_collection: &'a LineOffsetStrCollection<'a>,
    peeked: Option<Option<(usize, char)>>,
    current_row: usize,
}

// These should only be used in cases of inline, in which case the extra spaces will be by
// definition 0.
impl<'a> PeekableCharIndices for LineOffsetStrIterator<'a> {
    type Offset = IndexPosition;
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
                            self.current_iter =
                                self.base_collection.lines[self.current_row].1.into();
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
                        self.current_iter = self.base_collection.lines[self.current_row].1.into();
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

impl<'a> Iterator for LineOffsetStrIterator<'a> {
    type Item = char;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_and_index().map(|t| t.1)
    }
}
