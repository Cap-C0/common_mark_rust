use serde::de;
use serde_json::Value::Array;

use crate::inline::InlineTextComponent::*;
use crate::inline::{InlineContent::*};
use crate::chars::*;
use core::panic;
use std::arch::aarch64;
use std::collections::{HashMap, VecDeque};
use std::{mem, vec};


include!(concat!(env!("OUT_DIR"), "/unicode_categories.rs"));

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct Inline {
    pub chars: Vec<char>,
    pub content: Vec<InlineContent>,
}

impl Inline {
    pub fn new(chars: Vec<char>) -> Self {
        Inline{
            chars: chars,
            content: vec![],
        }
    }

    pub fn fill_content(&mut self, lrd_table: &HashMap<Vec<char>, (Vec<char>, Vec<char>)>) {
        // dbg!("called_fill_content");
        assert!(self.content.is_empty());
        self.content = parse_inline(&self.chars, lrd_table)
    }

    pub fn to_html(&self, string_builder: &mut String) {
        for ic in &self.content {
            ic.to_html(&self.chars, string_builder);
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
    /// href, title, link_text
    Link((usize, usize), (usize, usize), Vec<InlineContent>),
    /// src, title, link_text
    Image((usize, usize), (usize, usize), Vec<InlineContent>),
    /// href and text, is_email
    AutoLink((usize, usize), bool),
    /// start, end char index (exclusive)
    HTMLTag((usize,usize)),
    Code((usize,usize)),
    // for use when swapping memory
    Dummy, 
}

impl InlineContent {
    pub fn to_html(&self, characters: &[char], string_builder: &mut String) {
        match self {
            Softbreak => string_builder.push_str("\n"),
            Hardbreak => string_builder.push_str("<br />\n"),
            Text(start, end) => { 
                push_chars_with_entities_and_bs(&characters[*start..*end], string_builder);
            },
            Emph(inline_contents) => {
                string_builder.push_str("<em>");
                for ic in inline_contents {
                    ic.to_html(characters, string_builder);
                }
                string_builder.push_str("</em>");
            },
            Strong(inline_contents) => {
                                string_builder.push_str("<strong>");
                for ic in inline_contents {
                    ic.to_html(characters, string_builder);
                }
                string_builder.push_str("</strong>");

            }
            Link(_, _, inline_contents) => todo!(),
            Image(_, _, inline_contents) => todo!(),
            AutoLink((first_char,last_char), is_email) => {
                string_builder.push_str("<a href=\"");
                if *is_email {
                    string_builder.push_str("mailto:");
                }
                for ci in *first_char..*last_char {
                    push_character_in_uri(characters[ci], string_builder);
                }
                string_builder.push_str("\">");
                for ci in *first_char..*last_char {
                    push_html_reserved_char(characters[ci], string_builder);
                }
                string_builder.push_str("</a>");
                
            },
            HTMLTag((start,end)) => {
                for ci in *start..*end {
                    string_builder.push(characters[ci])
                }
            }
            Code((start, end)) => {
                dbg!(characters);
                let strip_space = " \n".contains(dbg!(characters[dbg!(*start)])) && " \n".contains(dbg!(characters[dbg!(*end - 1)])); 
                dbg!(&strip_space); 
                let mut entirely_space = true;
                string_builder.push_str("<code>");
                let strip_start = if strip_space {*start + 1} else {*start};
                let strip_end = if strip_space {*end - 1} else {*end};
                for i in strip_start..strip_end{
                    if " \n".contains(characters[i]) {
                        push_html_reserved_char(' ', string_builder);
                    } else {
                        entirely_space = false;
                        push_html_reserved_char(characters[i], string_builder);
                    }
                }
                if entirely_space && strip_space {
                    if *start != *end - 1{
                        push_html_reserved_char(' ', string_builder);
                    }
                    push_html_reserved_char(' ', string_builder);
                }
                string_builder.push_str("</code>");
            },
            Dummy => panic!("should not encounter dummy at this point"),
        }
    }
}


/*
 * returns Some(k) if text matches [.*(c| c != " \t\n")*.*]
 * to get the link label, from the return value, get text[1..k-1]*/
pub fn parse_link_label(text: &[char]) -> Option<usize> {
    let mut offset = 0;
    if text.len() <= offset || text[offset] != '[' {
        return None;
    }
    offset += 1;
    let mut non_space_encountered = false;

    while offset < text.len() && offset <= 1000 && text[offset] != ']' {
        if !text[offset].is_whitespace() {
            non_space_encountered = true
        }
        if text[offset] == '\\' {
            offset += 1;
        } else if text[offset] == '[' {
            return None;
        }
        offset += 1;
    }

    if offset < text.len() && offset <= 1000 && non_space_encountered {
        return Some(offset + 1);
    }

    None
}

pub fn normalize_label(label: Vec<char>) -> Vec<char> {
    let mut lab_out = vec![];
    let stripped_brackets = &label[1..label.len() - 1];
    let mut char_iter = stripped_brackets.iter().peekable();
    //strip leading whitespace
    while char_iter.peek().unwrap().is_whitespace() {
        char_iter.next();
    }
    let mut seen_space = false;
    while let Some(&c) = char_iter.next() {
        if " \n\t".contains(c) {
            seen_space = true
        } else {
            if seen_space {
                seen_space = false;
                lab_out.push(' ');
            }
            c.to_lowercase().for_each(|cl| lab_out.push(cl));
        }
    }
    lab_out
}
/*
 * returns Some(k, bool) if the first k chars of text matches link definition
 * returns true if the chars are in <> and false if they are not
 */
pub fn parse_link_destination(text: &[char]) -> Option<(usize, bool)> {
    let mut offset = 0;
    if text.len() <= offset {
        return None;
    }

    if text[offset] == '<' {
        offset += 1;
        while offset < text.len() && text[offset] != '>' {
            if text[offset] == '\\' {
                offset += 2;
            } else if text[offset] == '<' {
                return None;
            } else {
                offset += 1;
            }
        }
        if offset < text.len() {
            return Some((offset, true));
        }
        return None;
    }

    if text[offset].is_whitespace() || text[offset].is_ascii_control() {
        return None;
    }
    let mut paren_stack = 0;
    while offset < text.len()
        && !(text[offset].is_whitespace())
        && !(text[offset].is_ascii_control())
    {
        if text[offset] == '(' {
            paren_stack += 1;
        } else if text[offset] == ')' {
            if paren_stack > 0 {
                paren_stack -= 1;
            } else {
                return None;
            }
        } else if text[offset] == '\\'
            && text.len() > offset + 1
            && !text[offset + 1].is_whitespace()
        {
            offset += 1
        }
        offset += 1;
    }

    if paren_stack == 0 {
        return Some((offset, false));
    }
    None
}



pub fn parse_link_title(text: &[char]) -> Option<usize> {
    let mut offset = 0;
    if offset >= text.len() {
        return None;
    }
    let c = text[offset];
    if !"\"\'(".contains(c) {
        return None;
    }
    let matching_delimiter = if c == '(' { ')' } else { c };
    offset += 1;
    while offset < text.len() && text[offset] != matching_delimiter {
        if text[offset] == c {
            return None;
        }
        if text[offset] == '\\' {
            offset += 1;
        }
        offset += 1;
    }

    if offset < text.len() {
        return Some(offset + 1);
    }
    None
}

//For purposes of this spec, a scheme is any sequence of 2–32 characters beginning with an ASCII letter and followed by any combination 
//of ASCII letters, digits, or the symbols plus (“+”), period (“.”), or hyphen (“-”).
//returns Some(x) if the first x characters of the text are a scheme, otherwise None.
pub fn parse_scheme(text: &[char]) -> Option<usize> {
    let mut char_count = 0;
    while char_count < text.len() && char_count <= 32 && (text[char_count].is_ascii_alphanumeric() || "+.-".contains(text[char_count])) {
        char_count += 1;
    }
    if 2 <= char_count && char_count <= 32 {
        return Some(char_count);
    }
    None
}

pub fn parse_uri_autolink(text: &[char]) -> Option<usize> {
    let mut offset = 0;
    if text.len() > 0 && text[offset] == '<'{
        offset += 1;
    } else {
        return None;
    }
    if let Some(scheme_char_count) = parse_scheme(&text[offset..]) {
        if scheme_char_count + 1 < text.len() && text[scheme_char_count + 1] == ':' {
            offset += scheme_char_count + 2;
        } else {
            return None;
        }
    } else {
        return None;
    }
    while offset < text.len() && !text[offset].is_ascii_control() && !(" \n<>".contains(text[offset])) {
        offset += 1;
    }
    if offset < text.len() && text[offset] == '>' {
        return Some(offset + 1);
    }
    None
}

//An email address, for these purposes, is anything that matches the non-normative regex from the HTML5 spec:
// /^[a-zA-Z0-9.!#$%&'*+/=?^_`{|}~-]+@[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?
// (?:\.[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)*$/
pub fn parse_email(text: &[char]) -> Option<usize> {
    let is_atext_or_dot = |c: char| {
        c.is_ascii_alphanumeric() || ".!#$%&'*+/=?^_`{|}~-".contains(c)
    };
    let mut offset = 0;
    if text.len() > 0 && text[offset] == '<'{
        offset += 1;
    } else {
        return None;
    }
    while offset < text.len() && is_atext_or_dot(text[offset]) {
        offset += 1;
    }
    if offset >= 1 && offset < text.len() && text[offset] == '@' {
        offset += 1;
    } else {
        return None;
    }

    let parse_label = |text: &[char]| {
        let mut char_count = 0;
        if char_count < text.len() && text[char_count].is_ascii_alphanumeric() {
            char_count += 1; 
        } else {
            return None;
        };
        let mut last_is_hyphen = false;
        while char_count < text.len() && char_count <= 63 && text[char_count].is_ascii_alphanumeric() || text[char_count] == '-' {
            last_is_hyphen = text[char_count] == '-';
            char_count +=1;
        }
        if char_count <=63 && !last_is_hyphen {
            return Some(char_count);
        }
        None
    };

    if let Some(chars_eaten) = parse_label(&text[offset..]) {
        offset += chars_eaten
    } else {
        return None;
    }
    while offset < text.len() && text[offset] == '.' {
        offset += 1;
        if let Some(chars_eaten) = parse_label(&text[offset..]) {
            offset += chars_eaten
        } else {
            return None;
        }
    }
    if offset < text.len() && text[offset] == '>' {
        return Some(offset + 1);
    }
    None
}

pub fn parse_autolink(text: &[char]) -> Option<(usize, bool)> {
    if let Some(chars_eaten) = parse_uri_autolink(text) {
        return Some((chars_eaten,false));
    } else if let Some(chars_eaten) = parse_email(text) {
        return Some((chars_eaten, true));
    } else {
        return  None;
    }

}

pub fn parse_html_tag(text: &[char]) -> Option<usize> {
    let mut offset = 0;
    if offset >= text.len() || text[offset] != '<' {
        return None;
    }
    offset += 1;
    if offset < text.len() {
        match text[offset] {
            '/' => {
                if let Some(x) = parse_closing_tag(&text[offset + 1..]) {
                    offset += 1 + x;
                } else {
                    return None;
                }
            }
            '?' => {
                if let Some(x) = parse_processing_instruction(&text[offset + 1..]) {
                    offset += x + 1;
                } else {
                    return None;
                }
            }
            '!' => {
                println!("exclamation!");
                let cdata_str = "[CDATA[";
                if offset + 2 < text.len() && text[offset + 1] == '-' && text[offset + 2] == '-' {
                    if let Some(x) = parse_html_comment(&text[offset + 3..]) {
                        offset += 3 + x;
                    } else {
                        return None;
                    }
                } else 
                if offset + cdata_str.len() < text.len() && 
                    dbg!(&text[offset + 1..offset + 1 + cdata_str.len()]).iter().map(|c|*c).eq(cdata_str.chars()) {
                        dbg!("CDATA");
                    if let Some(x) = parse_cdata_section(&text[offset + cdata_str.len()..]) {
                        offset += cdata_str.len() + x;
                    } else {
                        return None;
                    }
                } else {
                    dbg!("parsing declaration!");
                    if let Some(x) = parse_declaration(&text[offset+1..]) {
                        offset += 1 + x;
                    } else {
                        return None;
                    }
                }
            }
            _ => {
                if let Some(x) = parse_opening_tag(&text[offset..]){
                    offset += x;
                } else {
                    return None;
                }
            }

        }
    }
    Some(offset)
}

// These functions are called based on lookahead by callers, so opening brackets are consumed
// closing brackets are not though.
pub fn parse_opening_tag(text: &[char]) -> Option<usize> {
    let mut offset = 0;
    if offset >= text.len() || !text[offset].is_ascii_alphabetic() {
        return None;
    }
        offset += 1;
    while offset < text.len() 
        && (text[offset] == '-' || text[offset].is_ascii_alphanumeric()) 
    {
        offset += 1;
    }

    while offset < text.len() && let Some(x) = parse_attribute(&text[offset..]){
        offset += x;
    }

    //trailing white space
    while offset < text.len() && " \t\n".contains(text[offset]) {
        offset += 1;
    }

    //optional
    if offset < text.len() && text[offset] == '/' {
        offset += 1;
    }

    if offset < text.len() && text[dbg!(offset)] == '>' {
        return Some(offset + 1);
    }
    None
}

pub fn parse_closing_tag(text: &[char]) -> Option<usize> {
    let mut offset = 0;
    if offset >= text.len() || !text[offset].is_ascii_alphabetic() {
        return None;
    }
        offset += 1;
    while offset < text.len() 
        && (text[offset] == '-' || text[offset].is_ascii_alphanumeric()) 
    {
        offset += 1;
    }

    //trailing white space
    while offset < text.len() && " \t\n".contains(text[offset]) {
        offset += 1;
    }

    if offset < text.len() && text[dbg!(offset)] == '>' {
        return Some(offset + 1);
    }
    None
}

pub fn parse_attribute (text: &[char]) -> Option<usize> {
    let mut offset = 0;
    //initial_white_space
    while " \t\n".contains(text[offset]) {
        offset += 1;
    }
    //attribute name
    //must have one or more space seperating attributes.
    if offset >= text.len()
        || offset == 0
        || !("_:".contains(text[offset]) || text[offset].is_alphabetic())
    // must start with letter, _, or :
    {
        return None;
    }
    offset += 1;
    while text.len() > offset
        && ("_.:-".contains(text[offset]) || text[offset].is_ascii_alphanumeric())
    {
        offset += 1;
    }
    let pre_val_offset = offset;

    // attribute value specification.
    while text.len() > offset && " \t".contains(text[offset]) {
        offset += 1;
    }
    if text.len() <= offset || text[offset] != '=' {
        return Some(pre_val_offset);
    }
    offset += 1;
    while text.len() > offset && " \t".contains(text[offset]) {
        offset += 1;
    }
    if text.len() <= offset {
        return Some(pre_val_offset);
    }
    let c = text[offset];
    //quoted attribute 
    if "\"\'".contains(c) {
        offset += 1;
        while text.len() > offset && text[offset] != c {
            offset += 1;
        }
        offset += 1;
        return Some(offset);
    }
    //unquoted attribute
    while text.len() > offset && !" \t\n\"\'=<>`".contains(text[offset]) {
        offset += 1;
    }
    Some(offset)
}


pub fn parse_html_comment(text: &[char]) -> Option<usize> {
    let mut offset = 0;
    if offset < text.len() && text[offset] == '>' {
        return Some(offset+1);
    }
    if offset + 1 < text.len() && text[offset] == '-' && text[offset+1] == '>' {
        return Some(offset+2);
    }
    while offset < text.len() {
        if text[offset] == '-' && offset + 2 < text.len()
            &&text[offset + 1] == '-' && text[offset+2] == '>' {
                return Some(offset + 3);
        }
        offset+=1;
    }
    None
}

pub fn parse_processing_instruction(text: &[char]) -> Option<usize> {
    let mut offset = 0;
    while offset < text.len() {
        if text[offset] == '?' && offset + 1 < text.len()
            &&text[offset + 1] == '>' {
                return Some(offset + 2);
        }
        offset+=1;
    }
    None
}

pub fn parse_declaration(text: &[char]) -> Option<usize> {
    let mut offset = 0;
    if offset >= text.len() || !dbg!(text[offset]).is_ascii_alphabetic(){
        return None;
    }
    while offset < text.len() {
        if text[offset] == '>'{
            return Some(offset + 1);
        }
        offset+=1;
    }
    None
}

pub fn parse_cdata_section(text: &[char]) -> Option<usize> {
    let mut offset = 0;
    while offset < text.len() {
        if text[offset] == ']' && offset + 2 < text.len()
            &&text[offset + 1] == ']' && text[offset+2] == '>' {
                return dbg!(Some(offset + 3));
        }
        offset+=1;
    }
    None
}
    

#[derive(Debug, PartialEq, Eq, Clone)]
enum InlineTextComponent {
    /// Total_count,Count_consumed, potential_opener, potential_closer
    Asts(usize,usize, bool,bool),
    /// Total_count,Count_consumed, potential_opener, potential_closer
    Unds(usize, usize, bool,bool),
    ImgOpen,
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
}

impl InlineTextComponent {
    fn to_text_comp(self) -> InlineTextComponent {
        match self {
            Unds(total,consumed,..) | Asts(total, consumed,.. ) => {
                TextualContent(total - consumed)
            },
            BackTick(count) => {
                TextualContent(count)
            }
            ImgOpen => TextualContent(2),
            TextualContent(_) | CompletedContent(_) => self,
            _ => TextualContent(1),
        }
    }
    fn to_inline_content(&mut self, char_offset: usize) -> InlineContent {
        match self {
            Unds(total,consumed,..) | Asts(total, consumed,.. ) => {
                Text(char_offset, char_offset + (*total - *consumed))
            },
            BackTick(count) | TextualContent(count)=> {
                Text(char_offset, char_offset + *count)
            }
            ImgOpen => Text(char_offset, char_offset+ 2),
            CompletedContent(c) => {
                let mut dummy = Dummy;
                mem::swap(c, &mut dummy);
                dummy
            }
            _ => Text(char_offset, char_offset + 1),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
struct DLLnode {
    beginning_char_index: usize,
    inline_component: InlineTextComponent,
    index_of_prev: Option<usize>,
    index_of_next: Option<usize>,
    index_of_this: usize, // this is helpful for getting around borrow checker shenanigans
}

impl DLLnode {
    fn new(begin_index: usize, component: InlineTextComponent,  prev_index: Option<usize>,next_index: Option<usize>,this_index: usize) -> Self {
        DLLnode{
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
#[derive(Debug, PartialEq, Eq, Clone)]
struct FakeDelimiterDLL {
    // beginning_char_offset,end_char_offset, dl, index_of_prev, index_of_next
    dl_stack: Vec<DLLnode>,
    // Since we only push to the dll at the beginning, we do not need to keep track of 
    // "freeing" things for later. (monotonic?)
    initial_index: Option<usize>,
    final_index: Option<usize>,
}



impl FakeDelimiterDLL {
    fn push_back(&mut self,begin_index:usize,dl: InlineTextComponent) {
        if self.initial_index.is_none() {
            self.initial_index = Some(self.dl_stack.len());
            self.final_index = Some(self.dl_stack.len());
            self.dl_stack.push(DLLnode::new(begin_index, dl, None, None, self.dl_stack.len()));
        } else {
             self.dl_stack.push(DLLnode::new(begin_index, dl, self.final_index, None, self.dl_stack.len()));
             self.dl_stack[self.final_index.unwrap()].index_of_next = Some(self.dl_stack.len() - 1);
             self.final_index = Some(self.dl_stack.len() - 1);
        }
    }

    fn get_first(&self) -> Option<&DLLnode> {
        self.initial_index.map(|i| &self.dl_stack[i])
    }

    fn get_first_mut(&mut self) ->Option<&mut DLLnode> {
        self.initial_index.map(|i| &mut self.dl_stack[i])
    }

    fn get_last(&self) -> Option<&DLLnode> {
        self.final_index.map(|i| &self.dl_stack[i])
    }

    fn get_last_mut(&mut self) ->Option<&mut DLLnode> {
        self.final_index.map(|i| &mut self.dl_stack[i])
    }

    fn get(& self, index: usize) -> &DLLnode {
        &self.dl_stack[index]
    }

    fn get_mut(&mut self, index: usize) -> &mut DLLnode {
        &mut self.dl_stack[index]
    }

    fn get_next(&self, node: &DLLnode) -> Option<&DLLnode> {
        node.index_of_next.map(|i|{
            &self.dl_stack[i]
        })
    }

    fn get_next_mut(&mut self, node: &DLLnode) -> Option<&mut DLLnode> {
        node.index_of_next.map(|i|{
            &mut self.dl_stack[i]
        })
    }

    fn delete_stack_above(&mut self, node_index: usize) {
        self.dl_stack[node_index].index_of_next = None;
        self.final_index = Some(node_index);
    }

    fn delete_stack_until_node(&mut self, bottom_node_index: usize, top_node_index: usize) {
        self.dl_stack[bottom_node_index].index_of_next = Some(top_node_index);
        self.dl_stack[top_node_index].index_of_prev = Some(bottom_node_index);
    }

    fn replace_inside_stack_range(&mut self, bottom_node_index: usize, top_node_index: usize, begin_char_index:usize,
        item: InlineTextComponent) {
        self.dl_stack.push(DLLnode{
            beginning_char_index: begin_char_index,
            inline_component: item,
            index_of_prev : Some(bottom_node_index),
            index_of_next : Some(top_node_index),
            index_of_this : self.dl_stack.len(),
        });
        self.dl_stack[bottom_node_index].index_of_next = Some(self.dl_stack.len() - 1);
        self.dl_stack[top_node_index].index_of_prev = Some(self.dl_stack.len() - 1);

    }

    fn delete_stack_above_including(&mut self, node_index: usize) {
        let prev_node_op = self.dl_stack[node_index].index_of_prev.map(|i| &mut self.dl_stack[i]);
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
        FakeDLLIter{index: self.initial_index, collection: self}
    }
}

struct FakeDLLIter<'a> {
    index: Option<usize>,
    collection: &'a FakeDelimiterDLL,
}


impl<'a> Iterator for FakeDLLIter<'a> {
    type Item = &'a DLLnode;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index.is_some(){
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
pub fn parse_inline(chars: &[char], lrd_table: &HashMap<Vec<char>, (Vec<char>, Vec<char>)> ) -> Vec<InlineContent> {
    // pointer, _, offset_to_next, offset_to_prev
    // dbg!("called parse inline!");
    let mut delimit_stack = FakeDelimiterDLL{
        dl_stack: vec![],
        initial_index: None,
        final_index: None
    };
    let mut char_iter = chars.iter().enumerate().peekable();
    let mut text_begin = 0;

    let mut add_text_to_stack = |sicl: &mut FakeDelimiterDLL, text_begin:usize, text_end: usize| {
        if text_end > text_begin {
            sicl.push_back(text_begin, InlineTextComponent::TextualContent(text_end - text_begin));
        }
    };
    let mut space_count = 0;
    //do the function!
    while let Some((char_index, &c)) = char_iter.next() {
        match c {
            ' ' => space_count += 1,
            '\n' => {
                add_text_to_stack(&mut delimit_stack, text_begin, char_index - space_count);
                if space_count >= 2 {
                    delimit_stack.push_back(char_index, InlineTextComponent::CompletedContent(Hardbreak));
                } else {
                    delimit_stack.push_back(char_index, InlineTextComponent::CompletedContent(Softbreak));
                }
                space_count = 0;
                text_begin = char_index + 1;
            }
            _ => space_count = 0,
            
        }
        match c {
            '\\' => {
                if let Some((_, '>')) = char_iter.peek() {
                    let mut next_node = delimit_stack.get_first();
                    let mut match_found = None;
                    while let Some(dllnode) = next_node {
                        if let AngleOpen = dllnode.inline_component {
                            // try to make it either autolink or Raw HTML
                            let start_char_index = dllnode.beginning_char_index;
                            let index_of_matching = dllnode.index_of_this;
                            if let Some((chars_eaten, is_email)) = parse_autolink(dbg!(&chars[start_char_index..char_index+2])) {
                                if start_char_index + chars_eaten == char_index + 2 {
                                    match_found = Some((index_of_matching, AutoLink((start_char_index + 1, char_index + 1), is_email)));
                                    break;
                                }
                            } else if let Some(chars_eaten) = dbg!(parse_html_tag(&chars[start_char_index..char_index+1])) {
                                if dbg!(start_char_index) + chars_eaten == dbg!(char_index + 2) {
                                    match_found = Some((index_of_matching, HTMLTag((start_char_index, char_index + 2))));
                                    break;
                                }
                            }
                        }
                        next_node = delimit_stack.get_next(dllnode);
                    }
                    if let Some((matching_bracket_index, ic_found)) = match_found {
                        let begin_char = delimit_stack.get(matching_bracket_index).beginning_char_index;
                        delimit_stack.delete_stack_above_including(matching_bracket_index);
                        delimit_stack.push_back(begin_char, CompletedContent(ic_found));
                    } else {
                        add_text_to_stack(&mut delimit_stack, text_begin, char_index);
                        delimit_stack.push_back(char_index+1, AngleClose);
                    }
                    text_begin = char_index + 2;
                    char_iter.next();

                } else if let Some((_, '`')) = char_iter.peek() {
                    dbg!("seen a backtick!");
                    add_text_to_stack(&mut delimit_stack,text_begin, char_index);
                    char_iter.next();
                    // see if that form a codespan.
                    let mut tick_count = 1;
                    while char_iter.peek().map_or(false, |&(_, &c)| c == '`') {
                        tick_count += 1;
                        char_iter.next();
                    }
                    // then look from beginning of stack for a matching tick_count
                    let mut next_node = delimit_stack.get_first();
                    while let Some(dllnode) = next_node {
                        if let BackTick(x) = dllnode.inline_component {
                            if x == tick_count {
                                break;
                            }
                        }
                        next_node = delimit_stack.get_next(dllnode);
                    }
                    if let Some(matching_node) = next_node {
                        let starting_char_index = matching_node.beginning_char_index;
                        let index_of_matching = matching_node.index_of_this;
                        delimit_stack.delete_stack_above_including(index_of_matching);
                        delimit_stack.push_back(starting_char_index, CompletedContent(
                                Code((starting_char_index + tick_count, char_index + 1)) //include
                                                                                         //the
                                                                                         //backslash!
                                ));
                    } else {
                        // add just the tick as an escaped char.
                        delimit_stack.push_back(char_index + 1, InlineTextComponent::TextualContent(1));
                        // and add the delimit_stack with -1 starting_char
                        if tick_count > 1 {
                            delimit_stack.push_back(char_index + 2, BackTick(tick_count - 1));
                        }
                    }
                    text_begin = char_index + 1 + tick_count;
                    // otherwise its an espaped backtick.
                } else if let Some((_,'\n')) = char_iter.peek() {
                        //create a Hardbreak
                        add_text_to_stack(&mut delimit_stack, text_begin, char_index);
                        delimit_stack.push_back(char_index + 1, InlineTextComponent::CompletedContent(Hardbreak));
                        text_begin = char_index + 2;
                        char_iter.next();
                }else if let Some((_, c)) = char_iter.peek(){
                    if c.is_ascii_punctuation() {
                        add_text_to_stack(&mut delimit_stack, text_begin, char_index);
                        //exclude the backslash, include just the punctuation
                        delimit_stack.push_back(char_index + 1, InlineTextComponent::TextualContent(1));
                        text_begin = char_index + 2;
                    }
                    char_iter.next();
                }
            },
            '`' =>{
                add_text_to_stack(&mut delimit_stack,text_begin, char_index);
                let mut tick_count = 1;
                while char_iter.peek().map_or(false, |&(_, &c)| c == '`') {
                    tick_count += 1;
                    char_iter.next();
                }
                // then look from beginning of stack for a matching tick_count
                let mut next_node = delimit_stack.get_first();
                while let Some(dllnode) = next_node {
                    if let BackTick(x) = dllnode.inline_component {
                        if x == tick_count {
                            break;
                        }
                    }
                    next_node = delimit_stack.get_next(dllnode);
                }
                if let Some(matching_node) = next_node {
                    let starting_char_index = matching_node.beginning_char_index;
                    let index_of_matching = matching_node.index_of_this;
                    dbg!( index_of_matching);
                    delimit_stack.delete_stack_above_including(index_of_matching);
                    delimit_stack.push_back(starting_char_index, CompletedContent(
                            Code((starting_char_index + tick_count, char_index))
                            ));
                }
                else {
                    // otherwise if none found, add to stack
                    delimit_stack.push_back(char_index, BackTick(tick_count));
                }
                text_begin = char_index + tick_count;
            },
            '_'|'*' => {
                add_text_to_stack(&mut delimit_stack, text_begin,char_index);
                let mut dc = 1; //delimiter count
                while char_iter.peek().map_or(false, |&(_, &d)| d == c) {
                    dc += 1;
                    char_iter.next();
                }
                let punc_preceded = char_index > 0 && (chars[char_index - 1].is_unicode_punctuation() || chars[char_index - 1].is_unicode_symbol());
                let punc_followed = char_index + dc < chars.len() && (chars[char_index  + dc].is_unicode_punctuation() || chars[char_index  + dc].is_unicode_symbol());
                let is_left_flanking: bool = char_index + dc < chars.len() &&    //not_whitespace_followed
                    !(chars[char_index + dc].is_whitespace()) &&
                    (
                        !punc_followed ||
                        (punc_followed &&
                            (char_index <= 0 || 
                            chars[char_index - 1].is_whitespace() || 
                            punc_preceded)
                        )
                    ); // or followed by punc and preceded by ws or punc
                let is_right_flanking: bool = char_index > 0 &&// not_whitespace_preceded
                    !(chars[char_index - 1].is_whitespace()) &&
                    (
                        !punc_preceded ||
                        (punc_preceded && 
                            (char_index + dc >= chars.len() ||
                            chars[char_index + dc].is_whitespace() ||
                            punc_followed)
                        )
                    );
                delimit_stack.push_back(char_index, 
                    match c {
                        '*' => {Asts(dc,0, is_left_flanking,is_right_flanking)}
                        '_' => {Unds(dc,0, 
                            is_left_flanking && 
                            (
                                !is_right_flanking
                                ||
                                (is_right_flanking && punc_preceded)
                            )
                            ,is_right_flanking &&
                            (
                                !is_left_flanking
                                ||
                                (is_left_flanking && punc_followed)
                            )
                            )},
                        _ => panic!()
                    }
                );
                text_begin = char_index + dc;
            },
            '!' => {
                    if char_iter.peek().map_or(false, |&(_,&d)| d == '[') {
                        add_text_to_stack(&mut delimit_stack,text_begin, char_index);
                        char_iter.next();
                        delimit_stack.push_back(char_index, ImgOpen);
                        text_begin = char_index + 2;
                    }
                }
            '[' => {
                add_text_to_stack(&mut delimit_stack, text_begin, char_index);
                delimit_stack.push_back(char_index, LinkOpen(true));
                text_begin = char_index + 1;
            }
            ']' => {
                add_text_to_stack(&mut delimit_stack, text_begin, char_index);
                delimit_stack.push_back(char_index, BrackClose);
                text_begin = char_index + 1;
            },
            '<' => {
                add_text_to_stack(&mut delimit_stack, text_begin, char_index);
                delimit_stack.push_back(char_index, AngleOpen);
                text_begin = char_index + 1;
            },
            '>' => {
                // try to make autolink or HTML tag.
                // search through opening tags.
                // if fail, go forward to next < and try again.
                let mut next_node = delimit_stack.get_first();
                let mut match_found = None;
                while let Some(dllnode) = next_node {
                    if let AngleOpen = dllnode.inline_component {
                        // try to make it either autolink or Raw HTML
                        let start_char_index = dllnode.beginning_char_index;
                        let index_of_matching = dllnode.index_of_this;
                        if let Some((chars_eaten, is_email)) = parse_autolink(dbg!(&chars[start_char_index..char_index+1])) {
                            if start_char_index + chars_eaten == char_index + 1 {
                                match_found = Some((index_of_matching, AutoLink((start_char_index + 1, char_index), is_email)));
                                break;
                            }
                        } else if let Some(chars_eaten) = dbg!(parse_html_tag(&chars[start_char_index..char_index+1])) {
                            if dbg!(start_char_index) + chars_eaten == dbg!(char_index + 1) {
                                match_found = Some((index_of_matching, HTMLTag((start_char_index, char_index + 1))));
                                break;
                            }
                        }
                    }
                    next_node = delimit_stack.get_next(dllnode);
                }
                if let Some((matching_bracket_index, ic_found)) = dbg!(match_found) {
                    let begin_char = delimit_stack.get(matching_bracket_index).beginning_char_index;
                    delimit_stack.delete_stack_above_including(matching_bracket_index);
                    delimit_stack.push_back(begin_char, CompletedContent(ic_found));
                } else {
                    add_text_to_stack(&mut delimit_stack, text_begin, char_index);
                    delimit_stack.push_back(char_index, AngleClose);
                }
                text_begin = char_index + 1;
                //todo!()
            }
            _ => {()
                // let _ = match c {
                //     ']' => Some(BrackClose),
                //     '<' => Some(AngleOpen),
                //     '>' => Some(AngleClose),
                //     _ => None
                // }.map(|dl| {
                //     add_text_to_stack(&mut delimit_stack,text_begin, char_index);
                //     delimit_stack.push_back(char_index, dl)});
            },
        }

    }
    add_text_to_stack(&mut delimit_stack, text_begin, chars.len() - space_count);

    // Now we can process links here.
    // for emph processing, "Set" the stack top to the closing bracket
    // then reset it once an item has been returned.

    // for dl_node in delimit_stack.iter() {
    //     dbg!(dl_node);
    // }
    // dbg!(&delimit_stack);
    // dbg!(&delimit_stack);
    // now at end of line we look through our stacks
    process_emphasis(None, &mut delimit_stack)
}

// This can come next (since links have lower priority than code spans, autolinks, and raw html tags)
fn process_links(stack: &mut FakeDelimiterDLL) -> Vec<InlineContent> {

    process_emphasis(None, stack)
}
// fsub: forward search upper bound
// boolean tells caller if it contains a link, deepest nested link has priority
fn process_emphasis(stack_bottom:Option<usize>, stack:&mut  FakeDelimiterDLL) -> Vec<InlineContent>{
    // dbg!("processing emph");
    let mut current_index_op = stack_bottom.map_or(stack.initial_index, 
        |i|{
            stack.get(i).index_of_next
        });
    let stack_bottom_char_index = stack_bottom.map(
        |dll_pointer| {
            stack.get(dll_pointer).beginning_char_index
        }
        );
    // only set op_bot when we find a non_matched closer_delimiter
    // set it as the *CHARACTER_OFFSET* in the corresponding vec of chars.
    let mut openers_bottom_asts_and_opening: [Option<usize >;3] = [stack_bottom_char_index;3];
    let mut openers_bottom_asts_not_opening: [Option<usize >;3] = [stack_bottom_char_index;3];
    let mut openers_bottom_unds_and_opening: [Option<usize >;3] = [stack_bottom_char_index;3];
    let mut openers_bottom_unds_not_opening: [Option<usize >;3] = [stack_bottom_char_index;3];
    // index_in_fakedldll, char_offset it points to
    let mut unds_op_stack:Vec<(usize,usize) > = vec![];
    let mut asts_op_stack: Vec<(usize,usize)> = vec![];

    while let Some(current_index) = current_index_op {
        let current_dllnode = stack.get(current_index);
        let current_beginning_char_index = current_dllnode.beginning_char_index;
        match current_dllnode.inline_component {
            Asts(total_count, consumed, pot_op, pot_clos) => {
                dbg!(&asts_op_stack);
                if pot_clos && !asts_op_stack.is_empty(){
                    let mut asts_op_stack_offset = asts_op_stack.len() - 1; 
                    let this_op_bottom = &mut
                        if pot_op {
                            openers_bottom_asts_and_opening[total_count % 3]
                        } else {
                            openers_bottom_asts_not_opening[total_count % 3]
                        };
                    let mut matching_dl_index_op = None;
                    while asts_op_stack_offset >= 0 && this_op_bottom.map_or(true, |x| asts_op_stack[asts_op_stack_offset].1 > x){
                        if let Asts(op_tc, op_used, true, op_pc) = (stack.get(asts_op_stack[asts_op_stack_offset].0).inline_component) {
                            if ((op_pc || pot_op)) && (total_count + op_tc) % 3 == 0 && !(total_count %3 == 0 && op_tc %3 == 0) {
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
                            Asts(ot, ou,..) => (ot, ou),
                            _ => panic!("should be Asts here")
                        };
                        let matching_dl_unconsumed = op_tc - op_used;
                        let this_dl_unconsumed = total_count - consumed;
                        let is_strong = matching_dl_unconsumed >=2 && this_dl_unconsumed >= 2;
                        // turn stack items inside the stack delimiters into actual inline content.
                        let mut emph_children: Vec<InlineContent> = vec![];
                        let mut node_to_eat_index_op = stack.get(matching_dl_index).index_of_next;
                        while node_to_eat_index_op.map_or(false, |ntei|stack.get(ntei).beginning_char_index < current_beginning_char_index) {
                            let nte = stack.get_mut(node_to_eat_index_op.unwrap());
                            emph_children.push(nte.inline_component.to_inline_content(nte.beginning_char_index));
                            node_to_eat_index_op = nte.index_of_next;
                        }
                        // also clear out any delimiters on the stacks weve made in this function
                        while let Some((dll_index, opener_char_index)) = unds_op_stack.pop_if(|(_, opener_char)|
                            {*opener_char > stack.get(matching_dl_index).beginning_char_index}){}
                        while let Some((dll_index, opener_char_index)) = asts_op_stack.pop_if(|(_, opener_char)|
                            {*opener_char > stack.get(matching_dl_index).beginning_char_index}){}
                        stack.replace_inside_stack_range(matching_dl_index, 
                            current_index, stack.get(matching_dl_index).beginning_char_index + op_tc, 
                            CompletedContent(
                                if is_strong {
                                    Strong(emph_children)
                                } else {
                                    Emph(emph_children)
                                }
                                )
                            );
                        let closer_used_is_total;
                        if let Asts(total, used,..) = &mut stack.get_mut(current_index).inline_component {
                            *used += if is_strong {2} else {1};
                            closer_used_is_total = *used == *total;
                        } else {
                            panic!("expect unds");
                        }
                        if closer_used_is_total {
                            current_index_op = stack.get_next(stack.get(current_index)).map(|s|s.index_of_this);
                            stack.remove_at_index(current_index);
                        }
                        let mut opener_used_is_total = false;
                        if let Asts(total, used,..) = &mut stack.get_mut(matching_dl_index).inline_component {
                            *used += if is_strong {2} else {1};
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
                            asts_op_stack.push((current_index, stack.get(current_index).beginning_char_index));
                        }
                        // we don't need a notion of "deleting" non potential_openers Since
                        // we track backwards in a different stack than this.
                        current_index_op = stack.get(current_index).index_of_next;
                    }
                } else if pot_op {
                    asts_op_stack.push((current_index, stack.get(current_index).beginning_char_index));
                    current_index_op = stack.get(current_index).index_of_next;
                } else {
                    current_index_op = stack.get(current_index).index_of_next;
                }
            }
            Unds(total_count,consumed, pot_op, pot_clos) => {
                dbg!(&unds_op_stack);
                if pot_clos && !unds_op_stack.is_empty(){
                    let mut unds_op_stack_offset = unds_op_stack.len() - 1; 
                    let this_op_bottom = &mut
                        if pot_op {
                            openers_bottom_unds_and_opening[total_count % 3]
                        } else {
                            openers_bottom_unds_not_opening[total_count % 3]
                        };
                    let mut matching_dl_index_op = None;
                    while unds_op_stack_offset >= 0 && this_op_bottom.map_or(true, |x| unds_op_stack[unds_op_stack_offset].1 > x){
                        if let Unds(op_tc, op_used, true, op_pc) = stack.get(unds_op_stack[unds_op_stack_offset].0).inline_component {
                            if (op_pc || pot_op) && (total_count + op_tc % 3 == 0 && !(total_count %3 == 0 && op_tc %3 == 0)) {
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
                            Unds(ot, ou,..) => (ot, ou),
                            _ => panic!("should be Unds here")
                        };
                        let matching_dl_unconsumed = op_tc - op_used;
                        let this_dl_unconsumed = total_count - consumed;
                        let is_strong = matching_dl_unconsumed >=2 && this_dl_unconsumed >= 2;
                        // turn stack items inside the stack delimiters into actual inline content.
                        let mut emph_children: Vec<InlineContent> = vec![];
                        let mut node_to_eat_index_op = stack.get(matching_dl_index).index_of_next;
                        while node_to_eat_index_op.map_or(false, |ntei|stack.get(ntei).beginning_char_index < current_beginning_char_index) {
                            let nte = stack.get_mut(node_to_eat_index_op.unwrap());
                            emph_children.push(nte.inline_component.to_inline_content(nte.beginning_char_index));
                            node_to_eat_index_op = nte.index_of_next;
                        }
                        // also clear out any delimiters on the stacks weve made in this function
                        while let Some((dll_index, opener_char_index)) = unds_op_stack.pop_if(|(_, opener_char)|
                            {*opener_char > stack.get(matching_dl_index).beginning_char_index}){}
                        while let Some((dll_index, opener_char_index)) = asts_op_stack.pop_if(|(_, opener_char)|
                            {*opener_char > stack.get(matching_dl_index).beginning_char_index}){}
                        stack.replace_inside_stack_range(matching_dl_index, 
                            current_index, stack.get(matching_dl_index).beginning_char_index + op_tc, 
                            CompletedContent(
                                if is_strong {
                                    Strong(emph_children)
                                } else {
                                    Emph(emph_children)
                                }
                                )
                            );
                        let closer_used_is_total;
                        if let Unds(total, used,..) = &mut stack.get_mut(current_index).inline_component {
                            *used += if is_strong {2} else {1};
                            closer_used_is_total = *used == *total;
                        } else {
                            panic!("expect unds");
                        }
                        if closer_used_is_total {
                            current_index_op = stack.get_next(stack.get(current_index)).map(|s|s.index_of_this);
                            stack.remove_at_index(current_index);
                        }
                        let mut opener_used_is_total = false;
                        if let Unds(total, used,..) = &mut stack.get_mut(matching_dl_index).inline_component {
                            *used += if is_strong {2} else {1};
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
                            unds_op_stack.push((current_index, stack.get(current_index).beginning_char_index));
                        }
                        // we don't need a notion of "deleting" non potential_openers Since
                        // we track backwards in a different stack than this.
                        current_index_op = stack.get(current_index).index_of_next;
                    }
                } else if pot_op {
                    unds_op_stack.push((current_index, stack.get(current_index).beginning_char_index));
                    current_index_op = stack.get(current_index).index_of_next;
                } else {
                    current_index_op = stack.get(current_index).index_of_next;
                }
            }
            _ =>         current_index_op = stack.dl_stack[current_index].index_of_next,
        }
    }

    // for dl_node in stack.iter() {
    //     dbg!(dl_node);
    // }
    // dbg!(asts_op_stack);
    let mut out = vec![];
    // now we can iterate through the stack above stack_bottom
    let mut ntei_op = stack_bottom.map_or(
        stack.initial_index, 
        |i| stack.get(i).index_of_next
        );
    while let Some(ntei) = ntei_op {
        let nte = stack.get_mut(ntei);
        out.push(nte.inline_component.to_inline_content(nte.beginning_char_index));
        ntei_op = nte.index_of_next;
    }
    // dbg!(&out);
    out
}
