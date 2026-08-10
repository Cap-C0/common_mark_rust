//# serde_json = "*"

use serde_json::Value;

fn main() -> () {
    let my_str = "abcdef";

    let mut my_it = my_str.chars().enumerate();
    while let Some(x) = my_it.next() {
        dbg!(x);
        dbg!(my_it.next());
    }
}
