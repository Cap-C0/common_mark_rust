use std::ops::ControlFlow::Break;

use crate::ast_types::Block::*;

pub type Inline = Vec<char>;

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
    IndentedCodeBlock(Inline),
    FencedCodeBlock(Inline, char, usize, usize),
    HTMLBlock(Inline),
}

impl Block {
    pub fn get_block(&mut self, next_offset: &Vec<usize>) -> &mut Block {
        self.get_block_helper(&mut next_offset.iter())
    }
    fn get_block_helper(
        &mut self,
        next_offset_iter: &mut dyn Iterator<Item = &usize>,
    ) -> &mut Block {
        match next_offset_iter.next() {
            None => self,
            Some(offset) => match self {
                Document(blocks)
                | BlockQuote(blocks, _)
                | List(blocks, _, _)
                | ListItem(blocks, _) => blocks
                    .get_mut(*offset)
                    .unwrap()
                    .get_block_helper(next_offset_iter),
                _ => unreachable!(),
            },
        }
    }

    // pub fn get_container(&mut self, next_offset: &Vec<usize>, last_is_leaf: bool) -> &mut Block {
    //     self.get_block(next_offset[..next_offset.len() - if last_is_leaf { 1 } else { 0 }])
    // }

    pub fn get_general_container(&mut self, next_offset: &Vec<usize>) -> &mut Block {
        let mut new_offsets: Vec<usize> = vec![];
        let mut prev_block_offset = 0;
        let mut list_item_offset: Option<usize> = None;
        let current_block = &self;
        for o in next_offset {
            match current_block {
                Document(blocks) | BlockQuote(blocks, _) | ListItem(blocks, _) => {
                    match list_item_offset {
                        Some(x) => {
                            new_offsets.push(x);
                            list_item_offset = None;
                            new_offsets.push(*o);
                        }
                        None => {}
                    }
                }
                List(blocks, _, _) => list_item_offset = Some(*o),
                _ => break,
            }
        }
        self.get_block(&new_offsets)
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

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum ListType {
    OrderedList(char, usize),
    UnorderedList(char),
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
    let descension: Vec<usize> = vec![0, 0, 0, 0];
    let des_iter = descension.iter();
    assert_eq!(test_tree.get_block(&descension), &mut ThematicBreak)
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
        BlockQuote(vec![Paragraph(vec!['p', 'o'], true)], true),
    ]);
    let descension: Vec<usize> = vec![1];
    let bq = test_tree.get_block(&descension);
    match bq {
        BlockQuote(v, _) => v.push(ThematicBreak),
        _ => unreachable!(),
    }
    assert_eq!(
        test_tree.get_block(&descension),
        &mut BlockQuote(vec![Paragraph(vec!['p', 'o'], true), ThematicBreak], true),
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
        BlockQuote(vec![Paragraph(vec!['p', 'o'], true)], true),
    ]);
    assert_eq!(
        test_tree.get_last_block(),
        &mut Paragraph(vec!['p', 'o'], true)
    )
}

#[test]
fn test_get_last_block_2() {
    let mut test_tree = Document(vec![]);
    assert_eq!(test_tree.get_last_block(), &mut Document(vec![]))
}
