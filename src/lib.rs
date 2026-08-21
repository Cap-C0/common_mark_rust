use crate::Block::*;
use crate::HTMLEndCondition::*;
use crate::ListType::*;
use crate::ast_types::*;
use crate::block_structure::*;
use crate::inline::*;

use std::collections::HashMap;
use std::mem;
pub mod ast_types;
pub mod block_structure;
pub mod chars;
pub mod inline;
pub mod peekable_char_indices;

pub fn markdown_to_html(markdown: &str) -> String {
    // first split into lines

    // Ok, there might be clever ways to not need to use lines as Vec<char>, but I can do that later
    // TODO figure out how not split lines
    // use raw stringes all the way!
    let mut lines: Vec<Vec<char>> = markdown.split('\n').map(|x| x.chars().collect()).collect();
    if lines.last().expect("lines is non empty").len() == 0 {
        lines.pop(); // an erroneous extra line is not needed
    }

    // 1st create block structure of document
    let mut document = Document(vec![]);
    let mut lrd_table: HashMap<Vec<char>, (Vec<char>, Vec<char>)> = HashMap::new();
    let mut blank_line_depth: Option<usize> = None;

    for (line_num, line) in lines.iter().enumerate() {
        println!("reading line: {}", line_num);
        // first check continuation conditions
        let mut char_offset: usize = 0;
        // if there are no tabs in the line, then effective_column_number should stay equal to
        // char_offset
        let mut effective_column_number: usize = 0;
        // similarly, no tabs should imply that additional_possible_spaces is always 0.
        let mut additional_possible_spaces: usize = 0;
        let mut open_block_depth: usize = 0;
        check_continuation_conditions(
            &document,
            &line,
            &mut char_offset,
            &mut effective_column_number,
            &mut additional_possible_spaces,
            &mut open_block_depth,
        );

        // now we look for the start of any new structure
        create_new_block_starts(
            &mut document,
            &line,
            &mut char_offset,
            &mut effective_column_number,
            &mut additional_possible_spaces,
            &mut open_block_depth,
            &mut blank_line_depth,
            &mut lrd_table,
        );
    }

    let mut open_par_exists = match document.get_last_block() {
        Paragraph(_, open) => *open,
        _ => false,
    };
    close_paragraph(
        &mut document,
        &mut false,
        &mut open_par_exists,
        &mut lrd_table,
    );

    document.parse_inlines(&lrd_table);

    dbg!(&document);

    document.to_html()
}
