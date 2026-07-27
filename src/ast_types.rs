use std::ops::ControlFlow::Break;

use crate::ast_types::{Block::*, InlineContent::*, ListType::*};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum InlineContent {
    Softbreak,
    Text(Vec<char>),
}

type Inline = Vec<InlineContent>;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Block {
    Document(Vec<Block>),
    BlockQuote(Vec<Block>, bool),
    List(Vec<Block>, bool, ListType),
    ListItem(Vec<Block>, usize),
    ATXHeading(Inline, usize),
    SetextHeading(Inline, usize),
    Paragraph(Inline, bool),
    ThematicBreak,
    IndentedCodeBlock(Vec<char>),
    FencedCodeBlock(Vec<char>, bool, char, Vec<char>, usize, usize),
    // (contents, is_open, marking char, info_string, indend_count, tilde_count)
    HTMLBlock(Inline),
}

impl Block {
    pub fn get_block(&mut self, open_block_depth: usize) -> &mut Block {
        self.get_block_helper(open_block_depth)
    }

    fn get_block_helper(&mut self, open_block_depth: usize) -> &mut Block {
        match open_block_depth {
            0 => self,
            x => match self {
                Document(blocks)
                | BlockQuote(blocks, _)
                | List(blocks, _, _)
                | ListItem(blocks, _) => blocks.last_mut().unwrap().get_block_helper(x - 1),
                _ => unreachable!(),
            },
        }
    }

    // pub fn get_container(&mut self, next_offset: &Vec<usize>, last_is_leaf: bool) -> &mut Block {
    //     self.get_block(next_offset[..next_offset.len() - if last_is_leaf { 1 } else { 0 }])
    // }

    pub fn get_general_container(&mut self, open_block_depth: usize) -> (&mut Block, usize) {
        let mut new_depth: usize = 0;
        let mut seen_list: bool = false;
        let mut current_block: &Block = self;
        dbg!(open_block_depth);
        while new_depth < open_block_depth {
            match current_block {
                Document(blocks) => match blocks.last() {
                    // no extra depth here
                    None => break,
                    Some(b) => current_block = b,
                },
                BlockQuote(blocks, _) | ListItem(blocks, _) => {
                    if seen_list {
                        if new_depth + 2 > open_block_depth {
                            break;
                        }
                        seen_list = false;
                        new_depth += 1;
                    }
                    new_depth += 1;
                    match blocks.last() {
                        None => break,
                        Some(b) => current_block = b,
                    }
                }
                List(blocks, _, _) => {
                    seen_list = true;
                    current_block = blocks.last().unwrap();
                }
                _ => break,
            }
        }
        (self.get_block(dbg!(new_depth)), new_depth)
    }

    pub fn is_leaf(&self) -> bool {
        match self {
            Document(_) | BlockQuote(_, _) | List(_, _, _) | ListItem(_, _) => false,
            _ => true,
        }
    }

    pub fn is_general_block_appendable(&self) -> bool {
        match self {
            Document(_) | BlockQuote(_, true) | ListItem(_, _) => true,
            _ => false,
        }
    }

    pub fn get_last_block(&mut self) -> &mut Block {
        let descend = match self {
            Document(blocks) | BlockQuote(blocks, _) | List(blocks, _, _) | ListItem(blocks, _) => {
                !blocks.is_empty()
            }
            _ => false,
        };

        if !descend {
            return self;
        } else {
            match self {
                Document(blocks)
                | BlockQuote(blocks, _)
                | List(blocks, _, _)
                | ListItem(blocks, _) => {
                    return blocks.last_mut().unwrap().get_last_block();
                }
                _ => unreachable!("already bool checked earlier"),
            };
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ListType {
    OrderedList(char, usize),
    UnorderedList(char),
}

impl ListType {
    pub fn same_list_eq(&self, other: &Self) -> bool {
        match other {
            OrderedList(c, _) => match self {
                OrderedList(d, _) => *c == *d,
                _ => false,
            },
            UnorderedList(c) => match self {
                UnorderedList(d) => *c == *d,
                _ => false,
            },
        }
    }
}

#[test]
fn test_get_block_1() {
    let mut test_tree = Document(vec![BlockQuote(
        vec![List(
            vec![ListItem(vec![ThematicBreak], 2)],
            true,
            ListType::UnorderedList('*'),
        )],
        false,
    )]);
    let descension: usize = 4;
    assert_eq!(test_tree.get_block(descension), &mut ThematicBreak)
}

#[test]
fn test_get_block_2() {
    let mut test_tree = Document(vec![
        BlockQuote(
            vec![List(
                vec![ListItem(vec![ThematicBreak], 2)],
                true,
                ListType::UnorderedList('*'),
            )],
            false,
        ),
        BlockQuote(vec![Paragraph(vec![Text(vec!['p', 'o'])], true)], true),
    ]);
    let descension = 1;
    let bq = test_tree.get_block(descension);
    match bq {
        BlockQuote(v, _) => v.push(ThematicBreak),
        _ => unreachable!(),
    }
    assert_eq!(
        test_tree.get_block(descension),
        &mut BlockQuote(
            vec![Paragraph(vec![Text(vec!['p', 'o'])], true), ThematicBreak],
            true
        ),
    )
}

#[test]
fn test_get_last_block_1() {
    let mut test_tree = Document(vec![
        BlockQuote(
            vec![List(
                vec![ListItem(vec![ThematicBreak], 2)],
                true,
                ListType::UnorderedList('*'),
            )],
            false,
        ),
        BlockQuote(vec![Paragraph(vec![Text(vec!['p', 'o'])], true)], true),
    ]);
    assert_eq!(
        test_tree.get_last_block(),
        &mut Paragraph(vec![Text(vec!['p', 'o'])], true)
    )
}

#[test]
fn test_get_last_block_2() {
    let mut test_tree = Document(vec![]);
    assert_eq!(test_tree.get_last_block(), &mut Document(vec![]))
}

#[test]
fn test_get_last_general_container() {
    let ast = &mut Document(vec![BlockQuote(
        vec![List(
            vec![ListItem(vec![ThematicBreak], 2)],
            true,
            UnorderedList('*'),
        )],
        true,
    )]);
    let depth = 2; // matched the list but not list item
    assert_eq!(
        ast.get_general_container(depth),
        (
            &mut BlockQuote(
                vec![List(
                    vec![ListItem(vec![ThematicBreak], 2)],
                    true,
                    UnorderedList('*'),
                )],
                true,
            ),
            1
        )
    )
}
