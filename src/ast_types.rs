use crate::ast_types::{Block::*, ListType::*};

pub type Inline = Vec<char>;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Block {
    Document(Vec<Block>),
    BlockQuote(Vec<Block>, bool),
    /// (children, tight, lt, blank_line_encountered)
    List(Vec<Block>, bool, ListType, bool),
    /// (children, continuable, indent requirement)
    ListItem(Vec<Block>, bool, usize),
    Heading(Inline, usize),
    Paragraph(Inline, bool),
    ThematicBreak,
    ///
    IndentedCodeBlock(Vec<char>, Vec<Vec<char>>), // unrealized blank lines
    /// (contents, is_open, marking char, info_string, indend_count, tilde_count)
    FencedCodeBlock(Vec<char>, bool, char, Vec<char>, usize, usize),
    /// (characters,is_open, end_condition, )
    HTMLBlock(Inline, bool, HTMLEndCondition),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum HTMLEndCondition {
    ContainsStrings(Vec<Vec<char>>),
    BlankLine,
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
                | List(blocks, ..)
                | ListItem(blocks, ..) => blocks.last_mut().unwrap().get_block_helper(x - 1),
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
        while new_depth < open_block_depth {
            match current_block {
                Document(blocks) => match blocks.last() {
                    // no extra depth here
                    None => break,
                    Some(b) => current_block = b,
                },
                BlockQuote(blocks, _) | ListItem(blocks, _, _) => {
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
                List(blocks, ..) => {
                    seen_list = true;
                    current_block = blocks.last().unwrap();
                }
                _ => break,
            }
        }
        (self.get_block(new_depth), new_depth)
    }

    pub fn is_leaf(&self) -> bool {
        match self {
            Document(_) | BlockQuote(..) | List(..) | ListItem(..) => false,
            _ => true,
        }
    }

    pub fn deepest_matched_blockquote(&mut self, max_depth: usize) -> usize {
        let mut current_depth = 0;
        let mut last_blockquote_depth = 0;
        let mut current_block: &Block = self;
        while current_depth < max_depth {
            match current_block {
                Document(blocks) | List(blocks, ..) | ListItem(blocks, ..) => {
                    current_depth += 1;
                    if !blocks.is_empty() {
                        current_block = blocks.last().unwrap()
                    } else {
                        break;
                    }
                }
                BlockQuote(blocks, _) => {
                    current_depth += 1;
                    last_blockquote_depth = current_depth;
                    if !blocks.is_empty() {
                        current_block = blocks.last().unwrap()
                    } else {
                        break;
                    }
                }
                _ => break,
            }
        }
        last_blockquote_depth
    }

    pub fn close_open_block(&mut self, deeper_than: i32) {
        match self {
            Document(blocks) | List(blocks, ..) | ListItem(blocks, ..) => {
                if !blocks.is_empty() {
                    blocks.last_mut().unwrap().close_open_block(deeper_than - 1)
                }
            }
            BlockQuote(blocks, is_open) => {
                if deeper_than < 0 {
                    *is_open = false
                } else {
                    if !blocks.is_empty() {
                        blocks.last_mut().unwrap().close_open_block(deeper_than - 1)
                    }
                }
            }
            _ => (),
        }
    }

    pub fn is_general_block_appendable(&self) -> bool {
        match self {
            Document(_) | BlockQuote(_, true) | ListItem(_, _, _) => true,
            _ => false,
        }
    }

    pub fn reset_blank_line_seen(&mut self, depth: usize) {
        if depth > 0 {
            match self {
                Document(blocks) | BlockQuote(blocks, _) | ListItem(blocks, _, _) => {
                    if blocks.is_empty() {
                        return;
                    }
                    blocks.last_mut().unwrap().reset_blank_line_seen(depth - 1);
                }
                List(blocks, _, _, blank_line_encountered) => {
                    *blank_line_encountered = false;
                    if blocks.is_empty() {
                        return;
                    }
                    blocks.last_mut().unwrap().reset_blank_line_seen(depth - 1);
                }
                _ => return,
            }
        }
    }

    fn detighten_deeper_than(&mut self, depth: i32) {
        match self {
            Document(blocks) | BlockQuote(blocks, _) | ListItem(blocks, _, _) => {
                if blocks.is_empty() {
                    return;
                }
                blocks.last_mut().unwrap().detighten_deeper_than(depth - 1);
            }
            List(blocks, _, _, blank_line_encountered) => {
                if depth <= 0 {
                    *blank_line_encountered = true
                }
                if blocks.is_empty() {
                    return;
                }
                blocks.last_mut().unwrap().detighten_deeper_than(depth - 1);
            }
            _ => return,
        }
    }

    pub fn get_last_block(&mut self) -> &mut Block {
        let descend = match self {
            Document(blocks)
            | BlockQuote(blocks, _)
            | List(blocks, _, _, _)
            | ListItem(blocks, _, _) => !blocks.is_empty(),
            _ => false,
        };

        if !descend {
            return self;
        } else {
            match self {
                Document(blocks)
                | BlockQuote(blocks, _)
                | List(blocks, _, _, _)
                | ListItem(blocks, _, _) => {
                    return blocks.last_mut().unwrap().get_last_block();
                }
                _ => unreachable!("already bool checked earlier"),
            };
        }
    }

    pub fn to_html(&self) -> String {
        let mut str_out = String::new();
        self.to_html_helper(false, &mut str_out);
        str_out
    }

    fn to_html_helper(&self, in_tight_list: bool, string_builder: &mut String) {
        match self {
            Document(blocks) => {
                for b in blocks {
                    b.to_html_helper(false, string_builder);
                }
            }
            BlockQuote(blocks, _) => {
                if string_builder.len() > 0 && !string_builder.ends_with('\n') {
                    string_builder.push('\n');
                }
                string_builder.push_str("<blockquote>\n");
                for b in blocks {
                    b.to_html_helper(false, string_builder);
                }
                string_builder.push_str("</blockquote>\n");
            }
            List(blocks, is_tight, list_type, _) => {
                if string_builder.len() > 0 && !string_builder.ends_with('\n') {
                    string_builder.push('\n');
                }
                match list_type {
                    OrderedList(_, n) => {
                        if *n != 1 {
                            string_builder.push_str(&format!("<ol start=\"{}\">\n", n));
                        } else {
                            string_builder.push_str("<ol>\n");
                        }
                        for b in blocks {
                            b.to_html_helper(*is_tight, string_builder);
                        }
                        string_builder.push_str("</ol>\n");
                    }
                    UnorderedList(_) => {
                        string_builder.push_str("<ul>\n");
                        for b in blocks {
                            b.to_html_helper(*is_tight, string_builder);
                        }
                        string_builder.push_str("</ul>\n");
                    }
                }
            }
            ListItem(blocks, ..) => {
                string_builder.push_str("<li>");
                for b in blocks {
                    b.to_html_helper(in_tight_list, string_builder);
                }
                string_builder.push_str("</li>\n");
            }
            Heading(items, h) => {
                if string_builder.len() > 0 && !string_builder.ends_with('\n') {
                    string_builder.push('\n');
                }
                string_builder.push_str(&format!("<h{}>", h));
                for c in items {
                    string_builder.push(*c);
                }
                string_builder.push_str(&format!("</h{}>\n", h));
            }
            Paragraph(items, _) => {
                if !in_tight_list && items.len() > 0 {
                    if string_builder.len() > 0 && !string_builder.ends_with('\n') {
                        string_builder.push('\n');
                    }
                    string_builder.push_str("<p>");
                }
                for c in items {
                    string_builder.push(*c);
                }
                if !in_tight_list && items.len() > 0 {
                    string_builder.push_str("</p>\n");
                }
            }
            ThematicBreak => {
                if string_builder.len() > 0 && !string_builder.ends_with('\n') {
                    string_builder.push('\n');
                }
                string_builder.push_str("<hr />\n")
            }
            IndentedCodeBlock(items, _items1) => {
                if string_builder.len() > 0 && !string_builder.ends_with('\n') {
                    string_builder.push('\n');
                }
                string_builder.push_str("<pre><code>");
                for c in items {
                    string_builder.push(*c);
                }
                string_builder.push_str("\n</code></pre>\n");
            }
            FencedCodeBlock(items, _, _, lang_hint, _, _) => {
                if string_builder.len() > 0 && !string_builder.ends_with('\n') {
                    string_builder.push('\n');
                }
                string_builder.push_str("<pre><code");
                if lang_hint.len() > 0 {
                    string_builder.push_str(" class=\"language-");
                    for c in lang_hint {
                        string_builder.push(*c);
                    }
                    string_builder.push('\"');
                }
                string_builder.push('>');
                for c in items {
                    string_builder.push(*c);
                }
                string_builder.push_str("</code></pre>\n");
            }
            HTMLBlock(items, ..) => {
                if string_builder.len() > 0 && !string_builder.ends_with('\n') {
                    string_builder.push('\n');
                }
                for c in items {
                    string_builder.push(*c);
                }
                string_builder.push('\n');
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_get_block_1() {
        let mut test_tree = Document(vec![BlockQuote(
            vec![List(
                vec![ListItem(vec![ThematicBreak], true, 2)],
                true,
                ListType::UnorderedList('*'),
                false,
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
                    vec![ListItem(vec![ThematicBreak], true, 2)],
                    true,
                    ListType::UnorderedList('*'),
                    false,
                )],
                false,
            ),
            BlockQuote(vec![Paragraph(vec!['p', 'o'], true)], true),
        ]);
        let descension = 1;
        let bq = test_tree.get_block(descension);
        match bq {
            BlockQuote(v, _) => v.push(ThematicBreak),
            _ => unreachable!(),
        }
        assert_eq!(
            test_tree.get_block(descension),
            &mut BlockQuote(vec![Paragraph(vec!['p', 'o'], true), ThematicBreak], true),
        )
    }

    #[test]
    fn test_get_last_block_1() {
        let mut test_tree = Document(vec![
            BlockQuote(
                vec![List(
                    vec![ListItem(vec![ThematicBreak], true, 2)],
                    true,
                    ListType::UnorderedList('*'),
                    false,
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

    #[test]
    fn test_get_last_general_container() {
        let ast = &mut Document(vec![BlockQuote(
            vec![List(
                vec![ListItem(vec![ThematicBreak], true, 2)],
                true,
                UnorderedList('*'),
                false,
            )],
            true,
        )]);
        let depth = 2; // matched the list but not list item
        assert_eq!(
            ast.get_general_container(depth),
            (
                &mut BlockQuote(
                    vec![List(
                        vec![ListItem(vec![ThematicBreak], true, 2)],
                        true,
                        UnorderedList('*'),
                        false,
                    )],
                    true,
                ),
                1
            )
        )
    }
}
