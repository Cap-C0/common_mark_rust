fn main() -> () {
    let my_str = "abc\n\ndef\nghi";
    let split: Vec<Vec<char>> = dbg!(my_str.split('\n').map(|x| x.chars().collect()).collect());
}
