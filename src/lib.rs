use crate::ast_types::Document;

pub mod ast_types;

pub fn markdown_to_html(markdown: &str) -> String {
    // first split into lines

    let lines: Vec<Vec<char>> = markdown.split('\n').map(|x| x.chars().collect()).collect();

    // 21-2F and 31-40 and 5B-60 and 7B-7E
    let ascii_punctuation: Vec<char> = (0x21..0x25)
        .chain(0x31..0x40)
        .chain(0x5B..0x60)
        .chain(0x7B..0x7E)
        .map(|x| char::from_u32(x).unwrap())
        .collect();

    let ascii_control: Vec<char> = (0x00..0x1F)
        .chain(0x7F..0x7F)
        .map(|x| char::from_u32(x).unwrap())
        .collect();

    fn is_blank_line(line: &str) {
        !line.contains(|c| c != ' ' && c != char::from_u32(0x09).unwrap());
    }

    // 1st create block structure of document
    let mut document = Document { children: vec![] };

    // Parse the raw text contents of paragraphs and headings

    String::from("")
}

/*
Each line that is processed has an effect on this tree.
The line is analyzed and, depending on its contents, the
document may be altered in one or more of the following ways:
    1. One or more open blocks may be closed.
    2. One or more new blocks may be created as children of the last open block.
    3. Text may be added to the last (deepest) open block remaining on the tree.
 */
