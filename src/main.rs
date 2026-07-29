use common_mark_rust::markdown_to_html;
use std::env;

fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let text = std::fs::read_to_string(&args[1])?;
    println!("{}", markdown_to_html(&text));
    Ok(())
}
