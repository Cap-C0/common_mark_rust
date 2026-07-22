use serde::de;

use crate::ast_types::Block::{BlockQuote, Document, List, ListItem, Paragraph, ThematicBreak};

type Inline = Vec<char>;

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
    pub fn get_block(&mut self, next_offset_iter: &mut dyn Iterator<Item = &usize>) -> &mut Block {
        match next_offset_iter.next() {
            None => self,
            Some(offset) => match self {
                Block::Document(blocks) => {
                    blocks.get_mut(*offset).unwrap().get_block(next_offset_iter)
                }
                Block::BlockQuote(blocks, _) => {
                    blocks.get_mut(*offset).unwrap().get_block(next_offset_iter)
                }
                Block::List(items, _, _) => {
                    items.get_mut(*offset).unwrap().get_block(next_offset_iter)
                }
                Block::ListItem(blocks, _) => {
                    blocks.get_mut(*offset).unwrap().get_block(next_offset_iter)
                }
                _ => unreachable!(),
            },
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
    assert_eq!(
        test_tree.get_block(&mut descension.iter()),
        &mut ThematicBreak
    )
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
    let bq = test_tree.get_block(&mut descension.iter());
    match bq {
        BlockQuote(v, _) => v.push(ThematicBreak),
        _ => unreachable!(),
    }
    assert_eq!(
        test_tree.get_block(&mut descension.iter()),
        &mut BlockQuote(vec![Paragraph(vec!['p', 'o'], true), ThematicBreak], true),
    )
}
//
// pub trait Block {
//     fn to_html(&self) -> String {
//         String::from("")
//     }
//
//     //use vec of chars instead of string for better unicode performance
//     fn continuation_condition(&self, line: &Vec<char>, offset: &mut usize) -> bool {
//         false
//     }
//
//     fn is_closed(&mut self) -> bool {
//         false
//     }
//
//     fn as_block(&self) -> &dyn Block;
// }
//
// pub trait Container<'a> {
//     fn get_last_child(&'a mut self) -> Option<&'static mut (dyn Block + 'a)>;
// }
//
// pub struct Document<'a> {
//     pub children: Vec<Box<dyn Block + 'a>>,
// }
//
// impl<'a> Block for Document<'a> {
//     fn continuation_condition(&self, line: &Vec<char>, offset: &mut usize) -> bool {
//         true // Document doesnt stop being valid til the end
//     }
//
//     fn to_html(&self) -> String {
//         self.children.iter().map(|child| child.to_html()).collect()
//     }
//
//     fn as_block(&self) -> &dyn Block {
//         self
//     }
// }
//
// impl<'a> Container<'a> for Document<'a> {
//     fn get_last_child(&'a mut self) -> Option<&'static mut (dyn Block + 'a)> {
//         self.children.last_mut().map(|b| b.as_mut())
//     }
// }
//
// pub struct BlockQuote<'a> {
//     pub children: Vec<Box<dyn Block + 'a>>,
// }
//
// impl<'a> Block for BlockQuote<'a> {
//     fn continuation_condition(&self, line: &Vec<char>, offset: &mut usize) -> bool {
//         for i in 0..3 {
//             if line.len() >= *offset + i {
//                 return false;
//             }
//             if line[*offset + i] == '>' {
//                 //bounds check
//                 if line.len() >= *offset + i + 1 {
//                     *offset += i + 1;
//                     return true;
//                 }
//                 if line[*offset + i + 1] == ' ' {
//                     *offset += i + 2;
//                 } else {
//                     *offset += i + 1;
//                 }
//                 return true;
//             }
//             if line[*offset + i] != ' ' {
//                 return false;
//             }
//         }
//         false
//     }
//
//     fn to_html(&self) -> String {
//         let mut out = "<blockquote>\n".to_string();
//         let child: String = self.children.iter().map(|child| child.to_html()).collect();
//         out.push_str(&child);
//         out.push_str("\n<blockquote>");
//         out
//     }
//
//     fn as_block(&self) -> &dyn Block {
//         self
//     }
// }
//
// impl<'a> Container<'a> for BlockQuote<'a> {
//     fn get_last_child(&'a mut self) -> Option<&mut (dyn Block + 'a)> {
//         self.children.last_mut().map(|b| b.as_mut())
//     }
// }
//
// pub struct ListItem<'a> {
//     pub children: Vec<Box<dyn Block + 'a>>,
//     pub indent_amount: usize,
// }
//
// impl<'a> ListItem<'a> {
//     fn to_html(&self, is_loose: bool) -> String {
//         "".to_string()
//     }
// }
//
// impl<'a> Block for ListItem<'a> {
//     fn continuation_condition(&self, line: &Vec<char>, offset: &mut usize) -> bool {
//         if line.len() - *offset > self.indent_amount {
//             for i in *offset..(*offset + self.indent_amount) {
//                 if line[i] != ' ' {
//                     return false;
//                 }
//             }
//             *offset += self.indent_amount;
//             return true;
//         }
//         false
//     }
//
//     fn as_block(&self) -> &dyn Block {
//         self
//     }
// }
//
// impl<'a> Container<'a> for ListItem<'a> {
//     fn get_last_child(&'a mut self) -> Option<&'a mut dyn Block> {
//         self.children.last_mut().map(move |b| b.as_mut())
//     }
// }
//
// pub enum ListType {
//     OrderedList(char, usize),
//     UnorderedList(char),
// }
//
// pub struct List<'a> {
//     pub children: Vec<ListItem<'a>>,
//     pub list_type: ListType,
//     pub is_loose: bool,
// }
//
// impl<'a> Block for List<'a> {
//     fn continuation_condition(&self, line: &Vec<char>, offset: &mut usize) -> bool {
//         // first we ask child if it can continue.
//         if self
//             .children
//             .last()
//             .unwrap()
//             .continuation_condition(line, offset)
//         {
//             return true;
//         }
//         false
//     }
// }
//
// impl<'a> Container<'a> for List<'a> {
//     fn get_last_child(&'a mut self) -> Option<&mut (dyn Block + 'a)> {
//         self.children.last_mut().map(|a| a as &mut dyn Block)
//     }
// }
//
// pub struct ThematicBreak {}
//
// impl Block for ThematicBreak {
//     fn to_html(&self) -> String {
//         String::from("<hr \\>")
//     }
//
//     fn as_block(&self) -> &dyn Block {
//         self
//     }
// }
//
// pub struct ATXHeading {
//     text: String,
//     level: usize,
// }
//
// impl Block for ATXHeading {
//     fn to_html(&self) -> String {
//         format!("<h{}>{}<h{}\\>", self.level, self.text, self.level)
//     }
// }
//
// pub struct SetextHeading {
//     text: String,
//     level: usize,
// }
//
// impl Block for SetextHeading {
//     fn to_html(&self) -> String {
//         format!("<h{}>{}<h{}\\>", self.level, self.text, self.level)
//     }
// }
//
// pub struct IndentedCodeBlock {}
// pub struct FencedCodeBlock {}
// pub struct HTMLBlock {}
// pub struct LinkReferenceDefinition {}
// pub struct Paragraphs {}
// pub struct BlankLine {}
