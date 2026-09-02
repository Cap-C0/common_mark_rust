use core::fmt;
use std::option::Option::{None as Leaf, Some as Container};

use crate::{
    ast_types::{Block::*, ListType::*},
    chars::{push_chars_with_entities_and_bs, push_html_reserved_char},
    inline::{InlineContent, parse_inline},
    inline_str_collection::LeafContainerInline,
    lrd_table::{self, LRDTable},
    peekable_char_indices::BorrowedStringPCI,
};

#[derive(Debug, Copy, PartialEq, Eq, Clone)]
pub struct NodeId(usize);

//TODO: make a nice debug for this.
#[derive(PartialEq, Eq)]
pub struct AbstractSyntaxTree<T, I> {
    nodes: Vec<Node<T, I>>,
    head: NodeId,
}

//this is just an Option<Vec<NodeId>>
// #[derive(Debug, PartialEq, Eq)]
// enum BlockKind {
//     Leaf,
//     Container(Vec<NodeId>),
// }

type BlockKind = Option<Vec<NodeId>>;

#[derive(Debug, PartialEq, Eq, Clone)]
struct Node<T, I> {
    block: Option<Block<T, I>>,
    block_kind: BlockKind,
    depth: usize,
    parent_id: Option<NodeId>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Block<T, I> {
    Document,
    BlockQuote(bool),
    /// (tight, lt,)
    List(bool, ListType),
    /// (continuable, indent requirement)
    ListItem(bool, usize),
    Heading(I, usize),
    Paragraph(I, bool),
    ThematicBreak,
    /// actualy chars, unrealized blanks
    IndentedCodeBlock(T, T), // unrealized blank lines
    /// (contents, is_open, marking char, info_string, indend_count, tilde_count)
    FencedCodeBlock(T, bool, char, T, usize, usize),
    /// (characters,is_open, end_condition, )
    HTMLBlock(T, bool, HTMLEndCondition),
}

impl<T: fmt::Debug, I: fmt::Debug> fmt::Debug for AbstractSyntaxTree<T, I> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f)?;
        self.fmt_helper(f, self.head, 0)
    }
}

impl<T: fmt::Debug, I: fmt::Debug> AbstractSyntaxTree<T, I> {
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

impl<T, I> Default for AbstractSyntaxTree<T, I> {
    fn default() -> Self {
        Self::new()
    }
}
impl<T, I> AbstractSyntaxTree<T, I> {
    pub fn new() -> Self {
        AbstractSyntaxTree {
            nodes: vec![Node {
                block: Some(Document),
                block_kind: Container(vec![]),
                depth: 0,
                parent_id: None,
            }],
            head: NodeId(0),
        }
    }

    pub fn add_new_node(&mut self, parent: NodeId, block: Block<T, I>) -> NodeId {
        let parent_depth = self.nodes[parent.0].depth;
        let block_kind = match block {
            BlockQuote(..) | List(..) | ListItem(..) => Container(vec![]),
            Document => unreachable!(),
            _ => Leaf,
        };
        let child_id = NodeId(self.nodes.len());
        self.nodes.push(Node {
            block: Some(block),
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

    pub fn get_block(&mut self, node_id: NodeId) -> &mut Block<T, I> {
        if let Some(ref mut b_out) = self.nodes[node_id.0].block {
            b_out
        } else {
            panic!("Should not ask for already taken DLLnodeID")
        }
    }

    pub fn get_block_ref(&self, node_id: NodeId) -> &Block<T, I> {
        if let Some(ref b_out) = self.nodes[node_id.0].block {
            b_out
        } else {
            panic!("Should not ask for already taken DLLnodeID")
        }
    }
    pub fn take_block(&mut self, node_id: NodeId) -> Block<T, I> {
        self.nodes[node_id.0].block.take().unwrap()
    }

    pub fn replace_block(&mut self, node_id: NodeId, new_block: Block<T, I>) {
        self.nodes[node_id.0].block_kind = match new_block {
            BlockQuote(..) | List(..) | ListItem(..) => Container(vec![]),
            Document => unreachable!(),
            _ => Leaf,
        };
        self.nodes[node_id.0].block = Some(new_block);
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

impl<'a, T, I: 'a + LeafContainerInline> AbstractSyntaxTree<T, I> {
    pub fn parse_inlines(
        &self,
        lrd_table: &LRDTable<I>,
    ) -> AbstractSyntaxTree<T, Vec<InlineContent<I::Chars<'a>>>> {
        let out = vec![];
        for n in self.nodes {
            out.push(match n.block {
                Some(Paragraph(il, is_open)) => Node {
                    block: Some(Paragraph(parse_inline(il, lrd_table), is_open)),
                    block_kind: n.block_kind,
                    depth: n.depth,
                    parent_id: n.parent_id,
                },
                Some(Heading(il, size)) => Node {
                    block: Some(Heading(parse_inline(il, lrd_table), size)),
                    block_kind: n.block_kind,
                    depth: n.depth,
                    parent_id: n.parent_id,
                },
                b => n.clone(),
            });
        }
        AbstractSyntaxTree {
            nodes: out,
            head: self.head,
        };
    }
}

impl<'a, T: LeafContainerInline> AbstractSyntaxTree<T, Vec<InlineContent<T::Chars<'a>>>> {
    pub fn to_html(&self, lrd_table: &LRDTable<T>) -> String {
        dbg!("calling_to_html!");
        let mut str_out = String::new();
        self.to_html_helper(self.head, false, &mut str_out, lrd_table);
        str_out
    }

    pub fn to_html_helper(
        &self,
        current_node: NodeId,
        in_tight_list: bool,
        string_builder: &mut String,
        lrd_table: &LRDTable<T>,
    ) {
        let child_op = if let Container(ref children) = self.nodes[current_node.0].block_kind {
            Some(children)
        } else {
            None
        };
        match &self.nodes[current_node.0].block.unwrap() {
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
                    push_chars_with_entities_and_bs(lang_hint.as_pci(), string_builder, false);
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
                for c in string.as_pci() {
                    string_builder.push(c);
                }
                string_builder.push('\n');
            }
        }
    }
}

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
