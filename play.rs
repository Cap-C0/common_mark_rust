fn main() -> () {
    dbg!('ẞ'.to_lowercase());
    // let my_str = "xはばん";
    // let mut my_iter = my_str.char_indices().peekable();
    // dbg!(my_iter.peek());
    // while let Some(x) = my_iter.next() {
    //     dbg!(x);
    // }
    // let mut my_iter_2 = my_str[1..].char_indices().peekable();
    // dbg!(my_iter_2.peek());
    // while let Some(x) = my_iter_2.next() {
    //     dbg!(x);
    // }
}
struct UnicodeCaseFoldIter {
    chars: [char; 3],
    char_count: usize,
    current_offset: usize,
}

impl iter for UnicodeCaseFoldIter {
    type Item = char;

    fn next(&mut self) -> Option<Self::Item> {
        if current_offset > char_count {
            None
        } else {
            out = chars[current_offset];
            current_offset += 1;
            Some(out)
        }
    }
}
