use crate::ast_types::*;
use crate::block_structure::*;
pub mod ast_types;
pub mod block_structure;
pub mod chars;
pub mod inline;
pub mod parsers;
pub mod peekable_char_indices;
pub mod string_normalize;

pub fn markdown_to_html(markdown: &str) -> String {
    // first split into lines

    // Ok, there might be clever ways to not need to use lines as Vec<char>, but I can do that later
    // TODO figure out how not split lines
    // use raw stringes all the way!

    let (document, lrd_table) = create_block_structure(markdown);

    // dbg!(&lrd_table);
    // document.parse_inlines(&lrd_table);
    //
    // dbg!(&document);

    document.to_html(&lrd_table)
}
