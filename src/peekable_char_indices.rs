use std::str::CharIndices;

pub trait PeekableCharIndices<Offset>: Iterator<Item = char> + Clone {
    // fn collect_until_offset(&mut self, last_offset: Offset) -> String;

    fn next_and_index(&mut self) -> Option<(Offset, char)>;

    fn peek_and_index(&mut self) -> Option<(Offset, char)>;

    fn peek(&mut self) -> Option<char> {
        self.peek_and_index().map(|(_, c)| c)
    }

    fn offset(&self) -> Offset;

    // fn is_empty(&mut self) -> bool;

    fn is_empty(&mut self) -> bool {
        self.peek().is_none()
    }

    fn next_if(&mut self, func: impl Fn(char) -> bool) -> Option<char> {
        next_if_helper(self, &func)
    }
    fn next_if_char_eq(&mut self, c: char) -> Option<char> {
        self.next_if(|peeked_c| peeked_c == c)
    }

    fn consume_while(&mut self, func: impl Fn(char) -> bool) {
        while next_if_helper(self, &func).is_some() {}
    }

    fn consume_while_char_eq(&mut self, c: char) {
        self.consume_while(|peeked_c| peeked_c == c);
    }
}

fn next_if_helper<S, T: PeekableCharIndices<S> + ?Sized>(
    iter: &mut T,
    func: &impl Fn(char) -> bool,
) -> Option<char> {
    match iter.peek() {
        Some(tup) => {
            if func(tup) {
                iter.next()
            } else {
                None
            }
        }
        _ => None,
    }
}

#[derive(Debug, Clone)]
pub struct BorrowedStringPCI<'a> {
    pub iter: CharIndices<'a>,
    pub peeked: Option<Option<(usize, char)>>,
}

impl<'a> Iterator for BorrowedStringPCI<'a> {
    type Item = char;

    fn next(&mut self) -> Option<char> {
        self.peeked
            .take()
            .unwrap_or_else(|| self.iter.next())
            .map(|t| t.1)
    }
}

impl<'a> From<&'a str> for BorrowedStringPCI<'a> {
    fn from(s: &'a str) -> Self {
        Self {
            iter: s.char_indices(),
            peeked: None,
        }
    }
}

impl<'a> BorrowedStringPCI<'a> {
    pub fn new(string_in: &'a str) -> Self {
        BorrowedStringPCI {
            iter: string_in.char_indices(),
            peeked: None,
        }
    }
    pub fn collect_until_offset(&mut self, last_offset: usize) -> String {
        let mut out = String::new();
        while self.offset() < last_offset {
            out.push(self.next().unwrap());
        }
        out
    }
}
// the index is not really used as it is returned from the next or peek calls
// so just return characters from the iterator
impl<'a> PeekableCharIndices<usize> for BorrowedStringPCI<'a> {
    fn next_and_index(&mut self) -> Option<(usize, char)> {
        self.peeked.take().unwrap_or_else(|| self.iter.next())
    }

    fn peek_and_index(&mut self) -> Option<(usize, char)> {
        if self.peeked.is_none() {
            self.peeked = Some(self.iter.next())
        }
        self.peeked.unwrap()
    }

    fn offset(&self) -> usize {
        self.peeked.map_or(self.iter.offset(), |op| {
            op.map_or(self.iter.offset(), |t| t.0)
        })
    }
}
#[cfg(test)]
mod pci_tests {
    use super::*;

    #[test]
    fn test_pci_1() {
        let mut my_iter = BorrowedStringPCI::new("abcde");

        assert_eq!(my_iter.offset(), 0);
        assert_eq!(my_iter.peek(), Some('a'));
        assert_eq!(my_iter.offset(), 0);
        assert_eq!(my_iter.next(), Some('a'));
        assert_eq!(my_iter.offset(), 1);
        assert_eq!(my_iter.peek(), Some('b'));
        assert_eq!(my_iter.peek(), Some('b'));
    }
}
