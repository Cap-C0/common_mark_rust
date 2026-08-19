fn main() -> () {
    let my_str = "xはばん";
    let mut my_iter = my_str.char_indices().peekable();
    dbg!(my_iter.peek());
    while let Some(x) = my_iter.next() {
        dbg!(x);
    }
    let mut my_iter_2 = my_str[1..].char_indices().peekable();
    dbg!(my_iter_2.peek());
    while let Some(x) = my_iter_2.next() {
        dbg!(x);
    }
}
