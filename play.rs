use std::ops::Range;
fn main() -> () {
    let my_range: Range<usize> = 3..5;
    (&"abcdefgh"[my_range]);
}
