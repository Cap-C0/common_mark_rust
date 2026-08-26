use crate::block_structure::LRDTable;
use crate::chars::*;
use crate::inline::InlineContent::InlineLink;
use crate::inline::InlineContent::*;
use crate::inline::InlineTextComponent::*;
use crate::parsers::ReferenceLinkType::Collapsed;
use crate::parsers::ReferenceLinkType::Full;
use crate::parsers::*;
use crate::peekable_char_indices::*;
use core::panic;
use std::arch::aarch64;
use std::collections::{HashMap, VecDeque};
use std::iter;
use std::iter::Peekable;
use std::mem::swap;
use std::str::CharIndices;
use std::{mem, vec};

include!(concat!(env!("OUT_DIR"), "/unicode_categories.rs"));

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct Inline {
    // pub chars: Vec<char>,
    pub string: String,
    pub content: Vec<InlineContent>,
}

impl Inline {
    pub fn new(chars: Vec<char>) -> Self {
        Inline {
            // chars: chars,
            string: String::new(),
            content: vec![],
        }
    }

    pub fn with_string(str_in: String) -> Self {
        Inline {
            // chars: vec![],
            string: str_in,
            content: vec![],
        }
    }

    pub fn fill_content(&mut self, lrd_table: &LRDTable) {
        // dbg!("called_fill_content");
        assert!(self.content.is_empty());
        self.content = parse_inline(&self.string, lrd_table)
    }

    pub fn to_html(&self, string_builder: &mut String, lrd_table: &LRDTable) {
        for ic in &self.content {
            ic.to_html(&self.string, string_builder, lrd_table);
        }
    }
}

// we do not need to enforce multiple new line requirements in these parsers as that will be enforced
// by paragraphs ending at new lines

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum InlineContent {
    Softbreak,
    Hardbreak,
    Text(usize, usize),
    Emph(Vec<InlineContent>),
    Strong(Vec<InlineContent>),
    /// is_image, href, title, link_text
    //TODO: make this work with reference links
    InlineLink(
        bool,
        Option<(usize, usize)>,
        Option<(usize, usize)>,
        Vec<InlineContent>,
    ),
    ReferenceLink(bool, String, Vec<InlineContent>),
    /// src, title, link_text
    // Image((usize, usize), (usize, usize), Vec<InlineContent>),
    /// href and text, is_email
    AutoLink((usize, usize), bool),
    /// start, end char index (exclusive)
    HTMLTag((usize, usize)),
    Code((usize, usize)),
    // for use when swapping memory
    Dummy,
}

impl InlineContent {
    pub fn to_html(&self, string_array: &str, string_builder: &mut String, lrd_table: &LRDTable) {
        match self {
            Softbreak => string_builder.push_str("\n"),
            Hardbreak => string_builder.push_str("<br />\n"),
            Text(start, end) => {
                push_chars_with_entities_and_bs(&string_array[*start..*end], string_builder, false);
            }
            Emph(inline_contents) => {
                string_builder.push_str("<em>");
                for ic in inline_contents {
                    ic.to_html(string_array, string_builder, lrd_table);
                }
                string_builder.push_str("</em>");
            }
            Strong(inline_contents) => {
                string_builder.push_str("<strong>");
                for ic in inline_contents {
                    ic.to_html(string_array, string_builder, lrd_table);
                }
                string_builder.push_str("</strong>");
            }
            InlineLink(is_image, dest_op, tit_op, inline_contents) => {
                if *is_image {
                } else {
                    string_builder.push_str("<a href=\"");
                    if let Some((start, end)) = dest_op {
                        push_chars_with_entities_and_bs(
                            &string_array[*start..*end],
                            string_builder,
                            true,
                        );
                    }
                    string_builder.push('\"');
                    if let Some((start, end)) = tit_op {
                        string_builder.push_str(" title=\"");
                        push_chars_with_entities_and_bs(
                            &string_array[*start..*end],
                            string_builder,
                            false,
                        );
                        string_builder.push('\"');
                    }
                    string_builder.push('>');
                    for ic in inline_contents {
                        ic.to_html(string_array, string_builder, lrd_table);
                    }
                    string_builder.push_str("</a>");
                }
            }
            ReferenceLink(is_image, normalized_label, inline_contents) => todo!(),
            AutoLink((first_char, last_char), is_email) => {
                string_builder.push_str("<a href=\"");
                if *is_email {
                    string_builder.push_str("mailto:");
                }
                for c in string_array[*first_char..*last_char].chars() {
                    push_character_in_uri(c, string_builder);
                }
                string_builder.push_str("\">");
                for c in string_array[*first_char..*last_char].chars() {
                    push_html_reserved_char(c, string_builder);
                }
                string_builder.push_str("</a>");
            }
            HTMLTag((start, end)) => {
                for c in string_array[*start..*end].chars() {
                    string_builder.push(c);
                }
            }
            Code((start, end)) => {
                string_builder.push_str("<code>");
                for c in string_array[*start..*end].chars() {
                    if c == '\n' {
                        push_html_reserved_char(' ', string_builder);
                    } else {
                        push_html_reserved_char(c, string_builder);
                    }
                }
                string_builder.push_str("</code>");
            }
            Dummy => panic!("should not encounter dummy at this point"),
        }
    }
}

#[derive(Debug, Clone)]
enum InlineTextComponent<'a> {
    /// Total_count,Count_consumed, potential_opener, potential_closer
    Asts(usize, usize, bool, bool),
    /// Total_count,Count_consumed, potential_opener, potential_closer
    Unds(usize, usize, bool, bool),
    ImgOpen(bool),
    /// active
    LinkOpen(bool),
    BrackClose,
    /// Total_count
    BackTick(usize),
    AngleOpen,
    AngleClose,
    /// Offset to last char (exclusive)
    TextualContent(usize),
    CompletedContent(InlineContent),
    Fake(&'a str),
}

impl<'a> InlineTextComponent<'a> {
    fn to_text_comp(&mut self) -> InlineTextComponent<'a> {
        match self {
            Unds(total, consumed, ..) | Asts(total, consumed, ..) => {
                TextualContent(*total - *consumed)
            }
            BackTick(count) => TextualContent(*count),
            ImgOpen(..) => TextualContent(2),
            TextualContent(count) => TextualContent(*count),
            CompletedContent(ic) => {
                let mut new_ic = InlineContent::Dummy;
                mem::swap(&mut new_ic, ic);
                CompletedContent(new_ic)
            }
            _ => TextualContent(1),
        }
    }
    fn to_inline_content(&mut self, char_offset: usize) -> InlineContent {
        match self {
            Unds(total, consumed, ..) | Asts(total, consumed, ..) => {
                Text(char_offset, char_offset + (*total - *consumed))
            }
            BackTick(count) | TextualContent(count) => Text(char_offset, char_offset + *count),
            ImgOpen(..) => Text(char_offset, char_offset + 2),
            CompletedContent(c) => {
                let mut dummy = Dummy;
                mem::swap(c, &mut dummy);
                dummy
            }
            _ => Text(char_offset, char_offset + 1),
        }
    }
}

#[derive(Debug, Clone)]
struct DLLnode<'a> {
    //this indexes into the string at the start of a char, the design of the program should
    //guarantee that this doesnt panic. Namely by only considering usizes that come
    //immediately from a char_indices() and only subtracting from that offset when the character before that is known.
    beginning_char_index: usize,
    inline_component: InlineTextComponent<'a>,
    index_of_prev: Option<usize>,
    index_of_next: Option<usize>,
    index_of_this: usize, // this is helpful for getting around borrow checker shenanigans
}

impl<'a> DLLnode<'a> {
    fn new(
        begin_index: usize,
        component: InlineTextComponent<'a>,
        prev_index: Option<usize>,
        next_index: Option<usize>,
        this_index: usize,
    ) -> Self {
        DLLnode {
            beginning_char_index: begin_index,
            inline_component: component,
            index_of_prev: prev_index,
            index_of_next: next_index,
            index_of_this: this_index,
        }
    }
}

// we will be approxiamating a double linked list in rust by having each item in the list keep
// track of the index of the next item.
#[derive(Debug, Clone)]
struct FakeDelimiterDLL<'a> {
    // beginning_char_offset,end_char_offset, dl, index_of_prev, index_of_next
    dl_stack: Vec<DLLnode<'a>>,
    // Since we only push to the dll at the beginning, we do not need to keep track of
    // "freeing" things for later. (monotonic?)
    initial_index: Option<usize>,
    final_index: Option<usize>,
}

impl<'a> FakeDelimiterDLL<'a> {
    fn push_back(&mut self, begin_index: usize, dl: InlineTextComponent<'a>) {
        if self.initial_index.is_none() {
            self.initial_index = Some(self.dl_stack.len());
            self.final_index = Some(self.dl_stack.len());
            self.dl_stack.push(DLLnode::new(
                begin_index,
                dl,
                None,
                None,
                self.dl_stack.len(),
            ));
        } else {
            self.dl_stack.push(DLLnode::new(
                begin_index,
                dl,
                self.final_index,
                None,
                self.dl_stack.len(),
            ));
            self.dl_stack[self.final_index.unwrap()].index_of_next = Some(self.dl_stack.len() - 1);
            self.final_index = Some(self.dl_stack.len() - 1);
        }
    }

    fn get_first(&self) -> Option<&DLLnode> {
        self.initial_index.map(|i| &self.dl_stack[i])
    }

    fn get_first_mut(&mut self) -> Option<&mut DLLnode<'a>> {
        self.initial_index.map(|i| &mut self.dl_stack[i])
    }

    fn get_last(&self) -> Option<&DLLnode> {
        self.final_index.map(|i| &self.dl_stack[i])
    }

    fn get_last_mut(&mut self) -> Option<&mut DLLnode<'a>> {
        self.final_index.map(|i| &mut self.dl_stack[i])
    }

    fn get(&self, index: usize) -> &DLLnode {
        &self.dl_stack[index]
    }

    fn get_mut(&mut self, index: usize) -> &mut DLLnode<'a> {
        &mut self.dl_stack[index]
    }

    fn get_next(&self, node: &DLLnode) -> Option<&DLLnode> {
        node.index_of_next.map(|i| &self.dl_stack[i])
    }

    fn get_next_mut(&mut self, node: &DLLnode) -> Option<&mut DLLnode<'a>> {
        node.index_of_next.map(|i| &mut self.dl_stack[i])
    }

    fn delete_stack_above(&mut self, node_index: usize) {
        self.dl_stack[node_index].index_of_next = None;
        self.final_index = Some(node_index);
    }

    fn delete_stack_until_node(&mut self, bottom_node_index: usize, top_node_index: usize) {
        self.dl_stack[bottom_node_index].index_of_next = Some(top_node_index);
        self.dl_stack[top_node_index].index_of_prev = Some(bottom_node_index);
    }

    fn replace_inside_stack_range(
        &mut self,
        bottom_node_index: usize,
        top_node_index: usize,
        begin_char_index: usize,
        item: InlineTextComponent<'a>,
    ) {
        self.dl_stack.push(DLLnode {
            beginning_char_index: begin_char_index,
            inline_component: item,
            index_of_prev: Some(bottom_node_index),
            index_of_next: Some(top_node_index),
            index_of_this: self.dl_stack.len(),
        });
        self.dl_stack[bottom_node_index].index_of_next = Some(self.dl_stack.len() - 1);
        self.dl_stack[top_node_index].index_of_prev = Some(self.dl_stack.len() - 1);
    }

    // returns "pointer" to new node
    fn replace_inside_stack_range_including(
        &mut self,
        bottom_node_index: usize,
        top_node_index: usize,
        begin_char_index: usize,
        item: InlineTextComponent<'a>,
    ) -> usize {
        let new_prev = self.get_index_of_prev(bottom_node_index);
        let new_next = self.get_index_of_next(top_node_index);
        self.dl_stack.push(DLLnode {
            beginning_char_index: begin_char_index,
            inline_component: item,
            index_of_prev: new_prev,
            index_of_next: new_next,
            index_of_this: self.dl_stack.len(),
        });

        if let Some(prev) = new_prev {
            self.dl_stack[prev].index_of_next = Some(self.dl_stack.len() - 1);
        } else {
            self.initial_index = Some(self.dl_stack.len() - 1)
        }

        if let Some(next) = new_next {
            self.dl_stack[next].index_of_prev = Some(self.dl_stack.len() - 1);
        } else {
            self.final_index = Some(self.dl_stack.len() - 1)
        }
        self.dl_stack.len() - 1
    }

    fn delete_stack_above_including(&mut self, node_index: usize) {
        let prev_node_op = self.dl_stack[node_index]
            .index_of_prev
            .map(|i| &mut self.dl_stack[i]);
        if let Some(prev_node) = prev_node_op {
            prev_node.index_of_next = None;
            self.final_index = Some(prev_node.index_of_this);
        } else {
            self.initial_index = None;
            self.final_index = None;
        }
    }

    fn get_index_of_next(&self, index: usize) -> Option<usize> {
        self.dl_stack[index].index_of_next
    }

    fn get_index_of_prev(&self, index: usize) -> Option<usize> {
        self.dl_stack[index].index_of_prev
    }

    fn remove_at_index(&mut self, index: usize) {
        let next = self.dl_stack[index].index_of_next;
        let prev = self.dl_stack[index].index_of_prev;
        if prev.is_some() {
            self.dl_stack[prev.unwrap()].index_of_next = next;
        } else {
            self.initial_index = next;
        }
        if next.is_some() {
            self.dl_stack[next.unwrap()].index_of_prev = prev;
        } else {
            self.final_index = prev;
        }
    }

    pub fn iter(&self) -> FakeDLLIter {
        FakeDLLIter {
            index: self.initial_index,
            collection: self,
        }
    }
}

struct FakeDLLIter<'a> {
    index: Option<usize>,
    collection: &'a FakeDelimiterDLL<'a>,
}

impl<'a> Iterator for FakeDLLIter<'a> {
    type Item = &'a DLLnode<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index.is_some() {
            let out = self.collection.get(self.index.unwrap());
            self.index = out.index_of_next;
            return Some(out);
        }
        None
    }
}

// think about what functionality we will need as we go along stack

//start with just emphasis and code, then do links
//adjust the code to 2 phases, where we put all delimiters in the stack at once,
//then go forward through the stack to create
//
//BackTick code spans, auto links, raw_html >
//brackets in link text >
//emph markers.
//
// Indentation and underlying structures like quotes (>) make this a bit harder to think about
// for now, I will be cloning the strings from the source string into the Inline structs.
pub fn parse_inline(inline_str: &str, lrd_table: &LRDTable) -> Vec<InlineContent> {
    // pointer, _, offset_to_next, offset_to_prev
    // dbg!("called parse inline!");
    let mut delimit_stack = FakeDelimiterDLL {
        dl_stack: vec![],
        initial_index: None,
        final_index: None,
    };
    // let mut char_iter = chars.iter().enumerate().peekable();
    let mut char_iter = PeekableCharIndices::new(inline_str.char_indices());

    let add_text_to_stack = |sicl: &mut FakeDelimiterDLL, text_begin: usize, text_end: usize| {
        if text_end > text_begin {
            sicl.push_back(
                text_begin,
                InlineTextComponent::TextualContent(text_end - text_begin),
            );
        }
    };
    let mut text_begin = 0;
    let mut space_count = 0;
    let mut preceding_char = ' '; // for purposes of emphasis delimeter types (beginning counts as
    // whitespace)
    //do the function!
    while let Some((char_index, c)) = char_iter.next_and_index() {
        match c {
            ' ' => space_count += 1,
            '\n' => {
                // simple subtraction is ok here, we know spaces take up one byte.
                add_text_to_stack(&mut delimit_stack, text_begin, char_index - space_count);
                if space_count >= 2 {
                    delimit_stack
                        .push_back(char_index, InlineTextComponent::CompletedContent(Hardbreak));
                } else {
                    delimit_stack
                        .push_back(char_index, InlineTextComponent::CompletedContent(Softbreak));
                }
                space_count = 0;
                text_begin = char_index + 1;
            }
            _ => space_count = 0,
        }
        match c {
            '\\' => {
                if let Some(c) = char_iter.peek() {
                    if c.is_ascii_punctuation() {
                        add_text_to_stack(&mut delimit_stack, text_begin, char_index);
                        //exclude the backslash, include just the punctuation
                        delimit_stack
                            .push_back(char_index + 1, InlineTextComponent::TextualContent(1));
                        // both characters are 1 byte.
                        text_begin = char_index + 2;
                    }
                    if c == '\n' {
                        add_text_to_stack(&mut delimit_stack, text_begin, char_index);
                        delimit_stack.push_back(char_index, CompletedContent(Hardbreak));
                        text_begin = char_index + 2;
                    }
                    char_iter.next();
                    preceding_char = 'c';
                }
            }
            '`' => {
                // since the backticks are ascii, they are one byte and safe to do string offset math on throughout
                // TODO: space stripping.
                add_text_to_stack(&mut delimit_stack, text_begin, char_index);

                char_iter.consume_while_char_eq('`');
                let initial_tick_count = char_iter.offset() - char_index;

                let mut matching_tick_count_search_iter = char_iter.clone();
                let mut code_completed = false;
                let starts_with_space = match matching_tick_count_search_iter.peek() {
                    Some(' ') | Some('\n') => true,
                    _ => false,
                };
                let mut entirely_space = starts_with_space;
                let mut last_is_space = starts_with_space;
                'search_for_matching_tc: while let Some((start_pos, c)) =
                    matching_tick_count_search_iter.next_and_index()
                {
                    if c == '`' {
                        matching_tick_count_search_iter.consume_while_char_eq('`');
                        let closing_tick_count =
                            matching_tick_count_search_iter.offset() - start_pos;

                        if closing_tick_count == initial_tick_count {
                            char_iter = matching_tick_count_search_iter;
                            let remove_space =
                                if starts_with_space && last_is_space && !entirely_space {
                                    1
                                } else {
                                    0
                                };
                            delimit_stack.push_back(
                                char_index,
                                CompletedContent(Code((
                                    char_index + initial_tick_count + remove_space,
                                    start_pos - remove_space,
                                ))),
                            );
                            code_completed = true;
                            text_begin = start_pos + closing_tick_count;
                            break 'search_for_matching_tc;
                        }
                    }
                    if " \n".contains(c) {
                        last_is_space = true;
                    } else {
                        last_is_space = false;
                        entirely_space = false;
                    }
                }
                if !code_completed {
                    add_text_to_stack(
                        &mut delimit_stack,
                        char_index,
                        char_index + initial_tick_count,
                    );
                    text_begin = char_index + initial_tick_count;
                }
                preceding_char = '`';
            }
            '_' | '*' => {
                // these delimeters are size 1 byte, so it is possible to count chars by subtracting
                // indices.
                add_text_to_stack(&mut delimit_stack, text_begin, char_index);

                char_iter.consume_while_char_eq(c);

                let dc = char_iter.offset() - char_index;

                //for these purposes, end and beginning of line count as whitespace.
                let following_char = char_iter.peek().unwrap_or(' ');

                let punc_preceded =
                    preceding_char.is_unicode_punctuation() || preceding_char.is_unicode_symbol();
                let punc_followed =
                    following_char.is_unicode_punctuation() || following_char.is_unicode_symbol();
                let is_left_flanking: bool = !following_char.is_whitespace()
                    && (!punc_followed
                        || (punc_followed && (preceding_char.is_whitespace() || punc_preceded)));
                let is_right_flanking: bool = !preceding_char.is_whitespace()
                    && (!punc_preceded
                        || (punc_preceded && (following_char.is_whitespace() || punc_followed)));
                delimit_stack.push_back(
                    char_index,
                    match c {
                        '*' => Asts(dc, 0, is_left_flanking, is_right_flanking),
                        '_' => Unds(
                            dc,
                            0,
                            is_left_flanking
                                && (!is_right_flanking || (is_right_flanking && punc_preceded)),
                            is_right_flanking
                                && (!is_left_flanking || (is_left_flanking && punc_followed)),
                        ),
                        _ => panic!(),
                    },
                );
                text_begin = char_index + dc;
                preceding_char = c;
            }
            '!' => {
                if char_iter.next_if_char_eq('[').is_some() {
                    add_text_to_stack(&mut delimit_stack, text_begin, char_index);
                    delimit_stack.push_back(char_index, ImgOpen(true));
                    text_begin = char_iter.offset();
                    preceding_char = '[';
                } else {
                    preceding_char = c;
                }
            }
            '[' => {
                add_text_to_stack(&mut delimit_stack, text_begin, char_index);
                delimit_stack.push_back(char_index, LinkOpen(true));
                text_begin = char_index + 1;
                preceding_char = '[';
            }
            ']' => {
                // do the back search thing.
                add_text_to_stack(&mut delimit_stack, text_begin, char_index);
                let mut matched_node_op = delimit_stack.get_last_mut();

                while let Some(ref matched_node) = matched_node_op {
                    if matches!(matched_node.inline_component, LinkOpen(..))
                        || matches!(matched_node.inline_component, ImgOpen(..))
                    {
                        break;
                    }
                    matched_node_op = matched_node.index_of_prev.map(|i| delimit_stack.get_mut(i));
                }

                let Some(matched_node) = matched_node_op else {
                    add_text_to_stack(&mut delimit_stack, text_begin, char_index);
                    delimit_stack.push_back(char_index, BrackClose);
                    text_begin = char_index + 1;
                    continue;
                };
                let matched_node_itc = &mut matched_node.inline_component;
                if matches!(matched_node_itc, LinkOpen(false, ..))
                    || matches!(matched_node_itc, ImgOpen(false, ..))
                {
                    matched_node.inline_component = matched_node_itc.to_text_comp();
                    add_text_to_stack(&mut delimit_stack, text_begin, char_index);
                    delimit_stack.push_back(char_index, BrackClose);
                    text_begin = char_index + 1;
                    continue;
                }
                let is_image = matches!(matched_node_itc, ImgOpen(..));
                // we have found one and it is active
                // let link_content_iter = match matched_node_itc {
                //     LinkOpen(true, iter) | ImgOpen(true, iter) => iter.clone(),
                //     _ => unreachable!(),
                // };

                let mut link_made = false;
                let mut try_to_inline_link_iter = char_iter.clone();
                let mut try_to_reference_link_iter = char_iter.clone();
                if let Some((dest_op, tit_op)) =
                    dbg!(parse_inline_suffix(&mut try_to_inline_link_iter))
                {
                    let bci = matched_node.beginning_char_index;
                    let link_displayed =
                        process_emphasis(Some(matched_node.index_of_this), &mut delimit_stack);
                    delimit_stack.push_back(
                        bci,
                        CompletedContent(InlineLink(is_image, dest_op, tit_op, link_displayed)),
                    );
                    char_iter = try_to_inline_link_iter;
                    text_begin = char_iter.offset();
                    link_made = true;
                } else if let Some(rlt) = parse_reference_link(&mut try_to_reference_link_iter) {
                    match rlt {
                        Full((start, end)) => {
                            let norm_lab = normalize_label(&inline_str[start..end]);
                            if lrd_table.contains_key(&norm_lab) {
                                let bci = matched_node.beginning_char_index;
                                let link_displayed = process_emphasis(
                                    Some(matched_node.index_of_this),
                                    &mut delimit_stack,
                                );
                                delimit_stack.push_back(
                                    bci,
                                    CompletedContent(ReferenceLink(
                                        is_image,
                                        norm_lab,
                                        link_displayed,
                                    )),
                                );
                                char_iter = try_to_reference_link_iter;
                                text_begin = char_iter.offset();
                                link_made = true;
                            }
                        }
                        Collapsed => {
                            let norm_lab = normalize_label(
                                &inline_str[matched_node.beginning_char_index..char_iter.offset()],
                            );
                            if lrd_table.contains_key(&norm_lab) {
                                let bci = matched_node.beginning_char_index;
                                let link_displayed = process_emphasis(
                                    Some(matched_node.index_of_this),
                                    &mut delimit_stack,
                                );
                                delimit_stack.push_back(
                                    bci,
                                    CompletedContent(ReferenceLink(
                                        is_image,
                                        norm_lab,
                                        link_displayed,
                                    )),
                                );
                                char_iter = try_to_reference_link_iter;
                                text_begin = char_iter.offset();
                                link_made = true;
                            }
                        }
                    }
                } else {
                    // try for a shortcut link.
                    let norm_lab = normalize_label(
                        &inline_str[matched_node.beginning_char_index..char_iter.offset()],
                    );
                    if lrd_table.contains_key(&norm_lab) {
                        let bci = matched_node.beginning_char_index;
                        let link_displayed =
                            process_emphasis(Some(matched_node.index_of_this), &mut delimit_stack);
                        delimit_stack.push_back(
                            bci,
                            CompletedContent(ReferenceLink(is_image, norm_lab, link_displayed)),
                        );
                        text_begin = char_iter.offset();
                        link_made = true;
                    }
                }
                if !link_made {
                    delimit_stack.push_back(char_index, TextualContent(1));
                    text_begin = char_index + 1;
                }
                if is_image {
                    //disable earlier link openers
                }
                // if !link_made {
                // }
            }
            '<' => {
                //eagerly try to make autolink or html,
                add_text_to_stack(&mut delimit_stack, text_begin, char_index);
                let mut autolink_iter = char_iter.clone();
                let mut html_iter = char_iter.clone();
                if let Some((final_offset, is_email)) = parse_autolink(&mut autolink_iter) {
                    delimit_stack.push_back(
                        char_index,
                        CompletedContent(AutoLink((char_index + 1, final_offset - 1), is_email)),
                    );
                    char_iter = autolink_iter;
                    text_begin = char_iter.offset();
                    preceding_char = '>'
                } else if let Some(tag_end) = parse_html_tag(&mut html_iter) {
                    delimit_stack
                        .push_back(char_index, CompletedContent(HTMLTag((char_index, tag_end))));
                    char_iter = html_iter;
                    text_begin = tag_end;
                    preceding_char = '>'
                } else {
                    add_text_to_stack(&mut delimit_stack, char_index, char_index + 1);
                    text_begin = char_index + 1;
                    preceding_char = '<'
                }
            }
            _ => {
                preceding_char = c;
            }
        }
    }
    add_text_to_stack(
        &mut delimit_stack,
        text_begin,
        inline_str.len() - space_count,
    );

    process_emphasis(None, &mut delimit_stack)
}

// This can come next (since links have lower priority than code spans, autolinks, and raw html tags)
fn process_links(stack: &mut FakeDelimiterDLL) -> Vec<InlineContent> {
    process_emphasis(None, stack)
}
// fsub: forward search upper bound
// boolean tells caller if it contains a link, deepest nested link has priority
fn process_emphasis(
    stack_bottom: Option<usize>,
    stack: &mut FakeDelimiterDLL,
) -> Vec<InlineContent> {
    // dbg!("processing emph");
    dbg!(&stack);
    dbg!(&stack_bottom);
    let mut current_index_op =
        stack_bottom.map_or(stack.initial_index, |i| stack.get(i).index_of_next);
    let stack_bottom_char_index =
        stack_bottom.map(|dll_pointer| stack.get(dll_pointer).beginning_char_index);
    // only set op_bot when we find a non_matched closer_delimiter
    // set it as the *CHARACTER_OFFSET* in the corresponding vec of chars.
    let mut openers_bottom_asts_and_opening: [Option<usize>; 3] = [stack_bottom_char_index; 3];
    let mut openers_bottom_asts_not_opening: [Option<usize>; 3] = [stack_bottom_char_index; 3];
    let mut openers_bottom_unds_and_opening: [Option<usize>; 3] = [stack_bottom_char_index; 3];
    let mut openers_bottom_unds_not_opening: [Option<usize>; 3] = [stack_bottom_char_index; 3];
    // index_in_fakedldll, char_offset it points to
    let mut unds_op_stack: Vec<(usize, usize)> = vec![];
    let mut asts_op_stack: Vec<(usize, usize)> = vec![];

    while let Some(current_index) = current_index_op {
        let current_dllnode = stack.get(current_index);
        let current_beginning_char_index = current_dllnode.beginning_char_index;
        match current_dllnode.inline_component {
            Asts(total_count, consumed, pot_op, pot_clos) => {
                dbg!(&asts_op_stack);
                if pot_clos && !asts_op_stack.is_empty() {
                    let mut asts_op_stack_offset = asts_op_stack.len() - 1;
                    let this_op_bottom = &mut if pot_op {
                        openers_bottom_asts_and_opening[total_count % 3]
                    } else {
                        openers_bottom_asts_not_opening[total_count % 3]
                    };
                    let mut matching_dl_index_op = None;
                    while asts_op_stack_offset >= 0
                        && this_op_bottom
                            .map_or(true, |x| asts_op_stack[asts_op_stack_offset].1 > x)
                    {
                        if let Asts(op_tc, op_used, true, op_pc) = (stack
                            .get(asts_op_stack[asts_op_stack_offset].0)
                            .inline_component)
                        {
                            if (op_pc || pot_op)
                                && (total_count + op_tc) % 3 == 0
                                && !(total_count % 3 == 0 && op_tc % 3 == 0)
                            {
                                if asts_op_stack_offset == 0 {
                                    break;
                                }
                                asts_op_stack_offset -= 1;
                                continue;
                            } else {
                                matching_dl_index_op = Some(asts_op_stack[asts_op_stack_offset].0);
                                break;
                            }
                        } else {
                            panic!("should be some asts here")
                        }
                    }
                    if let Some(matching_dl_index) = matching_dl_index_op {
                        let (op_tc, op_used) = match stack.get(matching_dl_index).inline_component {
                            Asts(ot, ou, ..) => (ot, ou),
                            _ => panic!("should be Asts here"),
                        };
                        let matching_dl_unconsumed = op_tc - op_used;
                        let this_dl_unconsumed = total_count - consumed;
                        let is_strong = matching_dl_unconsumed >= 2 && this_dl_unconsumed >= 2;
                        // turn stack items inside the stack delimiters into actual inline content.
                        let mut emph_children: Vec<InlineContent> = vec![];
                        let mut node_to_eat_index_op = stack.get(matching_dl_index).index_of_next;
                        while node_to_eat_index_op.map_or(false, |ntei| {
                            stack.get(ntei).beginning_char_index < current_beginning_char_index
                        }) {
                            let nte = stack.get_mut(node_to_eat_index_op.unwrap());
                            emph_children.push(
                                nte.inline_component
                                    .to_inline_content(nte.beginning_char_index),
                            );
                            node_to_eat_index_op = nte.index_of_next;
                        }
                        // also clear out any delimiters on the stacks weve made in this function
                        while let Some((dll_index, opener_char_index)) =
                            unds_op_stack.pop_if(|(_, opener_char)| {
                                *opener_char > stack.get(matching_dl_index).beginning_char_index
                            })
                        {}
                        while let Some((dll_index, opener_char_index)) =
                            asts_op_stack.pop_if(|(_, opener_char)| {
                                *opener_char > stack.get(matching_dl_index).beginning_char_index
                            })
                        {}
                        stack.replace_inside_stack_range(
                            matching_dl_index,
                            current_index,
                            stack.get(matching_dl_index).beginning_char_index + op_tc,
                            CompletedContent(if is_strong {
                                Strong(emph_children)
                            } else {
                                Emph(emph_children)
                            }),
                        );
                        let closer_used_is_total;
                        if let Asts(total, used, ..) =
                            &mut stack.get_mut(current_index).inline_component
                        {
                            *used += if is_strong { 2 } else { 1 };
                            closer_used_is_total = *used == *total;
                        } else {
                            panic!("expect unds");
                        }
                        if closer_used_is_total {
                            current_index_op = stack
                                .get_next(stack.get(current_index))
                                .map(|s| s.index_of_this);
                            stack.remove_at_index(current_index);
                        }
                        let mut opener_used_is_total = false;
                        if let Asts(total, used, ..) =
                            &mut stack.get_mut(matching_dl_index).inline_component
                        {
                            *used += if is_strong { 2 } else { 1 };
                            opener_used_is_total = *used == *total;
                        } else {
                            panic!("expected asts")
                        }
                        if opener_used_is_total {
                            stack.remove_at_index(matching_dl_index);
                            asts_op_stack.remove(asts_op_stack_offset);
                        }
                        continue;
                    } else {
                        *this_op_bottom = stack.get(current_index).index_of_prev;
                        if pot_op {
                            asts_op_stack.push((
                                current_index,
                                stack.get(current_index).beginning_char_index,
                            ));
                        }
                        // we don't need a notion of "deleting" non potential_openers Since
                        // we track backwards in a different stack than this.
                        current_index_op = stack.get(current_index).index_of_next;
                    }
                } else if pot_op {
                    asts_op_stack
                        .push((current_index, stack.get(current_index).beginning_char_index));
                    current_index_op = stack.get(current_index).index_of_next;
                } else {
                    current_index_op = stack.get(current_index).index_of_next;
                }
            }
            Unds(total_count, consumed, pot_op, pot_clos) => {
                dbg!(&unds_op_stack);
                if pot_clos && !unds_op_stack.is_empty() {
                    let mut unds_op_stack_offset = unds_op_stack.len() - 1;
                    let this_op_bottom = &mut if pot_op {
                        openers_bottom_unds_and_opening[total_count % 3]
                    } else {
                        openers_bottom_unds_not_opening[total_count % 3]
                    };
                    let mut matching_dl_index_op = None;
                    while unds_op_stack_offset >= 0
                        && this_op_bottom
                            .map_or(true, |x| unds_op_stack[unds_op_stack_offset].1 > x)
                    {
                        if let Unds(op_tc, op_used, true, op_pc) = stack
                            .get(unds_op_stack[unds_op_stack_offset].0)
                            .inline_component
                        {
                            if (op_pc || pot_op)
                                && (total_count + op_tc % 3 == 0
                                    && !(total_count % 3 == 0 && op_tc % 3 == 0))
                            {
                                if unds_op_stack_offset == 0 {
                                    break;
                                }
                                unds_op_stack_offset -= 1;
                                continue;
                            } else {
                                matching_dl_index_op = Some(unds_op_stack[unds_op_stack_offset].0);
                                break;
                            }
                        } else {
                            panic!("should be some unds here")
                        }
                    }
                    if let Some(matching_dl_index) = matching_dl_index_op {
                        let (op_tc, op_used) = match stack.get(matching_dl_index).inline_component {
                            Unds(ot, ou, ..) => (ot, ou),
                            _ => panic!("should be Unds here"),
                        };
                        let matching_dl_unconsumed = op_tc - op_used;
                        let this_dl_unconsumed = total_count - consumed;
                        let is_strong = matching_dl_unconsumed >= 2 && this_dl_unconsumed >= 2;
                        // turn stack items inside the stack delimiters into actual inline content.
                        let mut emph_children: Vec<InlineContent> = vec![];
                        let mut node_to_eat_index_op = stack.get(matching_dl_index).index_of_next;
                        while node_to_eat_index_op.map_or(false, |ntei| {
                            stack.get(ntei).beginning_char_index < current_beginning_char_index
                        }) {
                            let nte = stack.get_mut(node_to_eat_index_op.unwrap());
                            emph_children.push(
                                nte.inline_component
                                    .to_inline_content(nte.beginning_char_index),
                            );
                            node_to_eat_index_op = nte.index_of_next;
                        }
                        // also clear out any delimiters on the stacks weve made in this function
                        while let Some((dll_index, opener_char_index)) =
                            unds_op_stack.pop_if(|(_, opener_char)| {
                                *opener_char > stack.get(matching_dl_index).beginning_char_index
                            })
                        {}
                        while let Some((dll_index, opener_char_index)) =
                            asts_op_stack.pop_if(|(_, opener_char)| {
                                *opener_char > stack.get(matching_dl_index).beginning_char_index
                            })
                        {}
                        stack.replace_inside_stack_range(
                            matching_dl_index,
                            current_index,
                            stack.get(matching_dl_index).beginning_char_index + op_tc,
                            CompletedContent(if is_strong {
                                Strong(emph_children)
                            } else {
                                Emph(emph_children)
                            }),
                        );
                        let closer_used_is_total;
                        if let Unds(total, used, ..) =
                            &mut stack.get_mut(current_index).inline_component
                        {
                            *used += if is_strong { 2 } else { 1 };
                            closer_used_is_total = *used == *total;
                        } else {
                            panic!("expect unds");
                        }
                        if closer_used_is_total {
                            current_index_op = stack
                                .get_next(stack.get(current_index))
                                .map(|s| s.index_of_this);
                            stack.remove_at_index(current_index);
                        }
                        let mut opener_used_is_total = false;
                        if let Unds(total, used, ..) =
                            &mut stack.get_mut(matching_dl_index).inline_component
                        {
                            *used += if is_strong { 2 } else { 1 };
                            opener_used_is_total = *used == *total;
                        } else {
                            panic!("expected unds")
                        }
                        if opener_used_is_total {
                            stack.remove_at_index(matching_dl_index);
                            unds_op_stack.remove(unds_op_stack_offset);
                        }
                        continue;
                    } else {
                        *this_op_bottom = stack.get(current_index).index_of_prev;
                        if pot_op {
                            unds_op_stack.push((
                                current_index,
                                stack.get(current_index).beginning_char_index,
                            ));
                        }
                        // we don't need a notion of "deleting" non potential_openers Since
                        // we track backwards in a different stack than this.
                        current_index_op = stack.get(current_index).index_of_next;
                    }
                } else if pot_op {
                    unds_op_stack
                        .push((current_index, stack.get(current_index).beginning_char_index));
                    current_index_op = stack.get(current_index).index_of_next;
                } else {
                    current_index_op = stack.get(current_index).index_of_next;
                }
            }
            _ => current_index_op = stack.dl_stack[current_index].index_of_next,
        }
    }

    // for dl_node in stack.iter() {
    //     dbg!(dl_node);
    // }
    // dbg!(asts_op_stack);
    let mut out = vec![];
    // now we can iterate through the stack above stack_bottom
    let mut ntei_op = stack_bottom.map_or(stack.initial_index, |i| stack.get(i).index_of_next);
    while let Some(ntei) = ntei_op {
        let nte = stack.get_mut(ntei);
        out.push(
            nte.inline_component
                .to_inline_content(nte.beginning_char_index),
        );
        ntei_op = nte.index_of_next;
    }
    //then remove everything above
    if let Some(sb_index) = stack_bottom {
        stack.delete_stack_above_including(sb_index);
    }
    // dbg!(&out);
    out
}
