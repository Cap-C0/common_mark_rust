//# serde_json = "*"

use serde_json::Value;

fn main() -> () {
    let cases: Vec<Value> =
        serde_json::from_str(&std::fs::read_to_string("spec.json").unwrap()).unwrap();

    dbg!(&cases[89]);
    // dbg!("Foo \\\\\\n".replace("\\n", "\n").replace("\\\\", "\\"));
    // dbg!(
    //     "Foo \\\" in quotes! \\\" \\\\\\n"
    //         .replace("\\n", "\n")
    //         .replace("\\\"", "\"")
    //         .replace("\\\\", "\\")
    // );
    dbg!(
        cases[89]["markdown"]
            .to_string()
            .replace("\\n", "\n")
            .replace("\\\"", "\"")
            .replace("\\\\", "\\")
    );
}
