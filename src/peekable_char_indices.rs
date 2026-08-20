use std::str::CharIndices;

#[derive(Debug, Clone)]
pub struct PeekableCharIndices<'a> {
    iter: CharIndices<'a>,
    peeked: Option<Option<(usize, char)>>,
}

impl<'a> Iterator for PeekableCharIndices<'a> {
    type Item = (usize, char);

    fn next(&mut self) -> Option<(usize, char)> {
        self.peeked.take().unwrap_or_else(|| self.iter.next())
    }
}

impl<'a> PeekableCharIndices<'a> {
    pub fn new(iter: CharIndices<'a>) -> Self {
        PeekableCharIndices {
            iter: iter,
            peeked: None,
        }
    }

    pub fn peek(&mut self) -> Option<(usize, char)> {
        if self.peeked.is_none() {
            self.peeked = Some(self.iter.next())
        }
        self.peeked.unwrap()
    }

    pub fn offset(&self) -> usize {
        self.peeked.map_or(self.iter.offset(), |op| {
            op.map_or(self.iter.offset(), |(i, _)| i)
        })
    }

    pub fn next_if(&mut self, func: impl Fn((usize, char)) -> bool) -> Option<(usize, char)> {
        self.next_if_helper(&func)
    }
    pub fn next_if_char_eq(&mut self, c: char) -> Option<(usize, char)> {
        self.next_if(|(_, peeked_c)| peeked_c == c)
    }

    /// iterates until it finds a value that does not match the func
    pub fn consume_while(&mut self, func: impl Fn((usize, char)) -> bool) {
        while self.next_if_helper(&func).is_some() {}
    }

    pub fn consume_while_char_eq(&mut self, c: char) {
        self.consume_while(|(_, peeked_c)| peeked_c == c);
    }

    fn next_if_helper(&mut self, func: &impl Fn((usize, char)) -> bool) -> Option<(usize, char)> {
        match self.peek() {
            Some(tup) => {
                if func(tup) {
                    return self.next();
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}
