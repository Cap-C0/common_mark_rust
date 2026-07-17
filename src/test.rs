use serde::Deserialize;

#[derive(Deserialize)]
struct Case {
    markdown: String,
    html: String,
    example: u16,
    section: String,
}
