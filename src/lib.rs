use crate::Block::*;
use crate::ListType::*;
use crate::ast_types::Block;
use crate::ast_types::ListType;

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

    fn is_blank_line(line: Vec<char>, offset: usize) {
        line.iter()
            .take(offset)
            .any(|c| *c != ' ' && *c != char::from_u32(0x09).unwrap());
    }

    // 1st create block structure of document
    let mut document = Document(vec![]);

    for line in lines {
        // first check continuation conditions
        let mut offset: usize = 0;
        let mut last_matched_container: Vec<usize> = vec![];
        check_continuation_conditions(&document, &line, &mut offset, &mut last_matched_container);

        // now we look for the start of any new structure
        let mut last_open = document.get_block(&mut last_matched_container.iter());
    }

    // see if we can add a new item to the list
    todo!();
}

#[allow(unreachable_code)]
fn check_continuation_conditions(
    document: &Block,
    line: &Vec<char>,
    offset: &mut usize,
    last_matched_container: &mut Vec<usize>,
) {
    let mut current_block = document;
    let mut prev_block_offset = 0;
    // println!("called ccc!");
    'outer: loop {
        match current_block {
            Document(blocks) => match blocks.last() {
                None => break,
                Some(b) => {
                    // println!("has children!");
                    prev_block_offset = blocks.len() - 1;
                    current_block = b;
                }
            },
            BlockQuote(blocks, is_open) => {
                if !*is_open {
                    break 'outer;
                }
                // println!("checing cc for a bq!");
                for i in 0..4 {
                    if line.len() <= *offset + i {
                        break;
                    }
                    if line[*offset + i] == '>' {
                        //bounds check
                        if line.len() <= *offset + i + 1 {
                            *offset += i + 1;
                            last_matched_container.push(prev_block_offset);
                            match blocks.last() {
                                None => break 'outer,
                                Some(b) => {
                                    prev_block_offset = blocks.len() - 1;
                                    current_block = b;
                                }
                            }
                            continue 'outer;
                        }
                        if line[*offset + i + 1] == ' ' {
                            *offset += i + 2;
                        } else {
                            *offset += i + 1;
                        }
                        last_matched_container.push(prev_block_offset);
                        match blocks.last() {
                            None => break 'outer,
                            Some(b) => {
                                prev_block_offset = blocks.len() - 1;
                                current_block = b;
                            }
                        }
                        continue 'outer;
                    }
                    if line[*offset + i] != ' ' {
                        break;
                    }
                }
                break;
            }
            List(list_items, _, _) => {
                last_matched_container.push(prev_block_offset);
                prev_block_offset = list_items.len() - 1;
                // lists are guaranteed to have at least one item
                current_block = list_items.last().unwrap();
            }
            ListItem(blocks, indent_amount) => {
                for i in 0..*indent_amount {
                    if line.len() <= *offset + i || line[*offset + i] != ' ' {
                        break 'outer;
                    }
                }
                *offset += *indent_amount;
                last_matched_container.push(prev_block_offset);
                match blocks.last() {
                    None => break 'outer,
                    Some(b) => {
                        prev_block_offset = blocks.len() - 1;
                        current_block = b;
                    }
                }
            }
            ATXHeading(_, _) => break,
            SetextHeading(_, _) => break,
            Paragraph(_, is_open) => {
                if !is_blank_line(line, *offset) && *is_open {
                    last_matched_container.push(prev_block_offset);
                }
                break;
            }
            ThematicBreak => break,
            IndentedCodeBlock(_) => {
                for i in 0..4 {
                    if line.len() >= *offset + i {
                        break;
                    }
                    if line[*offset + i] != ' ' {
                        break;
                    }
                }
                *offset += 4;
                last_matched_container.push(prev_block_offset);
                break;
            }
            FencedCodeBlock(items, tic, tic_count, indent_count) => {
                last_matched_container.push(prev_block_offset);
                break;
            }
            HTMLBlock(items) => todo!(),
        }
    }
}

fn is_blank_line(line: &Vec<char>, offset: usize) -> bool {
    line.iter()
        .take(offset)
        .any(|c| *c != ' ' && *c != char::from_u32(0x09).unwrap())
}

fn test_cc(ast: &mut Block, line: &str, expected_block: &mut Block, exp_offset: usize) {
    let line: Vec<char> = line.chars().collect();
    // println!("{:?}", line);
    let mut lmc = vec![];
    let mut offset = 0;
    check_continuation_conditions(&ast, &line, &mut offset, &mut lmc);
    println!("{}", line[offset]);
    assert_eq!(
        (ast.get_block(&mut lmc.iter()), offset),
        (expected_block, exp_offset)
    );
}

#[test]
fn test_cc_1() {
    let mut test_tree = Document(vec![]);
    test_cc(&mut test_tree, "> abc", &mut Document(vec![]), 0);
}

#[test]
fn test_cc_qb_1() {
    let mut test_tree = Document(vec![BlockQuote(vec![ThematicBreak], true)]);
    let mut exp_tree = BlockQuote(vec![ThematicBreak], true);
    test_cc(&mut test_tree, ">hello", &mut exp_tree, 1);
}

#[test]
fn test_cc_qb_2() {
    let mut test_tree = Document(vec![BlockQuote(vec![ThematicBreak], true)]);
    let mut exp_tree = BlockQuote(vec![ThematicBreak], true);
    test_cc(&mut test_tree, "> hello", &mut exp_tree, 2);
}

#[test]
fn test_cc_qb_3() {
    let mut test_tree = Document(vec![BlockQuote(vec![ThematicBreak], true)]);
    let mut exp_tree = BlockQuote(vec![ThematicBreak], true);
    test_cc(&mut test_tree, "   > hello", &mut exp_tree, 5);
}

#[test]
fn test_cc_qb_4() {
    let mut test_tree = Document(vec![BlockQuote(vec![ThematicBreak], true)]);
    let mut exp_tree = BlockQuote(vec![ThematicBreak], true);
    test_cc(&mut test_tree, " > hello", &mut exp_tree, 3);
}

#[test]
fn test_cc_qb_5() {
    let mut test_tree = Document(vec![BlockQuote(vec![ThematicBreak], true)]);
    let mut exp_tree = BlockQuote(vec![ThematicBreak], true);
    test_cc(&mut test_tree, ">  hello", &mut exp_tree, 2);
}

#[test]
fn test_cc_qb_6() {
    let mut test_tree = Document(vec![BlockQuote(vec![ThematicBreak], false)]);
    let mut exp_tree = test_tree.clone();
    test_cc(&mut test_tree, ">  hello", &mut exp_tree, 0);
}

#[test]
fn test_cc_list_1() {
    let mut test_tree = Document(vec![List(
        vec![ListItem(vec![], 2)],
        true,
        UnorderedList('*'),
    )]);
    let mut exp_tree = ListItem(vec![], 2);
    test_cc(&mut test_tree, "  >  hello", &mut exp_tree, 2);
}

#[test]
fn test_cc_list_2() {
    let mut test_tree = Document(vec![List(
        vec![ListItem(vec![], 2)],
        true,
        UnorderedList('*'),
    )]);
    let mut exp_tree = List(vec![ListItem(vec![], 2)], true, UnorderedList('*'));
    test_cc(&mut test_tree, " >  hello", &mut exp_tree, 0);
}

#[test]
fn test_cc_both_3() {
    let mut test_tree = Document(vec![List(
        vec![ListItem(vec![BlockQuote(vec![], true)], 2)],
        true,
        UnorderedList('*'),
    )]);
    let mut exp_tree = BlockQuote(vec![], true);
    test_cc(&mut test_tree, "  > hello", &mut exp_tree, 4);
}
/*
Each line that is processed has an effect on this tree.
The line is analyzed and, depending on its contents, the
document may be altered in one or more of the following ways:
    1. One or more open blocks may be closed.
    2. One or more new blocks may be created as children of the last open block.
    3. Text may be added to the last (deepest) open block remaining on the tree.
 */
