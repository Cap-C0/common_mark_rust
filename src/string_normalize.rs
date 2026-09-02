use crate::peekable_char_indices::PeekableCharIndices;
use std::assert_matches;

#[derive(Clone)]
pub struct UnicodeCaseFoldIter {
    chars: [char; 3],
    char_count: usize,
    current_offset: usize,
}

impl Iterator for UnicodeCaseFoldIter {
    type Item = char;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_offset >= self.char_count {
            None
        } else {
            let out = self.chars[self.current_offset];
            self.current_offset += 1;
            Some(out)
        }
    }
}

pub trait UnicodeCaseFold {
    fn unicode_case_fold(&self) -> UnicodeCaseFoldIter;
}

include!(concat!(env!("OUT_DIR"), "/unicode_casefold.rs"));

pub fn normalize_label(char_iter: &impl PeekableCharIndices) -> String {
    let mut lab_out = String::new();
    let mut char_iter = char_iter.clone();
    //strip leading whitespace
    if char_iter.next_if_char_eq('[').is_none() {
        panic!()
    }
    char_iter.consume_while(|c| " \t".contains(c));
    let mut seen_space = false;
    let mut last_c = None;
    for c in char_iter {
        if " \n\t".contains(c) {
            seen_space = true
        } else {
            if seen_space {
                seen_space = false;
                lab_out.push(' ');
            }
            c.unicode_case_fold().for_each(|cl| lab_out.push(cl));
        }
        last_c = Some(c);
    }
    assert_matches!(last_c, Some(']'));
    lab_out.pop();
    lab_out
}
