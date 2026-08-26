use std::str::CharIndices;

#[derive(Debug, Clone)]
pub struct PeekableCharIndices<'a> {
    pub iter: CharIndices<'a>,
    pub peeked: Option<Option<(usize, char)>>,
}

impl<'a> Iterator for PeekableCharIndices<'a> {
    type Item = char;

    fn next(&mut self) -> Option<char> {
        self.peeked
            .take()
            .unwrap_or_else(|| self.iter.next())
            .map(|t| t.1)
    }
}

// the index is not really used as it is returned from the next or peek calls
// so just return characters from the iterator
impl<'a> PeekableCharIndices<'a> {
    pub fn new(iter: CharIndices<'a>) -> Self {
        PeekableCharIndices {
            iter: iter,
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

    pub fn next_and_index(&mut self) -> Option<(usize, char)> {
        self.peeked.take().unwrap_or_else(|| self.iter.next())
    }

    pub fn peek_and_index(&mut self) -> Option<(usize, char)> {
        if self.peeked.is_none() {
            self.peeked = Some(self.iter.next())
        }
        self.peeked.unwrap()
    }

    pub fn peek(&mut self) -> Option<char> {
        if self.peeked.is_none() {
            self.peeked = Some(self.iter.next())
        }
        self.peeked.unwrap().map(|t| t.1)
    }

    pub fn offset(&self) -> usize {
        self.peeked.map_or(self.iter.offset(), |op| {
            op.map_or(self.iter.offset(), |t| t.0)
        })
    }

    pub fn is_empty(&mut self) -> bool {
        self.peek().is_none()
    }

    pub fn next_if(&mut self, func: impl Fn(char) -> bool) -> Option<char> {
        self.next_if_helper(&func)
    }
    pub fn next_if_char_eq(&mut self, c: char) -> Option<char> {
        self.next_if(|peeked_c| peeked_c == c)
    }

    /// iterates until it finds a value that does not match the func
    pub fn consume_while(&mut self, func: impl Fn(char) -> bool) {
        while self.next_if_helper(&func).is_some() {}
    }

    pub fn consume_while_char_eq(&mut self, c: char) {
        self.consume_while(|peeked_c| peeked_c == c);
    }

    fn next_if_helper(&mut self, func: &impl Fn(char) -> bool) -> Option<char> {
        match self.peek() {
            Some(tup) => {
                if func(tup) {
                    self.next()
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}
#[cfg(test)]
mod pci_tests {
    use super::*;

    #[test]
    fn test_pci_1() {
        let mut my_iter = PeekableCharIndices::new("abcde".char_indices());

        assert_eq!(my_iter.offset(), 0);
        assert_eq!(my_iter.peek(), Some('a'));
        assert_eq!(my_iter.offset(), 0);
        assert_eq!(my_iter.next(), Some('a'));
        assert_eq!(my_iter.offset(), 1);
        assert_eq!(my_iter.peek(), Some('b'));
        assert_eq!(my_iter.peek(), Some('b'));
    }
}
