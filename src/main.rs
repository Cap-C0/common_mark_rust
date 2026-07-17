pub mod lib;

use crate::lib::markdown_to_html;
use serde_json::Value;
use std::env;
use std::fmt::Result;
use std::fs::File;
use std::io::{BufRead, BufReader};

fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let text = std::fs::read_to_string(&args[1])?;
    json_to_rust_test_case(text);
    Ok(())
}

fn json_to_rust_test_case(json_string: String) {
    let cases: Vec<Value> = serde_json::from_str(&json_string).unwrap();
    println!("pub mod lib");
    println!("use crate::lib::markdown_to_html;");
    println!();
    println!("#[cfg(test)]");
    println!("mod tests {{");
    for case in cases {
        println!("#[test]");
        println!(
            "fn {}_{}(){{",
            case["section"]
                .to_string()
                .replace(' ', "_")
                .replace('\"', "")
                .to_lowercase(),
            case["example"]
        );
        println!("  let markdown = {};", case["markdown"]);
        println!("  let html = {};", case["html"]);
        println!("  assert_eq!(markdown_to_html(markdown), html);");
        println!("}}");
        println!();
    }
    println!("}}");
}

#[test]
fn test_shit() {
    let x = "hello";
    let y = "hello";
    assert_eq!(x, y);
}
