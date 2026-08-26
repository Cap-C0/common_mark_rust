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

pub fn normalize_label(label: &str) -> String {
    let mut lab_out = String::new();
    dbg!(label);
    let mut char_iter = label[1..label.len() - 1].chars().peekable();
    //strip leading whitespace
    if char_iter.peek().is_none() {
        return String::new();
    }
    while let Some(c) = char_iter.peek()
        && c.is_whitespace()
    {
        char_iter.next();
    }
    if char_iter.peek().is_none() {
        return String::new();
    }
    let mut seen_space = false;
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
    }
    lab_out
}
