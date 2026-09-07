use core::fmt;
use std::option::Option::{None as Leaf, Some as Container};

use crate::{
    ast_types::{Block::*, ListType::*},
    block_structure::LRDTable,
    chars::{push_chars_with_entities_and_bs, push_html_reserved_char},
    inline::LeafContainerInline,
    peekable_char_indices::BorrowedStringPCI,
};

#[derive(Debug, Copy, PartialEq, Eq, Clone)]
pub struct NodeId(usize);

//TODO: make a nice debug for this.
#[derive(PartialEq, Eq)]
pub struct AbstractSyntaxTree<T> {
    nodes: Vec<Node<T>>,
    head: NodeId,
}

//this is just an Option<Vec<NodeId>>
// #[derive(Debug, PartialEq, Eq)]
// enum BlockKind {
//     Leaf,
//     Container(Vec<NodeId>),
// }

type BlockKind = Option<Vec<NodeId>>;

#[derive(Debug, PartialEq, Eq)]
struct Node<T> {
    block: Block<T>,
    block_kind: BlockKind,
    depth: usize,
    parent_id: Option<NodeId>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Block<T> {
    Document,
    BlockQuote(bool),
    /// (tight, lt,)
    List(bool, ListType),
    /// (continuable, indent requirement)
    ListItem(bool, usize),
    Heading(T, usize),
    Paragraph(T, bool),
    ThematicBreak,
    /// actualy chars, unrealized blanks
    IndentedCodeBlock(T, T), // unrealized blank lines
    /// (contents, is_open, marking char, info_string, indend_count, tilde_count)
    FencedCodeBlock(T, bool, char, T, usize, usize),
    /// (characters,is_open, end_condition, )
    HTMLBlock(String, bool, HTMLEndCondition),
}

impl<T: fmt::Debug> fmt::Debug for AbstractSyntaxTree<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f)?;
        self.fmt_helper(f, self.head, 0)
    }
}

impl<T: fmt::Debug> AbstractSyntaxTree<T> {
    fn fmt_helper(
        &self,
        f: &mut fmt::Formatter<'_>,
        current_node_id: NodeId,
        depth: usize,
    ) -> fmt::Result {
        // let _child_op = if let Container(ref children) = self.nodes[current_node_id.0].block_kind {
        //     Some(children)
        // } else {
        //     None
        // };
        let indent = "  ".repeat(depth);
        writeln!(
            f,
            "{}- [{:?}] {:?}",
            indent, current_node_id, self.nodes[current_node_id.0].block
        )?;

        if let Container(ref children_id) = self.nodes[current_node_id.0].block_kind {
            for child_id in children_id {
                self.fmt_helper(f, *child_id, depth + 1)?;
            }
        }

        Ok(())
    }
}

impl<T: LeafContainerInline> Default for AbstractSyntaxTree<T> {
    fn default() -> Self {
        Self::new()
    }
}
impl<T: LeafContainerInline> AbstractSyntaxTree<T> {
    pub fn new() -> Self {
        AbstractSyntaxTree {
            nodes: vec![Node {
                block: Document,
                block_kind: Container(vec![]),
                depth: 0,
                parent_id: None,
            }],
            head: NodeId(0),
        }
    }

    pub fn add_new_node(&mut self, parent: NodeId, block: Block<T>) -> NodeId {
        let parent_depth = self.nodes[parent.0].depth;
        let block_kind = match block {
            BlockQuote(..) | List(..) | ListItem(..) => Container(vec![]),
            Document => unreachable!(),
            _ => Leaf,
        };
        let child_id = NodeId(self.nodes.len());
        self.nodes.push(Node {
            block,
            block_kind,
            depth: parent_depth + 1,
            parent_id: Some(parent),
        });
        let children = self.nodes[parent.0].block_kind.as_mut().unwrap();
        children.push(child_id);
        child_id
    }

    pub fn get_last_child_id(&self, parent: NodeId) -> Option<NodeId> {
        self.nodes[parent.0].block_kind.as_ref()?.last().copied()
        // match self.nodes[parent.0].block_kind {
        //     Leaf => None,
        //     Container(ref child_ids) => child_ids.last().copied(),
        // }
    }

    pub fn get_block(&mut self, node_id: NodeId) -> &mut Block<T> {
        &mut self.nodes[node_id.0].block
    }

    pub fn get_block_ref(&self, node_id: NodeId) -> &Block<T> {
        &self.nodes[node_id.0].block
    }

    pub fn replace_block(&mut self, node_id: NodeId, new_block: Block<T>) {
        self.nodes[node_id.0].block_kind = match new_block {
            BlockQuote(..) | List(..) | ListItem(..) => Container(vec![]),
            Document => unreachable!(),
            _ => Leaf,
        };
        self.nodes[node_id.0].block = new_block;
    }

    pub fn get_block_depth(&self, node_id: NodeId) -> usize {
        self.nodes[node_id.0].depth
    }

    pub fn get_head_id(&self) -> NodeId {
        self.head
    }

    pub fn get_parent_id(&self, node_id: NodeId) -> Option<NodeId> {
        self.nodes[node_id.0].parent_id
    }

    pub fn has_no_children(&self, node_id: NodeId) -> bool {
        match self.nodes[node_id.0].block_kind {
            Leaf => true,
            Container(ref node_ids) => node_ids.is_empty(),
        }
    }
}
impl AbstractSyntaxTree<String> {
    pub fn to_html(&self, lrd_table: &LRDTable) -> String {
        ("calling_to_html!");
        let mut str_out = String::new();
        self.to_html_helper(self.head, false, &mut str_out, lrd_table);
        str_out
    }

    pub fn to_html_helper(
        &self,
        current_node: NodeId,
        in_tight_list: bool,
        string_builder: &mut String,
        lrd_table: &LRDTable,
    ) {
        let child_op = if let Container(ref children) = self.nodes[current_node.0].block_kind {
            Some(children)
        } else {
            None
        };
        match &self.nodes[current_node.0].block {
            Document => {
                for node_id in child_op.unwrap() {
                    self.to_html_helper(*node_id, false, string_builder, lrd_table);
                }
            }
            BlockQuote(_) => {
                if !string_builder.is_empty() && !string_builder.ends_with('\n') {
                    string_builder.push('\n');
                }
                string_builder.push_str("<blockquote>\n");
                for node_id in child_op.unwrap() {
                    self.to_html_helper(*node_id, false, string_builder, lrd_table);
                }
                string_builder.push_str("</blockquote>\n");
            }
            List(is_tight, list_type) => {
                if !string_builder.is_empty() && !string_builder.ends_with('\n') {
                    string_builder.push('\n');
                }
                match list_type {
                    OrderedList(_, n) => {
                        if *n != 1 {
                            string_builder.push_str(&format!("<ol start=\"{}\">\n", n));
                        } else {
                            string_builder.push_str("<ol>\n");
                        }
                        for node_id in child_op.unwrap() {
                            self.to_html_helper(*node_id, *is_tight, string_builder, lrd_table);
                        }
                        string_builder.push_str("</ol>\n");
                    }
                    UnorderedList(_) => {
                        string_builder.push_str("<ul>\n");
                        for node_id in child_op.unwrap() {
                            self.to_html_helper(*node_id, *is_tight, string_builder, lrd_table);
                        }
                        string_builder.push_str("</ul>\n");
                    }
                }
            }
            ListItem(..) => {
                string_builder.push_str("<li>");
                for node_id in child_op.unwrap() {
                    self.to_html_helper(*node_id, in_tight_list, string_builder, lrd_table);
                }
                string_builder.push_str("</li>\n");
            }
            Heading(il, h) => {
                if !string_builder.is_empty() && !string_builder.ends_with('\n') {
                    string_builder.push('\n');
                }
                string_builder.push_str(&format!("<h{}>", h));
                il.to_html(string_builder, lrd_table);
                string_builder.push_str(&format!("</h{}>\n", h));
            }
            Paragraph(il, _) => {
                if !in_tight_list && !il.is_empty() {
                    if !string_builder.is_empty() && !string_builder.ends_with('\n') {
                        string_builder.push('\n');
                    }
                    string_builder.push_str("<p>");
                }
                il.to_html(string_builder, lrd_table);
                if !in_tight_list && !il.is_empty() {
                    string_builder.push_str("</p>\n");
                }
            }
            ThematicBreak => {
                if !string_builder.is_empty() && !string_builder.ends_with('\n') {
                    string_builder.push('\n');
                }
                string_builder.push_str("<hr />\n")
            }
            IndentedCodeBlock(string, _items1) => {
                if !string_builder.is_empty() && !string_builder.ends_with('\n') {
                    string_builder.push('\n');
                }
                string_builder.push_str("<pre><code>");
                for c in string.char_iter() {
                    push_html_reserved_char(c, string_builder);
                }
                string_builder.push_str("\n</code></pre>\n");
            }
            FencedCodeBlock(string, _, _, lang_hint, _, _) => {
                if !string_builder.is_empty() && !string_builder.ends_with('\n') {
                    string_builder.push('\n');
                }
                string_builder.push_str("<pre><code");
                if !lang_hint.is_empty() {
                    string_builder.push_str(" class=\"language-");
                    push_chars_with_entities_and_bs(
                        &BorrowedStringPCI::new(lang_hint),
                        string_builder,
                        false,
                    );
                    string_builder.push('\"');
                }
                string_builder.push('>');
                for c in string.char_iter() {
                    push_html_reserved_char(c, string_builder);
                }
                string_builder.push_str("</code></pre>\n");
            }
            HTMLBlock(string, ..) => {
                if !string_builder.is_empty() && !string_builder.ends_with('\n') {
                    string_builder.push('\n');
                }
                for c in string.chars() {
                    string_builder.push(c);
                }
                string_builder.push('\n');
            }
        }
    }
}

//TODO: put this in an Arena!

// #[derive(Debug, PartialEq, Eq, Clone)]
// pub enum Block {
//     Document(Vec<Block>),
//     BlockQuote(Vec<Block>, bool),
//     /// (children, tight, lt, blank_line_encountered)
//     List(Vec<Block>, bool, ListType, bool),
//     /// (children, continuable, indent requirement)
//     ListItem(Vec<Block>, bool, usize),
//     Heading(Inline, usize),
//     Paragraph(Inline, bool),
//     ThematicBreak,
//     /// actualy chars, unrealized blanks
//     IndentedCodeBlock(String, String), // unrealized blank lines
//     /// (contents, is_open, marking char, info_string, indend_count, tilde_count)
//     FencedCodeBlock(String, bool, char, String, usize, usize),
//     /// (characters,is_open, end_condition, )
//     HTMLBlock(String, bool, HTMLEndCondition),
// }
//
// impl Block {
//     pub fn get_block(&mut self, open_block_depth: usize) -> &mut Block {
//         self.get_block_helper(open_block_depth)
//     }
//
//     fn get_block_helper(&mut self, open_block_depth: usize) -> &mut Block {
//         match open_block_depth {
//             0 => self,
//             x => match self {
//                 Document(blocks)
//                 | BlockQuote(blocks, _)
//                 | List(blocks, ..)
//                 | ListItem(blocks, ..) => blocks.last_mut().unwrap().get_block_helper(x - 1),
//                 _ => unreachable!(),
//             },
//         }
//     }
//
//     // pub fn get_container(&mut self, next_offset: &Vec<usize>, last_is_leaf: bool) -> &mut Block {
//     //     self.get_block(next_offset[..next_offset.len() - if last_is_leaf { 1 } else { 0 }])
//     // }
//
//     pub fn get_general_container(&mut self, open_block_depth: usize) -> (&mut Block, usize) {
//         let mut new_depth: usize = 0;
//         let mut seen_list: bool = false;
//         let mut current_block: &Block = self;
//         while new_depth < open_block_depth {
//             match current_block {
//                 Document(blocks) => match blocks.last() {
//                     // no extra depth here
//                     None => break,
//                     Some(b) => current_block = b,
//                 },
//                 BlockQuote(blocks, _) | ListItem(blocks, _, _) => {
//                     if seen_list {
//                         if new_depth + 2 > open_block_depth {
//                             break;
//                         }
//                         seen_list = false;
//                         new_depth += 1;
//                     }
//                     new_depth += 1;
//                     match blocks.last() {
//                         None => break,
//                         Some(b) => current_block = b,
//                     }
//                 }
//                 List(blocks, ..) => {
//                     seen_list = true;
//                     current_block = blocks.last().unwrap();
//                 }
//                 _ => break,
//             }
//         }
//         (self.get_block(new_depth), new_depth)
//     }
//
//     // pub fn is_leaf(&self) -> bool {
//     //     match self {
//     //         Document(_) | BlockQuote(..) | List(..) | ListItem(..) => false,
//     //         _ => true,
//     //     }
//     // }
//
//     // pub fn parse_inlines(&mut self, lrd_table: &LRDTable) {
//     //     match self {
//     //         Document(blocks) | BlockQuote(blocks, _) | List(blocks, ..) | ListItem(blocks, ..) => {
//     //             for b in blocks {
//     //                 b.parse_inlines(lrd_table);
//     //             }
//     //         }
//     //         Heading(il, _) | Paragraph(il, ..) => {
//     //             il.fill_content(lrd_table);
//     //         }
//     //         _ => (),
//     //     }
//     // }
//
//     pub fn deepest_matched_blockquote(&mut self, max_depth: usize) -> usize {
//         let mut current_depth = 0;
//         let mut last_blockquote_depth = 0;
//         let mut current_block: &Block = self;
//         while current_depth < max_depth {
//             match current_block {
//                 Document(blocks) | List(blocks, ..) | ListItem(blocks, ..) => {
//                     current_depth += 1;
//                     if !blocks.is_empty() {
//                         current_block = blocks.last().unwrap()
//                     } else {
//                         break;
//                     }
//                 }
//                 BlockQuote(blocks, _) => {
//                     current_depth += 1;
//                     last_blockquote_depth = current_depth;
//                     if !blocks.is_empty() {
//                         current_block = blocks.last().unwrap()
//                     } else {
//                         break;
//                     }
//                 }
//                 _ => break,
//             }
//         }
//         last_blockquote_depth
//     }
//
//     pub fn close_open_block(&mut self, deeper_than: i32) {
//         match self {
//             Document(blocks) | List(blocks, ..) | ListItem(blocks, ..) => {
//                 if !blocks.is_empty() {
//                     blocks.last_mut().unwrap().close_open_block(deeper_than - 1)
//                 }
//             }
//             BlockQuote(blocks, is_open) => {
//                 if deeper_than < 0 {
//                     *is_open = false
//                 } else {
//                     if !blocks.is_empty() {
//                         blocks.last_mut().unwrap().close_open_block(deeper_than - 1)
//                     }
//                 }
//             }
//             _ => (),
//         }
//     }
//
//     // pub fn is_general_block_appendable(&self) -> bool {
//     //     match self {
//     //         Document(_) | BlockQuote(_, true) | ListItem(_, _, _) => true,
//     //         _ => false,
//     //     }
//     // }
//
//     // pub fn reset_blank_line_seen(&mut self, depth: usize) {
//     //     if depth > 0 {
//     //         match self {
//     //             Document(blocks) | BlockQuote(blocks, _) | ListItem(blocks, _, _) => {
//     //                 if blocks.is_empty() {
//     //                     return;
//     //                 }
//     //                 blocks.last_mut().unwrap().reset_blank_line_seen(depth - 1);
//     //             }
//     //             List(blocks, _, _, blank_line_encountered) => {
//     //                 *blank_line_encountered = false;
//     //                 if blocks.is_empty() {
//     //                     return;
//     //                 }
//     //                 blocks.last_mut().unwrap().reset_blank_line_seen(depth - 1);
//     //             }
//     //             _ => return,
//     //         }
//     //     }
//     // }
//
//     // fn detighten_deeper_than(&mut self, depth: i32) {
//     //     match self {
//     //         Document(blocks) | BlockQuote(blocks, _) | ListItem(blocks, _, _) => {
//     //             if blocks.is_empty() {
//     //                 return;
//     //             }
//     //             blocks.last_mut().unwrap().detighten_deeper_than(depth - 1);
//     //         }
//     //         List(blocks, _, _, blank_line_encountered) => {
//     //             if depth <= 0 {
//     //                 *blank_line_encountered = true
//     //             }
//     //             if blocks.is_empty() {
//     //                 return;
//     //             }
//     //             blocks.last_mut().unwrap().detighten_deeper_than(depth - 1);
//     //         }
//     //         _ => return,
//     //     }
//     // }
//
//     pub fn get_last_block(&mut self) -> &mut Block {
//         let descend = match self {
//             Document(blocks)
//             | BlockQuote(blocks, _)
//             | List(blocks, _, _, _)
//             | ListItem(blocks, _, _) => !blocks.is_empty(),
//             _ => false,
//         };
//
//         if !descend {
//             self
//         } else {
//             match self {
//                 Document(blocks)
//                 | BlockQuote(blocks, _)
//                 | List(blocks, _, _, _)
//                 | ListItem(blocks, _, _) => blocks.last_mut().unwrap().get_last_block(),
//                 _ => unreachable!("already bool checked earlier"),
//             }
//         }
//     }
//
//     pub fn to_html(&self, lrd_table: &LRDTable) -> String {
//         let mut str_out = String::new();
//         self.to_html_helper(false, &mut str_out, lrd_table);
//         str_out
//     }
//
//     //TODO: find a more elegant way of doing this \n business
//
//     fn to_html_helper(
//         &self,
//         in_tight_list: bool,
//         string_builder: &mut String,
//         lrd_table: &LRDTable,
//     ) {
//         match self {
//             Document(blocks) => {
//                 for b in blocks {
//                     b.to_html_helper(false, string_builder, lrd_table);
//                 }
//             }
//             BlockQuote(blocks, _) => {
//                 if !string_builder.is_empty() && !string_builder.ends_with('\n') {
//                     string_builder.push('\n');
//                 }
//                 string_builder.push_str("<blockquote>\n");
//                 for b in blocks {
//                     b.to_html_helper(false, string_builder, lrd_table);
//                 }
//                 string_builder.push_str("</blockquote>\n");
//             }
//             List(blocks, is_tight, list_type, _) => {
//                 if !string_builder.is_empty() && !string_builder.ends_with('\n') {
//                     string_builder.push('\n');
//                 }
//                 match list_type {
//                     OrderedList(_, n) => {
//                         if *n != 1 {
//                             string_builder.push_str(&format!("<ol start=\"{}\">\n", n));
//                         } else {
//                             string_builder.push_str("<ol>\n");
//                         }
//                         for b in blocks {
//                             b.to_html_helper(*is_tight, string_builder, lrd_table);
//                         }
//                         string_builder.push_str("</ol>\n");
//                     }
//                     UnorderedList(_) => {
//                         string_builder.push_str("<ul>\n");
//                         for b in blocks {
//                             b.to_html_helper(*is_tight, string_builder, lrd_table);
//                         }
//                         string_builder.push_str("</ul>\n");
//                     }
//                 }
//             }
//             ListItem(blocks, ..) => {
//                 string_builder.push_str("<li>");
//                 for b in blocks {
//                     b.to_html_helper(in_tight_list, string_builder, lrd_table);
//                 }
//                 string_builder.push_str("</li>\n");
//             }
//             Heading(il, h) => {
//                 if !string_builder.is_empty() && !string_builder.ends_with('\n') {
//                     string_builder.push('\n');
//                 }
//                 string_builder.push_str(&format!("<h{}>", h));
//                 il.to_html(string_builder, lrd_table);
//                 string_builder.push_str(&format!("</h{}>\n", h));
//             }
//             Paragraph(il, _) => {
//                 if !in_tight_list && !il.string.is_empty() {
//                     if !string_builder.is_empty() && !string_builder.ends_with('\n') {
//                         string_builder.push('\n');
//                     }
//                     string_builder.push_str("<p>");
//                 }
//                 il.to_html(string_builder, lrd_table);
//                 if !in_tight_list && !il.string.is_empty() {
//                     string_builder.push_str("</p>\n");
//                 }
//             }
//             ThematicBreak => {
//                 if !string_builder.is_empty() && !string_builder.ends_with('\n') {
//                     string_builder.push('\n');
//                 }
//                 string_builder.push_str("<hr />\n")
//             }
//             IndentedCodeBlock(string, _items1) => {
//                 if !string_builder.is_empty() && !string_builder.ends_with('\n') {
//                     string_builder.push('\n');
//                 }
//                 string_builder.push_str("<pre><code>");
//                 for c in string.chars() {
//                     push_html_reserved_char(c, string_builder);
//                 }
//                 string_builder.push_str("\n</code></pre>\n");
//             }
//             FencedCodeBlock(string, _, _, lang_hint, _, _) => {
//                 if !string_builder.is_empty() && !string_builder.ends_with('\n') {
//                     string_builder.push('\n');
//                 }
//                 string_builder.push_str("<pre><code");
//                 if !lang_hint.is_empty() {
//                     string_builder.push_str(" class=\"language-");
//                     push_chars_with_entities_and_bs(lang_hint, string_builder, false);
//                     string_builder.push('\"');
//                 }
//                 string_builder.push('>');
//                 for c in string.chars() {
//                     push_html_reserved_char(c, string_builder);
//                 }
//                 string_builder.push_str("</code></pre>\n");
//             }
//             HTMLBlock(string, ..) => {
//                 if !string_builder.is_empty() && !string_builder.ends_with('\n') {
//                     string_builder.push('\n');
//                 }
//                 for c in string.chars() {
//                     string_builder.push(c);
//                 }
//                 string_builder.push('\n');
//             }
//         }
//     }
// }

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ListType {
    OrderedList(char, u32),
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

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum HTMLEndCondition {
    ContainsStrings(Vec<String>),
    BlankLine,
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     #[test]
//     fn test_get_block_1() {
//         let mut test_tree = Document(vec![BlockQuote(
//             vec![List(
//                 vec![ListItem(vec![ThematicBreak], true, 2)],
//                 true,
//                 ListType::UnorderedList('*'),
//                 false,
//             )],
//             false,
//         )]);
//         let descension: usize = 4;
//         assert_eq!(test_tree.get_block(descension), &mut ThematicBreak)
//     }
//
//     #[test]
//     fn test_get_block_2() {
//         let mut test_tree = Document(vec![
//             BlockQuote(
//                 vec![List(
//                     vec![ListItem(vec![ThematicBreak], true, 2)],
//                     true,
//                     ListType::UnorderedList('*'),
//                     false,
//                 )],
//                 false,
//             ),
//             BlockQuote(vec![Paragraph(Inline::new(vec!['p', 'o']), true)], true),
//         ]);
//         let descension = 1;
//         let bq = test_tree.get_block(descension);
//         match bq {
//             BlockQuote(v, _) => v.push(ThematicBreak),
//             _ => unreachable!(),
//         }
//         assert_eq!(
//             test_tree.get_block(descension),
//             &mut BlockQuote(
//                 vec![Paragraph(Inline::new(vec!['p', 'o']), true), ThematicBreak],
//                 true
//             ),
//         )
//     }
//
//     #[test]
//     fn test_get_last_block_1() {
//         let mut test_tree = Document(vec![
//             BlockQuote(
//                 vec![List(
//                     vec![ListItem(vec![ThematicBreak], true, 2)],
//                     true,
//                     ListType::UnorderedList('*'),
//                     false,
//                 )],
//                 false,
//             ),
//             BlockQuote(vec![Paragraph(Inline::new(vec!['p', 'o']), true)], true),
//         ]);
//         assert_eq!(
//             test_tree.get_last_block(),
//             &mut Paragraph(Inline::new(vec!['p', 'o']), true)
//         )
//     }
//
//     #[test]
//     fn test_get_last_block_2() {
//         let mut test_tree = Document(vec![]);
//         assert_eq!(test_tree.get_last_block(), &mut Document(vec![]))
//     }
//
//     #[test]
//     fn test_get_last_general_container() {
//         let ast = &mut Document(vec![BlockQuote(
//             vec![List(
//                 vec![ListItem(vec![ThematicBreak], true, 2)],
//                 true,
//                 UnorderedList('*'),
//                 false,
//             )],
//             true,
//         )]);
//         let depth = 2; // matched the list but not list item
//         assert_eq!(
//             ast.get_general_container(depth),
//             (
//                 &mut BlockQuote(
//                     vec![List(
//                         vec![ListItem(vec![ThematicBreak], true, 2)],
//                         true,
//                         UnorderedList('*'),
//                         false,
//                     )],
//                     true,
//                 ),
//                 1
//             )
//         )
//     }
// }
