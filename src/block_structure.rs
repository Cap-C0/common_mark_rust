use std::collections::HashMap;
use std::mem;

use crate::Block::*;
use crate::HTMLEndCondition::*;
use crate::ListType::*;
use crate::ast_types::*;
use crate::inline::*;
use crate::parsers::*;
use crate::peekable_char_indices::PeekableCharIndices;

type LRDTable = HashMap<String, (String, String)>;

#[derive(Debug, Clone)]
struct ParsingState<'a> {
    // current_line_number: usize,
    document: Block,
    lrd_table: LRDTable,
    line_state: LineState<'a>,
    open_block_depth: usize,
    open_par_above: bool,
    open_par_exists: bool,
    // at what "level" was a blank line seen, important
    // for telling what which list block gets loosened in nested lists.
    blank_line_depth: Option<usize>,
    line_number: usize,
}

impl<'a> ParsingState<'a> {
    fn set_open_par_above(&mut self) {
        self.open_par_above = matches!(
            self.document.get_block(self.open_block_depth),
            Paragraph(_, true)
        );
    }

    fn set_open_par_exists(&mut self) {
        self.open_par_exists = matches!(self.document.get_last_block(), Paragraph(_, true));
    }

    fn set_open_pars(&mut self) {
        self.set_open_par_above();
        self.set_open_par_exists();
    }

    pub fn get_open_block(&mut self) -> &mut Block {
        self.document.get_block(self.open_block_depth)
    }

    fn set_new_line_state(&mut self) {
        self.open_block_depth = 0;
        self.line_number += 1;
        self.open_par_above = false;
        self.open_par_exists = false;
    }
}

#[derive(Debug, Clone)]
struct LineState<'a> {
    char_iter: PeekableCharIndices<'a>,
    effective_column_number: usize,
    space_from_last_structure: usize,
}

impl<'a> Iterator for LineState<'a> {
    type Item = char;

    fn next(&mut self) -> Option<char> {
        let out = self.char_iter.next()?;
        if out == '\t' {
            self.space_from_last_structure += 4 - (self.effective_column_number % 4);
            self.effective_column_number += 4 - (self.effective_column_number % 4);
        } else if out == ' ' {
            self.space_from_last_structure += 1;
            self.effective_column_number += 1;
        } else {
            self.space_from_last_structure = 0;
            self.effective_column_number += 1;
        }
        // dbg!(&self);
        Some(out)
    }
}

impl<'a> LineState<'a> {
    pub fn new(line_in: &'a str) -> Self {
        LineState {
            char_iter: PeekableCharIndices::new(line_in.char_indices()),
            effective_column_number: 0,
            space_from_last_structure: 0,
        }
    }

    pub fn next_and_index(&mut self) -> Option<(usize, char)> {
        self.char_iter.next_and_index()
    }

    pub fn peek(&mut self) -> Option<char> {
        self.char_iter.peek()
    }

    pub fn offset(&self) -> usize {
        self.char_iter.offset()
    }

    pub fn is_empty(&mut self) -> bool {
        self.char_iter.is_empty()
    }

    pub fn next_if(&mut self, func: impl Fn(char) -> bool) -> Option<char> {
        self.next_if_helper(&func)
    }
    pub fn next_if_char_eq(&mut self, c: char) -> Option<char> {
        self.next_if(|peeked_c| peeked_c == c)
    }

    /// iterates until it finds a value that does not match the func
    pub fn consume_while(&mut self, func: impl Fn(char) -> bool) {
        while self.next_if_helper(&func).is_some() {}
    }

    pub fn consume_while_char_eq(&mut self, c: char) {
        self.consume_while(|peeked_c| peeked_c == c);
    }

    fn next_if_helper(&mut self, func: &impl Fn(char) -> bool) -> Option<char> {
        match self.peek() {
            Some(tup) => {
                if func(tup) {
                    return self.next();
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn consume_indent(&mut self) {
        self.consume_while(|c| " \t".contains(c));
    }

    fn consume_one_space(&mut self) -> bool {
        self.next_if(|c| " \t".contains(c)).is_some()
    }

    fn consume_indent_until_sfls_ge(&mut self, n: usize) {
        // dbg!("called consume_indent");
        while (self.space_from_last_structure) < n && self.next_if(|c| " \t".contains(c)).is_some()
        {
        }
    }

    fn is_blank_line(&self) -> bool {
        let mut self_clone = self.clone();
        self_clone.consume_indent();
        self_clone.is_empty()
    }
}

#[cfg(test)]
mod test_ls {
    use super::*;

    #[test]
    fn test_ls_1() {
        let mut my_iter = LineState::new(" \ta");

        assert_eq!(my_iter.offset(), 0);
        assert_eq!(my_iter.peek(), Some(' '));
        assert_eq!(my_iter.offset(), 0);
        my_iter.consume_indent_until_sfls_ge(2);
        assert_eq!(my_iter.offset(), 2);
        assert_eq!(my_iter.effective_column_number, 4);
        assert_eq!(my_iter.space_from_last_structure, 4);
        assert_eq!(my_iter.peek(), Some('a'));
        assert_eq!(my_iter.next(), Some('a'));
        assert!(my_iter.is_blank_line());
        assert!(my_iter.is_empty());
    }
}

pub fn create_block_structure(markdown: &str) -> (Block, LRDTable) {
    let mut lines_list: Vec<&str> = markdown.split('\n').collect();
    lines_list.pop();
    let mut lines = lines_list.iter().peekable();

    let lrd_table = HashMap::new();
    let document = Document(vec![]);

    let Some(line_0) = lines.next() else {
        return (document, lrd_table);
    };
    dbg!(line_0);
    let mut parsing_state = ParsingState {
        document: document,
        lrd_table: lrd_table,
        line_state: LineState::new(line_0),
        open_block_depth: 0,
        open_par_above: false,
        open_par_exists: false,
        blank_line_depth: None,
        line_number: 0,
    };

    for line in lines {
        dbg!(&parsing_state);
        parsing_state.set_new_line_state();
        check_continuation_conditions(&mut parsing_state);
        parsing_state.set_open_par_exists();
        create_new_block_starts(&mut parsing_state);
        // dbg!(line);
        parsing_state.line_state = LineState::new(line)
    }
    dbg!(&parsing_state);
    parsing_state.set_new_line_state();
    check_continuation_conditions(&mut parsing_state);
    parsing_state.set_open_par_exists();
    create_new_block_starts(&mut parsing_state);
    close_paragraph(&mut parsing_state);
    (parsing_state.document, parsing_state.lrd_table)
}

fn check_continuation_conditions(parsing_state: &mut ParsingState) {
    let mut current_block = &parsing_state.document;
    let line_state = &mut parsing_state.line_state;
    // println!("called ccc!");
    'outer: loop {
        match current_block {
            Document(blocks) => match blocks.last() {
                None => break,
                Some(b) => {
                    // println!("has children!");
                    current_block = b;
                }
            },
            BlockQuote(blocks, is_open) => {
                if !*is_open {
                    break 'outer;
                }
                line_state.consume_indent_until_sfls_ge(4);
                if line_state.space_from_last_structure <= 3 && block_quote_encountered(line_state)
                {
                    parsing_state.open_block_depth += 1;
                    match blocks.last() {
                        None => break 'outer,
                        Some(b) => {
                            current_block = b;
                            continue 'outer;
                        }
                    }
                } else {
                    break 'outer;
                }
            }
            List(list_items, _, _, _) => {
                parsing_state.open_block_depth += 1;
                // lists are guaranteed to have at least one item
                current_block = list_items.last().unwrap();
            }
            ListItem(blocks, continuable, indent_amount) => {
                if !continuable {
                    break 'outer;
                }
                let old_line_state = line_state.clone();
                line_state.consume_indent_until_sfls_ge(*indent_amount);
                //TODO: make this work with blank lines too?
                if line_state.space_from_last_structure >= *indent_amount || line_state.is_empty() {
                    line_state.space_from_last_structure -=
                        std::cmp::min(*indent_amount, line_state.space_from_last_structure);
                    parsing_state.open_block_depth += 1;
                    match blocks.last() {
                        None => break 'outer,
                        Some(b) => {
                            current_block = b;
                        }
                    }
                } else {
                    *line_state = old_line_state;
                    break 'outer;
                }
            }
            Heading(_, _) => break,
            Paragraph(_, is_open) => {
                if *is_open {
                    parsing_state.open_block_depth += 1;
                    parsing_state.open_par_above = true;
                }
                break;
            }
            ThematicBreak => break,
            IndentedCodeBlock(_, _) => {
                line_state.consume_indent_until_sfls_ge(4);
                if line_state.space_from_last_structure >= 4 || line_state.is_empty() {
                    parsing_state.open_block_depth += 1;
                }
                break;
            }
            FencedCodeBlock(_, is_open, _, _, _, _) => {
                if *is_open {
                    parsing_state.open_block_depth += 1;
                }
                break;
            }
            HTMLBlock(_, is_open, _) => {
                if *is_open {
                    parsing_state.open_block_depth += 1;
                }
                break;
            }
        }
    }
}

// #[cfg(test)]
// mod cc_tests {
//     use super::*;
//     use pretty_assertions::assert_eq;
//
//     fn test_cc(ast: &mut Block, line: &str, expected_block: &mut Block, exp_offset: usize) {
//         let line: Vec<char> = line.chars().collect();
//         // println!("{:?}", line);
//         let mut char_offset: usize = 0;
//         let mut effective_column_number: usize = 0;
//         let mut additional_possible_spaces: usize = 0;
//         let mut open_block_depth: usize = 0;
//         check_continuation_conditions(
//             &ast,
//             &line,
//             &mut char_offset,
//             &mut effective_column_number,
//             &mut additional_possible_spaces,
//             &mut open_block_depth,
//         );
//         assert_eq!(
//             (ast.get_block(open_block_depth), char_offset),
//             (expected_block, exp_offset)
//         );
//     }
//
//     #[test]
//     fn test_cc_1() {
//         let mut test_tree = Document(vec![]);
//         test_cc(&mut test_tree, "> abc", &mut Document(vec![]), 0);
//     }
//
//     #[test]
//     fn test_cc_qb_1() {
//         let mut test_tree = Document(vec![BlockQuote(vec![ThematicBreak], true)]);
//         let mut exp_tree = BlockQuote(vec![ThematicBreak], true);
//         test_cc(&mut test_tree, ">hello", &mut exp_tree, 1);
//     }
//
//     #[test]
//     fn test_cc_qb_2() {
//         let mut test_tree = Document(vec![BlockQuote(vec![ThematicBreak], true)]);
//         let mut exp_tree = BlockQuote(vec![ThematicBreak], true);
//         test_cc(&mut test_tree, "> hello", &mut exp_tree, 2);
//     }
//
//     #[test]
//     fn test_cc_qb_3() {
//         let mut test_tree = Document(vec![BlockQuote(vec![ThematicBreak], true)]);
//         let mut exp_tree = BlockQuote(vec![ThematicBreak], true);
//         test_cc(&mut test_tree, "   > hello", &mut exp_tree, 5);
//     }
//
//     #[test]
//     fn test_cc_qb_4() {
//         let mut test_tree = Document(vec![BlockQuote(vec![ThematicBreak], true)]);
//         let mut exp_tree = BlockQuote(vec![ThematicBreak], true);
//         test_cc(&mut test_tree, " > hello", &mut exp_tree, 3);
//     }
//
//     #[test]
//     fn test_cc_qb_5() {
//         let mut test_tree = Document(vec![BlockQuote(vec![ThematicBreak], true)]);
//         let mut exp_tree = BlockQuote(vec![ThematicBreak], true);
//         test_cc(&mut test_tree, ">  hello", &mut exp_tree, 2);
//     }
//
//     #[test]
//     fn test_cc_qb_6() {
//         let mut test_tree = Document(vec![BlockQuote(vec![ThematicBreak], false)]);
//         let mut exp_tree = test_tree.clone();
//         test_cc(&mut test_tree, ">  hello", &mut exp_tree, 0);
//     }
//
//     #[test]
//     fn test_cc_list_1() {
//         let mut test_tree = Document(vec![List(
//             vec![ListItem(vec![], true, 2)],
//             true,
//             UnorderedList('*'),
//             false,
//         )]);
//         let mut exp_tree = ListItem(vec![], true, 2);
//         test_cc(&mut test_tree, "  >  hello", &mut exp_tree, 2);
//     }
//
//     #[test]
//     fn test_cc_list_2() {
//         let mut test_tree = Document(vec![List(
//             vec![ListItem(vec![], true, 2)],
//             true,
//             UnorderedList('*'),
//             false,
//         )]);
//         let mut exp_tree = List(
//             vec![ListItem(vec![], true, 2)],
//             true,
//             UnorderedList('*'),
//             false,
//         );
//         test_cc(&mut test_tree, " >  hello", &mut exp_tree, 0);
//     }
//
//     #[test]
//     fn test_cc_both_3() {
//         let mut test_tree = Document(vec![List(
//             vec![ListItem(vec![BlockQuote(vec![], true)], true, 2)],
//             true,
//             UnorderedList('*'),
//             false,
//         )]);
//         let mut exp_tree = BlockQuote(vec![], true);
//         test_cc(&mut test_tree, "  > hello", &mut exp_tree, 4);
//     }
// }

// fn consume_indent(parsing_state: &mut ParsingState, line_iter: &mut PeekableCharIndices) {
//     while let Some((_, c)) = line_iter.next_if(|(_, c)| " \t".contains(c)) {
//         if c == '\t' {
//             parsing_state.space_from_last_structure +=
//                 4 - (parsing_state.effective_column_number % 4);
//             parsing_state.effective_column_number +=
//                 4 - (parsing_state.effective_column_number % 4);
//         } else {
//             parsing_state.space_from_last_structure += 1;
//             parsing_state.effective_column_number += 1;
//         }
//     }
// }
//
// fn consume_indent_until_sfls_ge(
//     parsing_state: &mut ParsingState,
//     line_iter: &mut PeekableCharIndices,
//     n: usize,
// ) {
//     if parsing_state.space_from_last_structure >= n {
//         return;
//     }
//     while let Some((_, c)) = line_iter.next_if(|(_, c)| " \t".contains(c)) {
//         if c == '\t' {
//             parsing_state.space_from_last_structure +=
//                 4 - (parsing_state.effective_column_number % 4);
//             parsing_state.effective_column_number +=
//                 4 - (parsing_state.effective_column_number % 4);
//         } else {
//             parsing_state.space_from_last_structure += 1;
//             parsing_state.effective_column_number += 1;
//         }
//         if parsing_state.space_from_last_structure >= n {
//             parsing_state.additional_possible_spaces = parsing_state.space_from_last_structure - n;
//             break;
//         }
//     }
// }
//
/// char_offset: how far you have to literally offset the line to get to the space character
/// effective_offset: on what "column" this space is aligning
/// returns: (new_char_offset, new_effective_column_number)
/// This is hard to think about, no wonder they start with it in the spec, cause it would have been good to
/// design around tabs in the first place.
// fn consume_effective_indent(
//     line: &Vec<char>,
//     mut char_offset: usize,
//     mut effective_column_number: usize,
// ) -> (usize, usize) {
//     //in contexts where spaces help to define block structure, tabs behave as if they were replaced by spaces with a tab stop of 4 characters.
//     while line.len() > char_offset && " \t".contains(line[char_offset]) {
//         if line[char_offset] == '\t' {
//             effective_column_number += 4 - (effective_column_number % 4);
//         } else {
//             effective_column_number += 1;
//         }
//         char_offset += 1;
//     }
//     return (char_offset, effective_column_number);
// }

fn thematic_break_encountered(char_iter: &mut LineState) -> bool {
    let Some(br_sym) = char_iter.next_if(|c| "-*_".contains(c)) else {
        return false;
    };
    let mut c_count = 1;
    while let Some(c) = char_iter.next_if(|c| " \t".contains(c) || c == br_sym) {
        if c == br_sym {
            c_count += 1
        }
    }
    if char_iter.is_empty() && c_count >= 3 {
        return true;
    }
    false
}

fn list_item_encountered(line_state: &mut LineState) -> Option<(ListType, Block)> {
    let space_before = line_state.space_from_last_structure;
    let c_0 = line_state.next()?;
    if c_0.is_numeric() {
        let mut digit_count = 1;
        let mut count = c_0.to_digit(10).unwrap();
        while let Some(c) = line_state.next_if(|c| c.is_numeric())
            && digit_count <= 9
        {
            count *= 10;
            count += c.to_digit(10).unwrap();
            digit_count += 1;
        }
        if let Some(c) = line_state.next_if(|c| ".)".contains(c))
            && digit_count <= 9
        {
            let post_li_iter = line_state.clone();

            line_state.consume_indent_until_sfls_ge(5);

            if line_state.is_empty() {
                //dont need to update parsing state related to this in this situation. because
                //there is no more line.
                return Some((
                    OrderedList(c, count),
                    ListItem(vec![], true, space_before + digit_count + 2),
                ));
            }

            if 1 <= line_state.space_from_last_structure
                && line_state.space_from_last_structure <= 4
            {
                let space_after = line_state.space_from_last_structure;
                line_state.space_from_last_structure = 0;
                return Some((
                    OrderedList(c, count),
                    ListItem(vec![], true, space_before + digit_count + 1 + space_after),
                ));
            } else if line_state.space_from_last_structure > 4 {
                *line_state = post_li_iter;
                assert!(line_state.consume_one_space());
                line_state.space_from_last_structure -= 1;
                return Some((
                    OrderedList(c, count),
                    ListItem(vec![], true, space_before + digit_count + 2),
                ));
            }
        }
        return None;
    }

    if "-*+".contains(c_0) {
        let post_li_iter = line_state.clone();

        line_state.consume_indent_until_sfls_ge(5);
        if line_state.is_empty() {
            //dont need to update parsing state related to this in this situation. because
            //there is no more line.
            return Some((UnorderedList(c_0), ListItem(vec![], true, space_before + 2)));
        }

        if 1 <= line_state.space_from_last_structure && line_state.space_from_last_structure <= 4 {
            let space_after = line_state.space_from_last_structure;
            line_state.space_from_last_structure = 0;
            return Some((
                UnorderedList(c_0),
                ListItem(vec![], true, space_before + 1 + space_after),
            ));
        } else if line_state.space_from_last_structure > 4 {
            *line_state = post_li_iter;
            assert!(line_state.consume_one_space());
            line_state.space_from_last_structure -= 1;
            return Some((UnorderedList(c_0), ListItem(vec![], true, space_before + 2)));
        }
    }
    None
}

/// returns hypothetically, if you "ate" and matched this, where the offsets would end up
/// returns Opt(listtype, listitem, new_char_offset, new_eff_column_number,
/// new additional_possible_spaces)
/// bounds checking is expected by caller
/// preceding spaces should be dealt with by caller
/// char_offset should be set on the first non-space char
// fn list_item_encountered(
//     line: &Vec<char>,
//     char_offset: usize,
//     eff_column_number: usize,
//     psc: usize,
// ) -> Option<(ListType, Block, usize, usize, usize)> {
//     let c = line[char_offset];
//     if c.is_numeric() {
//         let mut number_builder = String::from(line[char_offset]);
//         for j in 1..10 {
//             if line.len() <= char_offset + j {
//                 return None;
//             }
//             if line[char_offset + j].is_numeric() {
//                 (&mut number_builder).push(line[char_offset + j]);
//                 continue;
//             }
//             if ".)".contains(line[char_offset + j]) {
//                 let (post_space_char_offset, post_space_eff_col_number) =
//                     consume_indent(line, char_offset + j + 1, eff_column_number + j + 1);
//
//                 if line.len() <= post_space_char_offset {
//                     // ie, the rest of the line is blank
//                     return Some((
//                         OrderedList(line[char_offset + j], number_builder.parse().unwrap()),
//                         ListItem(vec![], true, psc + j + 2),
//                         post_space_char_offset,
//                         post_space_eff_col_number,
//                         0,
//                     ));
//                 }
//                 let following_space_count = post_space_eff_col_number - eff_column_number - j - 1;
//                 if 1 <= following_space_count && following_space_count <= 4 {
//                     return Some((
//                         OrderedList(line[char_offset + j], number_builder.parse().unwrap()),
//                         ListItem(vec![], true, psc + j + 1 + following_space_count),
//                         post_space_char_offset,
//                         post_space_eff_col_number,
//                         0, // a tab character immediately following the list delimiter will be
//                            // entirely consumed in this manner
//                     ));
//                 }
//                 if following_space_count > 4 {
//                     let (new_eff_col_number, possible_spaces) = if line[char_offset + j + 1] == '\t'
//                     {
//                         let tmp = eff_column_number + 1;
//                         let dist_to_new_stop = 4 - (tmp % 4);
//                         (tmp + dist_to_new_stop, dist_to_new_stop - 1)
//                     } else {
//                         (eff_column_number + j + 1, 0)
//                     };
//                     return Some((
//                         OrderedList(line[char_offset + j], number_builder.parse().unwrap()),
//                         ListItem(vec![], true, psc + j + 2),
//                         char_offset + j + 2,
//                         new_eff_col_number,
//                         possible_spaces,
//                     ));
//                 }
//             }
//             return None;
//         }
//     }
//
//     if "-+*".contains(line[char_offset]) {
//         let (post_space_char_offset, post_space_eff_col_number) =
//             consume_indent(line, char_offset + 1, eff_column_number + 1);
//
//         if line.len() <= post_space_char_offset {
//             // ie, the rest of the line is blank
//             return Some((
//                 UnorderedList(line[char_offset]),
//                 ListItem(vec![], true, psc + 2),
//                 post_space_char_offset,
//                 post_space_eff_col_number,
//                 0,
//             ));
//         }
//         let following_space_count = post_space_eff_col_number - eff_column_number - 1;
//         if 1 <= following_space_count && following_space_count <= 4 {
//             return Some((
//                 UnorderedList(line[char_offset]),
//                 ListItem(vec![], true, psc + 1 + following_space_count),
//                 post_space_char_offset,
//                 post_space_eff_col_number,
//                 0, // a tab character immediately following the list delimiter will be
//                    // entirely consumed in this manner
//             ));
//         }
//         if following_space_count > 4 {
//             let (new_eff_col_number, possible_spaces) = if line[char_offset + 1] == '\t' {
//                 let tmp = eff_column_number + 1;
//                 let dist_to_new_stop = 4 - (tmp % 4);
//                 (tmp + dist_to_new_stop, dist_to_new_stop - 1)
//             } else {
//                 (eff_column_number + 2, 0)
//             };
//             return Some((
//                 UnorderedList(line[char_offset]),
//                 ListItem(vec![], true, psc + 2),
//                 char_offset + 2,
//                 new_eff_col_number,
//                 possible_spaces,
//             ));
//         }
//     }
//     return None;
// }

fn block_quote_encountered(line_state: &mut LineState) -> bool {
    if line_state.next_if_char_eq('>').is_none() {
        return false;
    }
    if line_state.consume_one_space() {
        line_state.space_from_last_structure -= 1;
    }
    return true;
}

/// returns hypothetically, if you "ate" and matched this, where the offsets would end up
/// returns Opt(new_char_offset, new_eff_column_number, additional_possible_spaces)
/// bounds checking is expected by caller
/// preceding spaces should be dealt with by caller ie, line[char_offset] should be not space
/// char_offset should be set on the first non-space char
// fn block_quote_encountered(
//     line: &Vec<char>,
//     char_offset_after_space: usize,
//     eff_column_number: usize,
// ) -> Option<(usize, usize, usize)> {
//     if line[char_offset_after_space] == '>' {
//         if line.len() <= char_offset_after_space + 1 {
//             return Some((char_offset_after_space + 1, eff_column_number + 1, 0));
//         }
//         if line[char_offset_after_space + 1] == ' ' {
//             return Some((char_offset_after_space + 2, eff_column_number + 2, 0));
//         } else if line[char_offset_after_space + 1] == '\t' {
//             let new_eff_col = eff_column_number + 1; // since there is the > being consumed
//             let dist_to_new_stop = 4 - (new_eff_col % 4);
//             return Some((
//                 char_offset_after_space + 2,
//                 new_eff_col + dist_to_new_stop,
//                 dist_to_new_stop - 1,
//             ));
//             // dist_to_new_stop - 1 cause the block quote "eats" one of the spaces.
//         } else {
//             return Some((char_offset_after_space + 1, eff_column_number + 1, 0));
//         }
//     }
//     None
// }

fn fenced_code_block_encountered(line_state: &mut LineState) -> Option<Block> {
    let pre_space_count = line_state.space_from_last_structure;
    let start_offset = line_state.offset();
    let t = line_state.next_if(|c| "`~".contains(c))?;

    line_state.consume_while_char_eq(t);
    let tick_count = line_state.offset() - start_offset;
    if tick_count < 3 {
        return None;
    }
    line_state.consume_indent();
    let mut info_str = String::new();
    while let Some(c) = line_state.next_if(|c| !" \t".contains(c)) {
        info_str.push(c);
        if t == '`' && c == '`' {
            return None;
        }
    }
    if t == '`' {
        while let Some(c) = line_state.next() {
            if c == '`' {
                return None;
            }
        }
    }
    return Some(FencedCodeBlock(
        String::new(),
        true,
        t,
        info_str,
        pre_space_count,
        tick_count,
    ));
}
// fn fenced_code_block_encountered(
//     line: &Vec<char>,
//     char_offset_after_space: usize,
//     psc: usize,
// ) -> Option<Block> {
//     let t = line[char_offset_after_space];
//     if "~`".contains(t) {
//         let tick_count = line[char_offset_after_space..]
//             .iter()
//             .take_while(|c| **c == t)
//             .count();
//         if tick_count >= 3 {
//             if t == '`' {
//                 if (&line[char_offset_after_space + tick_count..])
//                     .iter()
//                     .any(|c| *c == '`')
//                 {
//                     return None;
//                 }
//             }
//             let info_string: Vec<char> = line[char_offset_after_space + tick_count..]
//                 .iter()
//                 .skip_while(|c| **c == ' ')
//                 .take_while(|c| **c != ' ')
//                 .map(|c| *c)
//                 .collect();
//             return Some(FencedCodeBlock(
//                 vec![],
//                 true,
//                 t,
//                 info_string,
//                 psc,
//                 tick_count,
//             ));
//         }
//     }
//     return None;
// }

fn atx_heading_encountered(line_state: &mut LineState) -> Option<Block> {
    let start_offset = dbg!(line_state.offset());
    line_state.consume_while_char_eq('#');
    let pound_count = line_state.offset() - start_offset;
    if 0 >= pound_count || pound_count > 6 {
        return None;
    }

    if line_state.is_empty() {
        return Some(Heading(Inline::new(vec![]), pound_count));
    }

    if !dbg!(line_state.consume_one_space()) {
        return None;
    }

    line_state.consume_indent();

    let mut trailing_line_state = line_state.clone();
    // let offset_after_space = line_state.offset();

    let mut str_out = String::new();
    let mut pre_space_seen = true;
    let mut post_space_pounds = None;
    let mut post_pounds_space = false;

    while let Some((c_i, c)) = line_state.next_and_index() {
        if " \t".contains(c) && post_space_pounds.is_none() {
            pre_space_seen = true
        } else if c == '#' && pre_space_seen {
            if post_pounds_space {
                //post_pounds_space implies that post_space_pounds is some.
                while trailing_line_state.offset() <= post_space_pounds.unwrap() {
                    str_out.push(trailing_line_state.next().unwrap());
                }
                post_pounds_space = false;
                post_space_pounds = Some(c_i);
            } else {
                post_space_pounds = Some(c_i);
            }
        } else if " \t".contains(c) && post_space_pounds.is_some() {
            post_pounds_space = true;
        } else {
            pre_space_seen = false;
            post_space_pounds = None;
            post_pounds_space = false;
            while trailing_line_state.offset() <= c_i {
                str_out.push(trailing_line_state.next().unwrap());
            }
        }
    }

    return Some(Heading(Inline::with_string(str_out), pound_count));
}

fn html_start_encountered(parsing_state: &mut ParsingState) -> Option<Block> {
    let line_state = &mut parsing_state.line_state.clone();
    let simple_starts_with = |target_str: &'static str| -> Box<dyn Fn(&mut LineState) -> bool> {
        Box::new(move |char_iter: &mut LineState| -> bool {
            let mut chars_seen = 0;
            // need to prevent zip from calling the last (unsuccessful) next.
            for (c_in, c_target) in char_iter
                .by_ref()
                .take(target_str.len())
                .zip(target_str.chars())
            {
                if c_in.to_ascii_lowercase() != c_target {
                    return false;
                }
                chars_seen += 1;
            }
            dbg!(target_str);
            chars_seen == target_str.len()
        })
    };
    let starts_with_match_case = |target_str: &'static str| -> Box<dyn Fn(&mut LineState) -> bool> {
        Box::new(move |char_iter: &mut LineState| -> bool {
            let mut chars_seen = 0;
            // need to prevent zip from calling the last (unsuccessful) next.
            for (c_in, c_target) in char_iter
                .by_ref()
                .take(target_str.len())
                .zip(target_str.chars())
            {
                if c_in != c_target {
                    return false;
                }
                chars_seen += 1;
            }
            dbg!(target_str);
            chars_seen == target_str.len()
        })
    };
    let special_tag: Box<dyn Fn(&mut LineState) -> bool> =
        Box::new(|char_iter: &mut LineState| -> bool {
            for s in ["<pre", "<script", "<style", "<textarea"] {
                let mut new_iter = char_iter.clone();
                if simple_starts_with(s)(&mut new_iter)
                    && (new_iter.is_empty() || new_iter.next_if(|c| " \t>".contains(c)).is_some())
                {
                    return true;
                }
            }
            false
        });

    let exclamation: Box<dyn Fn(&mut LineState) -> bool> =
        Box::new(|char_iter: &mut LineState| -> bool {
            return simple_starts_with("<!")(char_iter)
                && char_iter.next_if(|c| c.is_ascii_alphabetic()).is_some();
        });
    for (start_con, end_strs) in [
        (
            special_tag,
            vec!["</pre>", "</script>", "</style>", "</textarea>"],
        ),
        (simple_starts_with("<!--"), vec!["-->"]),
        (simple_starts_with("<?"), vec!["?>"]),
        (exclamation, vec![">"]),
        (starts_with_match_case("<![CDATA["), vec!["]]>"]),
    ] {
        let mut line_state_clone = line_state.clone();
        if start_con(&mut line_state_clone) {
            let mut string_out: String = String::new();
            for _ in 0..parsing_state.line_state.space_from_last_structure {
                string_out.push(' ');
            }
            for c in &mut *line_state {
                string_out.push(c);
            }

            string_out.push_str(&(line_state.collect::<String>()));
            let is_open = !end_strs.iter().any(|s| string_out.contains(s));
            return Some(HTMLBlock(
                string_out,
                is_open,
                ContainsStrings(end_strs.iter().map(|s| s.chars().collect()).collect()),
            ));
        }
    }

    //todo: use tries?
    let reserved_tags = [
        "address",
        "article",
        "aside",
        "base",
        "basefont",
        "blockquote",
        "body",
        "caption",
        "center",
        "col",
        "colgroup",
        "dd",
        "details",
        "dialog",
        "dir",
        "div",
        "dl",
        "dt",
        "fieldset",
        "figcaption",
        "figure",
        "footer",
        "form",
        "frame",
        "frameset",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "head",
        "header",
        "hr",
        "html",
        "iframe",
        "legend",
        "li",
        "link",
        "main",
        "menu",
        "menuitem",
        "nav",
        "noframes",
        "ol",
        "optgroup",
        "option",
        "p",
        "param",
        "search",
        "section",
        "summary",
        "table",
        "tbody",
        "td",
        "tfoot",
        "th",
        "thead",
        "title",
        "tr",
        "track",
        "ul",
    ];

    let mut eat_lt_iter = line_state.clone();
    dbg!(eat_lt_iter.next_if_char_eq('<'))?;
    dbg!("---------------------------------------------");
    dbg!(&eat_lt_iter);
    dbg!("---------------------------------------------");

    for rt in reserved_tags {
        let mut line_state_clone = eat_lt_iter.clone();
        if (simple_starts_with(rt)(&mut line_state_clone))
            && dbg!(
                (dbg!(line_state_clone.is_empty())
                    || dbg!(&mut line_state_clone)
                        .next_if(|c| " \t>".contains(c))
                        .is_some()
                    || (line_state_clone.next_if_char_eq('/').is_some()
                        && line_state_clone.next_if_char_eq('>').is_some()))
            )
        {
            let mut string_out = String::new();
            for _ in 0..parsing_state.line_state.space_from_last_structure {
                string_out.push(' ');
            }
            for c in line_state {
                string_out.push(c);
            }
            return Some(HTMLBlock(string_out, true, BlankLine));
        }
    }

    // non reserved (type 7) cant interrupt paragraph
    if parsing_state.open_par_above {
        return None;
    }

    let mut tag_finder = eat_lt_iter.char_iter.clone();
    match tag_finder.next()? {
        '/' => parse_closing_tag(&mut tag_finder),
        c => {
            if c.is_ascii_alphabetic() {
                parse_opening_tag(&mut tag_finder)
            } else {
                None
            }
        }
    };

    tag_finder.consume_while(|c| " \t".contains(c));
    if tag_finder.is_empty() {
        let mut string_out = String::new();
        for _ in 0..parsing_state.line_state.space_from_last_structure {
            string_out.push(' ');
        }
        for c in &mut *line_state {
            string_out.push(c);
        }
        return Some(HTMLBlock(string_out, true, BlankLine));
    } else {
        return None;
    }
}

//TODO: make this use
fn close_paragraph(parsing_state: &mut ParsingState) {
    if !parsing_state.open_par_exists {
        parsing_state.open_par_above = false;
        return;
    } //no paragraph to close, job done.

    let Paragraph(il, b @ true) = parsing_state.document.get_last_block() else {
        panic!()
    };

    let mut chars = PeekableCharIndices::new(il.string.char_indices());

    'collect_lrds: loop {
        let mut current_iteration_iter = chars.clone();
        // if current_iteration_iter.next_if_char_eq('[').is_none() {
        //     break 'collect_lrds;
        // }
        let mut link_lab_iter_start = current_iteration_iter.clone();

        let Some(link_label_end) = parse_link_label(&mut current_iteration_iter) else {
            break 'collect_lrds;
        };

        if current_iteration_iter.next_if_char_eq(':').is_none() {
            break 'collect_lrds;
        }

        //optional spaces or tabs
        current_iteration_iter.consume_while(|c| " \t\n".contains(c));

        let mut link_dest_iter_start = current_iteration_iter.clone();
        let Some((mut link_dest_end, link_dest_in_brackets)) =
            parse_link_destination(&mut current_iteration_iter)
        else {
            break 'collect_lrds;
        };
        if link_dest_in_brackets {
            link_dest_iter_start.next();
            link_dest_end -= 1;
        }

        current_iteration_iter.consume_while(|c| " \t".contains(c));

        let is_valid_lrd = current_iteration_iter.next_if_char_eq('\n').is_some();

        // the way we created paragraphs means that pre_whitespace in line is already stripped
        // so !c.is_whitespace() is true for all c that follow an \n
        let mut link_title_iter_start = current_iteration_iter.clone();
        let mut current_iter_clone = current_iteration_iter.clone();

        if let Some(link_title_offset) = parse_link_title(&mut current_iter_clone)
            && {
                current_iter_clone.consume_while(|c| " \t".contains(c));
                match current_iter_clone.peek() {
                    Some('\n') | None => true,
                    _ => false,
                }
            }
        {
            parsing_state
                .lrd_table
                .entry(normalize_label(
                    &(link_lab_iter_start.collect_until_offset(link_label_end)),
                ))
                .or_insert((
                    link_dest_iter_start.collect_until_offset(link_dest_end),
                    link_title_iter_start.collect_until_offset(link_title_offset),
                ));
            chars = current_iter_clone;
            continue 'collect_lrds;
        } else if is_valid_lrd {
            parsing_state
                .lrd_table
                .entry(normalize_label(
                    &(link_lab_iter_start.collect_until_offset(link_label_end)),
                ))
                .or_insert((
                    link_dest_iter_start.collect_until_offset(link_dest_end),
                    String::new(),
                ));
            chars = current_iteration_iter;
            continue 'collect_lrds;
        };
        break 'collect_lrds;
    }

    if chars.offset() > 0 {
        let mut new_str = chars.collect();
        mem::swap(&mut new_str, &mut il.string);
    }
    *b = false;

    parsing_state.open_par_above = false;
    parsing_state.open_par_exists = false;
}

// #[cfg(test)]
// mod cp_tests {
//     use super::*;
//     use pretty_assertions::assert_eq;
//
//     fn test_cp(
//         ast: &mut Block,
//         expected_ast: &mut Block,
//         exp_table: &mut HashMap<Vec<char>, (Vec<char>, Vec<char>)>,
//     ) {
//         let (mut opa, mut ope) = (true, true);
//         let mut lrd_table: HashMap<Vec<char>, (Vec<char>, Vec<char>)> = HashMap::new();
//         close_paragraph(ast, &mut opa, &mut ope, &mut lrd_table);
//         assert_eq!(
//             (expected_ast, exp_table, false, false),
//             (ast, &mut lrd_table, opa, ope)
//         )
//     }
//
//     #[test]
//     fn test_cp_no_def() {
//         test_cp(
//             &mut Document(vec![Paragraph(
//                 Inline::new("hello".chars().collect()),
//                 true,
//             )]),
//             &mut Document(vec![Paragraph(
//                 Inline::new("hello".chars().collect()),
//                 false,
//             )]),
//             &mut HashMap::new(),
//         )
//     }
//
//     #[test]
//     fn test_cp_no_title() {
//         test_cp(
//             &mut Document(vec![Paragraph(
//                 Inline::new("[hello]:link".chars().collect()),
//                 true,
//             )]),
//             &mut Document(vec![Paragraph(Inline::new("".chars().collect()), false)]),
//             &mut HashMap::from([(
//                 "hello".chars().collect(),
//                 ("link".chars().collect(), "".chars().collect()),
//             )]),
//         )
//     }
//
//     #[test]
//     fn test_cp_title() {
//         test_cp(
//             &mut Document(vec![Paragraph(
//                 Inline::new("[hello]:link (your_mom)".chars().collect()),
//                 true,
//             )]),
//             &mut Document(vec![Paragraph(Inline::new("".chars().collect()), false)]),
//             &mut HashMap::from([(
//                 "hello".chars().collect(),
//                 ("link".chars().collect(), "your_mom".chars().collect()),
//             )]),
//         )
//     }
//
//     #[test]
//     fn test_cp_multiline() {
//         test_cp(
//             &mut Document(vec![Paragraph(
//                 Inline::new("[\nhel\nlo\n]:\nlink \n(your_mom)".chars().collect()),
//                 true,
//             )]),
//             &mut Document(vec![Paragraph(Inline::new("".chars().collect()), false)]),
//             &mut HashMap::from([(
//                 "hel lo".chars().collect(),
//                 ("link".chars().collect(), "your_mom".chars().collect()),
//             )]),
//         )
//     }
//     #[test]
//     fn test_cp_title_fail() {
//         test_cp(
//             &mut Document(vec![Paragraph(
//                 Inline::new("[\nhel\nlo\n]:\nlink    \n(your_mom".chars().collect()),
//                 true,
//             )]),
//             &mut Document(vec![Paragraph(
//                 Inline::new("(your_mom".chars().collect()),
//                 false,
//             )]),
//             &mut HashMap::from([(
//                 "hel lo".chars().collect(),
//                 ("link".chars().collect(), "".chars().collect()),
//             )]),
//         )
//     }
//
//     #[test]
//     fn test_cp_link_fail_1() {
//         test_cp(
//             &mut Document(vec![Paragraph(
//                 Inline::new("[\nhel\nlo\n]:\n)link    \n(your_mom".chars().collect()),
//                 true,
//             )]),
//             &mut Document(vec![Paragraph(
//                 Inline::new("[\nhel\nlo\n]:\n)link    \n(your_mom".chars().collect()),
//                 false,
//             )]),
//             &mut HashMap::from([]),
//         )
//     }
//     #[test]
//     fn test_cp_link_fail_2() {
//         test_cp(
//             &mut Document(vec![Paragraph(
//                 Inline::new("[\nhel\nlo\n]:\n(link(())    \n(your_mom".chars().collect()),
//                 true,
//             )]),
//             &mut Document(vec![Paragraph(
//                 Inline::new("[\nhel\nlo\n]:\n(link(())    \n(your_mom".chars().collect()),
//                 false,
//             )]),
//             &mut HashMap::from([]),
//         )
//     }
//     #[test]
//     fn test_cp_more_paragraph() {
//         test_cp(
//             &mut Document(vec![Paragraph(
//                 Inline::new(
//                     "[\nhel\nlo\n]:\nlink    \n'your_mom'\nand theres more"
//                         .chars()
//                         .collect(),
//                 ),
//                 true,
//             )]),
//             &mut Document(vec![Paragraph(
//                 Inline::new("and theres more".chars().collect()),
//                 false,
//             )]),
//             &mut HashMap::from([(
//                 "hel lo".chars().collect(),
//                 ("link".chars().collect(), "your_mom".chars().collect()),
//             )]),
//         )
//     }
//     #[test]
//     fn test_cp_2_def() {
//         test_cp(
//             &mut Document(vec![Paragraph(
//                 Inline::new(
//                     "[\nhel\nlo\n]:\nlink    \n'your_mom'\n[def2]:link2 \"desc_2\""
//                         .chars()
//                         .collect(),
//                 ),
//                 true,
//             )]),
//             &mut Document(vec![Paragraph(Inline::new("".chars().collect()), false)]),
//             &mut HashMap::from([
//                 (
//                     "hel lo".chars().collect(),
//                     ("link".chars().collect(), "your_mom".chars().collect()),
//                 ),
//                 (
//                     "def2".chars().collect(),
//                     ("link2".chars().collect(), "desc_2".chars().collect()),
//                 ),
//             ]),
//         )
//     }
//     #[test]
//     fn test_cp_2_def_override() {
//         test_cp(
//             &mut Document(vec![Paragraph(
//                 Inline::new(
//                     "[\nhel\nlo\n]:\nlink    \n'your_mom'\n[hel    lo   ]:link2 \"desc_2\""
//                         .chars()
//                         .collect(),
//                 ),
//                 true,
//             )]),
//             &mut Document(vec![Paragraph(Inline::new("".chars().collect()), false)]),
//             &mut HashMap::from([(
//                 "hel lo".chars().collect(),
//                 ("link".chars().collect(), "your_mom".chars().collect()),
//             )]),
//         )
//     }
//     #[test]
//     fn test_cp_fails() {
//         test_cp(
//             &mut Document(vec![Paragraph(
//                 Inline::new(
//                     "[\nhel\nlo\n]:\n<link    \n'your_mom'\n[hel    lo   ]:link2 \"desc_2\""
//                         .chars()
//                         .collect(),
//                 ),
//                 true,
//             )]),
//             &mut Document(vec![Paragraph(
//                 Inline::new(
//                     "[\nhel\nlo\n]:\n<link    \n'your_mom'\n[hel    lo   ]:link2 \"desc_2\""
//                         .chars()
//                         .collect(),
//                 ),
//                 false,
//             )]),
//             &mut HashMap::from([]),
//         )
//     }
//     #[test]
//     fn test_cp_2_escapes() {
//         test_cp(
//             &mut Document(vec![Paragraph(
//                 Inline::new(
//                     "[\nh\\el\nlo\n]:\nlink    \n'your_mom'\n[hel    lo   ]:link2 \"desc_2\""
//                         .chars()
//                         .collect(),
//                 ),
//                 true,
//             )]),
//             &mut Document(vec![Paragraph(Inline::new("".chars().collect()), false)]),
//             &mut HashMap::from([
//                 (
//                     "h\\el lo".chars().collect(),
//                     ("link".chars().collect(), "your_mom".chars().collect()),
//                 ),
//                 (
//                     "hel lo".chars().collect(),
//                     ("link2".chars().collect(), "desc_2".chars().collect()),
//                 ),
//             ]),
//         )
//     }
// }

fn create_new_block_starts(parsing_state: &mut ParsingState) {
    'blank_line: {
        if dbg!(parsing_state.line_state.is_blank_line()) {
            match parsing_state
                .document
                .get_block(parsing_state.open_block_depth)
            {
                IndentedCodeBlock(_, unrealized_blanks) => {
                    let line_state = &mut parsing_state.line_state;
                    line_state.consume_indent_until_sfls_ge(4);
                    unrealized_blanks.push('\n');

                    if line_state.space_from_last_structure > 4 {
                        for _ in 0..(line_state.space_from_last_structure - 4) {
                            unrealized_blanks.push(' ');
                        }
                    }
                    for ch in line_state {
                        unrealized_blanks.push(ch);
                    }
                    // let mut next_blank = vec![];
                    // // still need to be better recognizng tabs here
                    // if line.len() > *char_offset {
                    //     next_blank = line[*char_offset..].iter().map(|&c| c).collect();
                    // }
                    // unrealized_blanks.push(next_blank);
                }
                FencedCodeBlock(..) => break 'blank_line,
                HTMLBlock(_, true, ContainsStrings(_)) => break 'blank_line,
                HTMLBlock(_, is_open @ true, BlankLine) => *is_open = false,
                ListItem(blocks, continuable, _) => {
                    if blocks.is_empty() {
                        *continuable = false;
                    }
                }
                _ => (),
            }
            close_paragraph(parsing_state);
            parsing_state.blank_line_depth = Some(
                parsing_state
                    .document
                    .deepest_matched_blockquote(parsing_state.open_block_depth),
            );
            // make block quotes closed
            parsing_state
                .document
                .close_open_block(parsing_state.open_block_depth as i32);
            return;
        }
    }

    match parsing_state
        .document
        .get_block(parsing_state.open_block_depth)
    {
        IndentedCodeBlock(chars, spaces) => {
            // None Blank Line!
            chars.push_str(&spaces);
            *spaces = String::new();
            chars.push('\n');
            // dbg!(&char_offset);

            let line_state = &mut parsing_state.line_state;
            line_state.consume_indent_until_sfls_ge(4);
            if line_state.space_from_last_structure > 4 {
                for _ in 0..(line_state.space_from_last_structure - 4) {
                    chars.push(' ');
                }
            }
            for c in line_state {
                chars.push(c);
            }
            return;
        }
        FencedCodeBlock(chars, is_open @ true, marker, _, indent_count, marker_count) => {
            let mut try_to_close_chars = parsing_state.line_state.clone();
            try_to_close_chars.consume_indent_until_sfls_ge(4);
            if try_to_close_chars.space_from_last_structure <= 3 {
                match fenced_code_block_encountered(&mut try_to_close_chars) {
                    Some(FencedCodeBlock(_, _, t, infstr, _, tc)) => {
                        if t == *marker && infstr.len() == 0 && tc >= *marker_count {
                            *is_open = false;
                            return;
                        }
                    }
                    _ => (),
                }
            }
            // println!("line: {:?} can be added!", &line[*char_offset..]);
            let line_state = &mut parsing_state.line_state;
            line_state.consume_indent_until_sfls_ge(*indent_count);
            if line_state.space_from_last_structure > *indent_count {
                for _ in 0..(line_state.space_from_last_structure - *indent_count) {
                    chars.push(' ');
                }
            }
            for c in line_state {
                chars.push(c);
            }
            chars.push('\n');

            return;
        }
        //TODO
        HTMLBlock(chars, is_open @ true, end_condition) => {
            dbg!("HTMLBlock parent matched!");
            let iter_clone = parsing_state.line_state.clone();
            let current_line_as_str: String = iter_clone.collect();
            if let ContainsStrings(strs) = end_condition {
                for s in strs {
                    if current_line_as_str.contains(s.as_str()) {
                        *is_open = false;
                    }
                }
            }
            chars.push('\n');
            for c in parsing_state.line_state.clone() {
                chars.push(c);
            }
            return;
        }
        _ => (),
    }
    // we do not want to consume more than 4 for the case that we are creating an indented code
    // block in which case we have to copy in those spaces
    // let mut pre_space_line = parsing_state.line_state.clone();
    parsing_state.line_state.consume_indent_until_sfls_ge(4);
    parsing_state.set_open_pars();
    // obd is unmodified at this point, since we know we are not in a blank line at  this point,
    // that means that *something* will eventually get added to list item
    let mut depth_of_list_being_added_to = None;
    if let (ListItem(..), depth) = dbg!(
        parsing_state
            .document
            .get_general_container(parsing_state.open_block_depth)
    ) {
        depth_of_list_being_added_to = Some(depth - 1);
    }

    if parsing_state.line_state.space_from_last_structure <= 3 {
        // First check for SetextHeading
        // c_0 is first non space character after offset
        dbg!(&parsing_state.line_state);
        let c_0 = parsing_state.line_state.peek().unwrap();
        'setext_check: {
            if parsing_state.open_par_above && "-=".contains(c_0) {
                let mut try_to_setext_iter = parsing_state.line_state.clone();
                // the line is of the form "[pre-matched-structure][1-3 space](-|=)*' '*"
                try_to_setext_iter.consume_while_char_eq(c_0);

                // let dash_count = try_to_setext_iter.offset() - pre_dash_offset;

                if try_to_setext_iter.is_blank_line() {
                    close_paragraph(parsing_state);
                    let mut setext_inline = Inline::new(vec![]);
                    match parsing_state.get_open_block() {
                        Paragraph(il, _) => {
                            if il.string.is_empty() {
                                break 'setext_check;
                            }
                            while let Some(last_char) = il.string.pop() {
                                if " \t".contains(last_char) {
                                    continue;
                                } else {
                                    il.string.push(last_char);
                                    break;
                                }
                            }
                            mem::swap(&mut setext_inline, il);
                        }
                        _ => panic!(),
                    }
                    match parsing_state.document.get_general_container(parsing_state.open_block_depth).0 {
                        // by definition some container block
                        Document(blocks) |
                        BlockQuote(blocks, _) |
                        // List(blocks, _, list_type) => the only children of a list are list_blocks
                        ListItem(blocks,_, _) => {
                            *blocks.last_mut().unwrap() = Heading(setext_inline, if c_0 == '=' {1} else {2});
                        }
                        _ => panic!(),
                    }
                    return;
                }
            }
        }

        // next check for thematic break (before we add to list for priority reasons)
        // because it is established non empty, bounds check is not needed
        let mut thematic_break_line = parsing_state.line_state.clone();
        if thematic_break_encountered(&mut thematic_break_line) {
            close_paragraph(parsing_state);

            if let Some(list_depth) = depth_of_list_being_added_to {
                match parsing_state.document.get_block(list_depth) {
                    List(_, is_tight, _, _blank_line_encountered) => {
                        *is_tight = parsing_state
                            .blank_line_depth
                            .map_or(true, |d| d > list_depth)
                            && *is_tight
                    }
                    _ => unreachable!(),
                }
            }
            match (parsing_state
                .document
                .get_general_container(parsing_state.open_block_depth))
            .0
            {
                Document(blocks) | BlockQuote(blocks, _) | ListItem(blocks, _, _) => {
                    blocks.push(ThematicBreak);
                    return;
                }
                _ => panic!("should only match on general container"),
            }
        }

        //now check if we can add a new list item to an existing list
        let last_block_list: Option<ListType> = match parsing_state
            .document
            .get_block(parsing_state.open_block_depth)
        {
            List(_, _, lt, _) => Some(lt.clone()),
            _ => None,
        };

        if let Some(list_type) = last_block_list {
            // dbg!(last_block_list);
            // dbg!(pre_space_count);
            //now check if we can add a new list item to an existing list
            let mut list_item_iter = parsing_state.line_state.clone();
            match list_item_encountered(&mut list_item_iter) {
                Some((lt, li)) => {
                    if list_type.same_list_eq(&lt) {
                        close_paragraph(parsing_state);
                        depth_of_list_being_added_to = Some(parsing_state.open_block_depth);
                        match parsing_state.get_open_block() {
                            List(blocks, _, _, _) => blocks.push(li),
                            _ => unreachable!(),
                        }
                        parsing_state.open_block_depth += 1;
                        parsing_state.line_state = list_item_iter;
                        parsing_state.line_state.consume_indent_until_sfls_ge(4);
                    }
                }
                None => (),
            }
        }

        // keep looking for new block starts
        while parsing_state.line_state.space_from_last_structure <= 3 {
            let mut them_break_iter = parsing_state.line_state.clone();
            if thematic_break_encountered(&mut them_break_iter) {
                close_paragraph(parsing_state);
                match parsing_state
                    .document
                    .get_general_container(parsing_state.open_block_depth)
                    .0
                {
                    Document(blocks) | BlockQuote(blocks, _) | ListItem(blocks, _, _) => {
                        blocks.push(ThematicBreak);
                        return;
                    }
                    _ => panic!("should only match on general container"),
                }
            }
            let mut list_item_iter = parsing_state.line_state.clone();
            if let Some((lt, b)) = list_item_encountered(&mut list_item_iter) {
                //TODO: make empty lists not interrupt either
                if parsing_state.open_par_above {
                    match lt {
                        OrderedList(_, 1) | UnorderedList(_) => {
                            if list_item_iter.is_blank_line() {
                                break;
                            }
                        }
                        OrderedList(_, _) => break,
                    }
                } //only ordered lists starting with 1 can interrupt paragraphs. and must be non
                //empty afterwards
                close_paragraph(parsing_state);
                let (parent, new_obd) = parsing_state
                    .document
                    .get_general_container(parsing_state.open_block_depth);
                match parent {
                    Document(blocks) | BlockQuote(blocks, _) | ListItem(blocks, _, _) => {
                        blocks.push(List(vec![b], true, lt, false));
                        parsing_state.open_block_depth = new_obd + 2;
                        parsing_state.line_state = list_item_iter;
                        parsing_state.line_state.consume_indent_until_sfls_ge(4);
                        continue;
                    }
                    _ => unreachable!(),
                }
            }
            let mut block_quote_iter = parsing_state.line_state.clone();
            if block_quote_encountered(&mut block_quote_iter) {
                close_paragraph(parsing_state);
                let (parent, new_obd) = parsing_state
                    .document
                    .get_general_container(parsing_state.open_block_depth);
                match parent {
                    Document(blocks) | BlockQuote(blocks, _) | ListItem(blocks, _, _) => {
                        blocks.push(BlockQuote(vec![], true));
                        parsing_state.open_block_depth = new_obd + 1;
                        parsing_state.line_state = block_quote_iter;
                        parsing_state.line_state.consume_indent_until_sfls_ge(4);
                        continue;
                    }
                    _ => unreachable!(),
                }
            }
            let mut fcb_iter = parsing_state.line_state.clone();
            if let Some(fcb) = fenced_code_block_encountered(&mut fcb_iter) {
                close_paragraph(parsing_state);
                let (parent, new_obd) = parsing_state
                    .document
                    .get_general_container(parsing_state.open_block_depth);
                match parent {
                    Document(blocks) | BlockQuote(blocks, _) | ListItem(blocks, _, _) => {
                        blocks.push(fcb);
                        parsing_state.open_block_depth = new_obd + 1;
                        return;
                    }
                    _ => unreachable!(),
                }
            }
            let mut atx_iter = parsing_state.line_state.clone();
            // dbg!("TRYING_TO_ATX_ITER");
            if let Some(atxh) = atx_heading_encountered(&mut atx_iter) {
                close_paragraph(parsing_state);
                let parent = parsing_state
                    .document
                    .get_general_container(parsing_state.open_block_depth)
                    .0;
                match parent {
                    Document(blocks) | BlockQuote(blocks, _) | ListItem(blocks, _, _) => {
                        blocks.push(atxh);
                        return;
                    }
                    _ => unreachable!(),
                }
            }
            if let Some(html) = html_start_encountered(parsing_state) {
                close_paragraph(parsing_state);
                let parent = parsing_state
                    .document
                    .get_general_container(parsing_state.open_block_depth)
                    .0;
                match parent {
                    Document(blocks) | BlockQuote(blocks, _) | ListItem(blocks, _, _) => {
                        blocks.push(html);
                        return;
                    }
                    _ => unreachable!(),
                }
            }
            break;
        }
    }

    // at this point, if the last matched block is a list item, then we can let potentially
    // non-tight lists actually be non-tight
    if let Some(depth_of_list_block) = depth_of_list_being_added_to {
        match parsing_state.document.get_block(depth_of_list_block) {
            List(_, is_tight, _, _blank_line_encountered) => {
                *is_tight = parsing_state
                    .blank_line_depth
                    .map_or(true, |d| dbg!(d > depth_of_list_block))
                    && *is_tight
            }
            _ => unreachable!(),
        }
    }
    parsing_state.blank_line_depth = None;

    // its a blank line from here on out
    if parsing_state.line_state.is_blank_line() {
        return;
    }

    //now check if theres a paragraph we can continue lazily
    match parsing_state.document.get_last_block() {
        Paragraph(inline, true) => {
            inline.string.push('\n');
            parsing_state.line_state.consume_indent();
            for c in parsing_state.line_state.clone() {
                inline.string.push(c);
            }
            return;
        }
        _ => (),
    }

    // if theres no paragraph to continue, check to create an indented code block
    if parsing_state.line_state.space_from_last_structure >= 4 {
        match parsing_state
            .document
            .get_general_container(parsing_state.open_block_depth)
            .0
        {
            Document(blocks) | BlockQuote(blocks, _) | ListItem(blocks, _, _) => {
                let mut str_out = String::new();
                for _ in 0..parsing_state.line_state.space_from_last_structure - 4 {
                    str_out.push(' ');
                }
                for c in parsing_state.line_state.clone() {
                    str_out.push(c);
                }
                blocks.push(IndentedCodeBlock(str_out, String::new()));
                return;
            }
            _ => unreachable!(),
        }
    }

    // finally, we can create a new paragraph with the line remaining
    // since we have been consuming spaces beforehand
    match parsing_state
        .document
        .get_general_container(parsing_state.open_block_depth)
        .0
    {
        Document(blocks) | BlockQuote(blocks, _) | ListItem(blocks, _, _) => {
            assert!(
                parsing_state
                    .line_state
                    .next_if(|c| " \t".contains(c))
                    .is_none()
            );
            blocks.push(Paragraph(
                Inline::with_string(parsing_state.line_state.clone().collect()),
                true,
            ));
            return;
        }
        _ => unreachable!(),
    }
}

// #[cfg(test)]
// mod cnbs_tests {
//     use super::*;
//     use pretty_assertions::assert_eq;
//     fn test_cnbs(ast: &mut Block, line: &str, expected_ast: &mut Block, exp_offset: usize) {
//         let line: Vec<char> = line.chars().collect();
//         // println!("{:?}", line);
//         let mut char_offset: usize = 0;
//         let mut effective_column_number: usize = 0;
//         let mut additional_possible_spaces: usize = 0;
//         let mut obd = 0;
//         let mut blank_line_depth = None;
//         check_continuation_conditions(
//             &ast,
//             &line,
//             &mut char_offset,
//             &mut effective_column_number,
//             &mut additional_possible_spaces,
//             &mut obd,
//         );
//         create_new_block_starts(
//             ast,
//             &line,
//             &mut char_offset,
//             &mut effective_column_number,
//             &mut additional_possible_spaces,
//             &mut obd,
//             &mut blank_line_depth,
//             &mut HashMap::new(),
//         );
//         assert_eq!((ast, char_offset), (expected_ast, exp_offset));
//     }
//
//     #[test]
//     fn test_cnbs_1() {
//         test_cnbs(
//             &mut Document(vec![BlockQuote(
//                 vec![Paragraph(Inline::new(vec!['a', 'b']), true)],
//                 true,
//             )]),
//             ">-",
//             &mut Document(vec![BlockQuote(
//                 vec![Heading(Inline::new(vec!['a', 'b']), 2)],
//                 true,
//             )]),
//             1,
//         );
//     }
//
//     #[test]
//     fn test_cnbs_2() {
//         test_cnbs(
//             &mut Document(vec![Paragraph(Inline::new(vec!['a', 'b']), true)]),
//             "-",
//             &mut Document(vec![Heading(Inline::new(vec!['a', 'b']), 2)]),
//             0,
//         );
//     }
//
//     #[test]
//     fn test_cnbs_3() {
//         test_cnbs(
//             &mut Document(vec![Paragraph(Inline::new(vec!['a', 'b']), true)]),
//             "=          ",
//             &mut Document(vec![Heading(Inline::new(vec!['a', 'b']), 1)]),
//             10,
//         );
//     }
//
//     #[test]
//     fn test_cnbs_4() {
//         test_cnbs(
//             &mut Document(vec![Paragraph(Inline::new(vec!['a', 'b']), true)]),
//             "- -",
//             &mut Document(vec![
//                 Paragraph(Inline::new(vec!['a', 'b']), false),
//                 List(
//                     vec![ListItem(
//                         vec![List(
//                             vec![ListItem(vec![], true, 2)],
//                             true,
//                             UnorderedList('-'),
//                             false,
//                         )],
//                         true,
//                         2,
//                     )],
//                     true,
//                     UnorderedList('-'),
//                     false,
//                 ),
//             ]),
//             3,
//         );
//     }
//
//     #[test]
//     fn test_cnbs_5() {
//         test_cnbs(
//             &mut Document(vec![BlockQuote(
//                 vec![Paragraph(Inline::new(vec!['a', 'b']), true)],
//                 false,
//             )]),
//             ">-",
//             &mut Document(vec![
//                 BlockQuote(vec![Paragraph(Inline::new(vec!['a', 'b']), false)], false),
//                 BlockQuote(
//                     vec![List(
//                         vec![ListItem(vec![], true, 2)],
//                         true,
//                         UnorderedList('-'),
//                         false,
//                     )],
//                     true,
//                 ),
//             ]),
//             2,
//         );
//     }
//
//     #[test]
//     fn test_cnbs_6() {
//         test_cnbs(
//             &mut Document(vec![BlockQuote(
//                 vec![Paragraph(Inline::new(vec!['a', 'b']), true)],
//                 true,
//             )]),
//             ">---",
//             &mut Document(vec![BlockQuote(
//                 vec![Heading(Inline::new(vec!['a', 'b']), 2)],
//                 true,
//             )]),
//             3,
//         );
//     }
//
//     #[test]
//     fn test_cnbs_7() {
//         test_cnbs(
//             &mut Document(vec![BlockQuote(
//                 vec![Paragraph(Inline::new(vec!['a', 'b']), true)],
//                 true,
//             )]),
//             ">***",
//             &mut Document(vec![BlockQuote(
//                 vec![Paragraph(Inline::new(vec!['a', 'b']), false), ThematicBreak],
//                 true,
//             )]),
//             3,
//         );
//     }
//
//     #[test]
//     fn test_cnbs_8() {
//         test_cnbs(
//             &mut Document(vec![BlockQuote(
//                 vec![List(
//                     vec![ListItem(vec![ThematicBreak], true, 2)],
//                     true,
//                     UnorderedList('*'),
//                     false,
//                 )],
//                 true,
//             )]),
//             ">***",
//             &mut Document(vec![BlockQuote(
//                 vec![
//                     List(
//                         vec![ListItem(vec![ThematicBreak], true, 2)],
//                         true,
//                         UnorderedList('*'),
//                         false,
//                     ),
//                     ThematicBreak,
//                 ],
//                 true,
//             )]),
//             3,
//         );
//     }
//
//     #[test]
//     fn test_cnbs_9() {
//         test_cnbs(
//             &mut Document(vec![BlockQuote(
//                 vec![List(
//                     vec![ListItem(vec![ThematicBreak], true, 2)],
//                     true,
//                     UnorderedList('*'),
//                     false,
//                 )],
//                 true,
//             )]),
//             ">   ***",
//             &mut Document(vec![BlockQuote(
//                 vec![List(
//                     vec![ListItem(vec![ThematicBreak, ThematicBreak], true, 2)],
//                     true,
//                     UnorderedList('*'),
//                     false,
//                 )],
//                 true,
//             )]),
//             6,
//         );
//     }
//
//     #[test]
//     fn test_cnbs_continue_list_item_1() {
//         test_cnbs(
//             &mut Document(vec![List(
//                 vec![ListItem(vec![ThematicBreak], true, 2)],
//                 true,
//                 UnorderedList('-'),
//                 false,
//             )]),
//             "-",
//             &mut Document(vec![List(
//                 vec![
//                     ListItem(vec![ThematicBreak], true, 2),
//                     ListItem(vec![], true, 2),
//                 ],
//                 true,
//                 UnorderedList('-'),
//                 false,
//             )]),
//             1,
//         );
//     }
//     #[test]
//     fn test_cnbs_continue_list_item_2() {
//         test_cnbs(
//             &mut Document(vec![List(
//                 vec![ListItem(vec![ThematicBreak], true, 2)],
//                 true,
//                 OrderedList('.', 2),
//                 false,
//             )]),
//             "123. ",
//             &mut Document(vec![List(
//                 vec![
//                     ListItem(vec![ThematicBreak], true, 2),
//                     ListItem(vec![], true, 5),
//                 ],
//                 true,
//                 OrderedList('.', 2),
//                 false,
//             )]),
//             5,
//         );
//     }
//
//     #[test]
//     fn test_cnbs_new_list_1() {
//         test_cnbs(
//             &mut Document(vec![]),
//             "123. ",
//             &mut Document(vec![List(
//                 vec![ListItem(vec![], true, 5)],
//                 true,
//                 OrderedList('.', 123),
//                 false,
//             )]),
//             5,
//         );
//     }
//
//     #[test]
//     fn test_cnbs_new_list_2() {
//         test_cnbs(
//             &mut Document(vec![List(
//                 vec![ListItem(vec![ThematicBreak], true, 2)],
//                 true,
//                 OrderedList('.', 2),
//                 false,
//             )]),
//             " -  ",
//             &mut Document(vec![
//                 List(
//                     vec![ListItem(vec![ThematicBreak], true, 2)],
//                     true,
//                     OrderedList('.', 2),
//                     false,
//                 ),
//                 List(
//                     vec![ListItem(vec![], true, 3)],
//                     true,
//                     UnorderedList('-'),
//                     false,
//                 ),
//             ]),
//             4,
//         );
//     }
//
//     #[test]
//     fn test_cnbs_atx_heading_1() {
//         test_cnbs(
//             &mut Document(vec![]),
//             "#",
//             &mut Document(vec![Heading(Inline::new(vec![]), 1)]),
//             1,
//         );
//     }
//
//     #[test]
//     fn test_cnbs_atx_heading_2() {
//         test_cnbs(
//             &mut Document(vec![]),
//             "  #           #       ",
//             &mut Document(vec![Heading(Inline::new(vec![]), 1)]),
//             22,
//         );
//     }
//
//     #[test]
//     fn test_cnbs_atx_heading_3() {
//         test_cnbs(
//             &mut Document(vec![]),
//             "  #           #       hello # ",
//             &mut Document(vec![Heading(
//                 Inline::new("#       hello".chars().collect()),
//                 1,
//             )]),
//             30,
//         );
//     }
// }
