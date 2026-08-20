use phf::ordered_set::Iter;
use serde::de;
use serde_json::Value::Array;

use crate::chars::*;
use crate::inline::InlineContent::*;
use crate::inline::InlineTextComponent::*;
use crate::inline::LinkType::{InlineLink, ReferenceLink};
use crate::peekable_char_indices::*;
use core::panic;
use std::arch::aarch64;
use std::collections::{HashMap, VecDeque};
use std::iter::Peekable;
use std::str::CharIndices;
use std::{mem, vec};

include!(concat!(env!("OUT_DIR"), "/unicode_categories.rs"));

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct Inline {
    pub chars: Vec<char>,
    pub string: String,
    pub content: Vec<InlineContent>,
}

impl Inline {
    pub fn new(chars: Vec<char>) -> Self {
        Inline {
            chars: chars,
            string: String::new(),
            content: vec![],
        }
    }

    pub fn fill_content(&mut self, lrd_table: &HashMap<Vec<char>, (Vec<char>, Vec<char>)>) {
        // dbg!("called_fill_content");
        assert!(self.content.is_empty());
        self.content = parse_inline(&self.chars, &self.string, lrd_table)
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
    //TODO: make this work with reference links
    Link((usize, usize), (usize, usize), Vec<InlineContent>),
    /// src, title, link_text
    Image((usize, usize), (usize, usize), Vec<InlineContent>),
    /// href and text, is_email
    AutoLink((usize, usize), bool),
    /// start, end char index (exclusive)
    HTMLTag((usize, usize)),
    Code((usize, usize)),
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
            }
            Emph(inline_contents) => {
                string_builder.push_str("<em>");
                for ic in inline_contents {
                    ic.to_html(characters, string_builder);
                }
                string_builder.push_str("</em>");
            }
            Strong(inline_contents) => {
                string_builder.push_str("<strong>");
                for ic in inline_contents {
                    ic.to_html(characters, string_builder);
                }
                string_builder.push_str("</strong>");
            }
            Link(_, _, inline_contents) => todo!(),
            Image(_, _, inline_contents) => todo!(),
            AutoLink((first_char, last_char), is_email) => {
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
            }
            HTMLTag((start, end)) => {
                for ci in *start..*end {
                    string_builder.push(characters[ci])
                }
            }
            Code((start, end)) => {
                dbg!(characters);
                let strip_space = " \n".contains(dbg!(characters[dbg!(*start)]))
                    && " \n".contains(dbg!(characters[dbg!(*end - 1)]));
                dbg!(&strip_space);
                let mut entirely_space = true;
                string_builder.push_str("<code>");
                let strip_start = if strip_space { *start + 1 } else { *start };
                let strip_end = if strip_space { *end - 1 } else { *end };
                for i in strip_start..strip_end {
                    if " \n".contains(characters[i]) {
                        push_html_reserved_char(' ', string_builder);
                    } else {
                        entirely_space = false;
                        push_html_reserved_char(characters[i], string_builder);
                    }
                }
                if entirely_space && strip_space {
                    if *start != *end - 1 {
                        push_html_reserved_char(' ', string_builder);
                    }
                    push_html_reserved_char(' ', string_builder);
                }
                string_builder.push_str("</code>");
            }
            Dummy => panic!("should not encounter dummy at this point"),
        }
    }
}

pub enum LinkType {
    /// optional link destination, optional link title.
    /// the char offsets are given relative to the start of text,
    /// the caller needs to adjust these to the actual char_ptr in
    /// the full chars vector.
    InlineLink(Option<(usize, usize)>, Option<(usize, usize)>),
    ReferenceLink,
    CollapsedReferenceLink,
    ShortcutReferenceLink,
}

// inline link, reference link, collapsed reference link, shortcut reference link
// These are the same for links and images. If the caller is looking to parse an image,
// they just need to set the start to the opening '[' (not the '!')
// Ok, with new refactors, makes me think it may be better to give the parsers mutable (peekable) iterators instead of
// text arrays.
// this way we can clone an iterator, give it to a parser, then on success, reassign the original
// iter to the new iter we created, and on failure simply throw it away.
// also will probably make refactoring around strings instead of char arrays easier.
//TODO: make parsers work with iters insead of char arrays.
pub fn parse_link(
    text: &[char],
    mut char_iter: &mut PeekableCharIndices,
) -> Option<(usize, LinkType)> {
    // first get a link label.
    let offset_after_link_lab = parse_link_label(char_iter)?;
    if char_iter.next_if(|(_, c)| c == '(').is_some() {
        // try to parse an inline link
        //optional link destination
        let mut link_des_out = None;
        let mut link_title_out = None;
        let mut current_offset = char_iter.offset();
        if current_offset < text.len()
            && let Some((link_des_chars_eaten, in_bracks)) = parse_link_destination(char_iter)
        {
            if in_bracks {
                link_des_out = Some((current_offset + 1, current_offset + link_des_chars_eaten))
            } else {
                link_des_out = Some((current_offset, current_offset + link_des_chars_eaten + 1))
            }

            //go through whitespace
            current_offset += link_des_chars_eaten;
            while current_offset < text.len() && " \t\n".contains(text[current_offset]) {
                current_offset += 1;
            }

            // link title is dependent on link destination existing.
            if current_offset < text.len()
                && let Some(link_tit_chars_eaten) = parse_link_title(&text[current_offset..])
            {
                link_title_out = Some((current_offset + 1, current_offset + link_tit_chars_eaten));
                current_offset += link_tit_chars_eaten;
            }
        }
        if current_offset < text.len() && text[current_offset] == ')' {
            return Some((current_offset, InlineLink(link_des_out, link_title_out)));
        } else {
            return Some((offset_after_link_lab, ReferenceLink));
        }
    }

    None
}

pub fn parse_link_label(char_iter: &mut PeekableCharIndices) -> Option<usize> {
    let (_, c) = char_iter.next()?;
    if c != '[' {
        return None;
    }

    let mut non_space_encountered = false;
    let mut char_count = 0;
    while char_count <= 1000
        && let Some((ci, c)) = char_iter.next()
    {
        non_space_encountered |= !c.is_whitespace();
        match c {
            '\\' => {
                char_count += 2;
                char_iter.next();
            }
            '[' => return None,
            ']' => {
                if non_space_encountered {
                    return Some(ci);
                } else {
                    return None;
                }
            }
            _ => char_count += 2,
        }
    }
    None
}

/*
 * returns Some(k) if text matches [.*(c| c != " \t\n")*.*]
 * to get the link label, from the return value, get text[1..k-1]*/
// pub fn parse_link_label(text: &[char]) -> Option<usize> {
//     let mut offset = 0;
//     if text.len() <= offset || text[offset] != '[' {
//         return None;
//     }
//     offset += 1;
//     let mut non_space_encountered = false;
//
//     while offset < text.len() && offset <= 1000 && text[offset] != ']' {
//         if !text[offset].is_whitespace() {
//             non_space_encountered = true
//         }
//         if text[offset] == '\\' {
//             offset += 1;
//         } else if text[offset] == '[' {
//             return None;
//         }
//         offset += 1;
//     }
//
//     if offset < text.len() && offset <= 1000 && non_space_encountered {
//         return Some(offset + 1);
//     }
//
//     None
// }

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

pub fn parse_link_destination(char_iter: &mut PeekableCharIndices) -> Option<(usize, bool)> {
    let (_, c_0) = char_iter.peek()?;
    if c_0.is_whitespace() || c_0.is_ascii_control() {
        return None;
    }

    if c_0 == '<' {
        char_iter.next();
        while let Some((ci, c)) = char_iter.next() {
            if c == '\\' {
                char_iter.next();
            } else if c == '<' {
                return None;
            } else if c == '>' {
                return Some((ci, true));
            }
        }
        return None;
    }

    let mut paren_stack = 0;
    while let Some((ci, c)) = char_iter.next() {
        if c == '(' {
            paren_stack += 1;
        } else if c == ')' {
            if paren_stack > 0 {
                paren_stack -= 1;
            } else {
                return None;
            }
        } else if c == '\\'
            && let Some((_, cpbs)) = char_iter.peek()
        {
            if cpbs.is_whitespace() || cpbs.is_ascii_control() {
                return Some((ci, false));
            }
            char_iter.next();
        }
        if c.is_whitespace() || c.is_ascii_control() {
            return Some((ci, false));
        }
    }
    None
}
/*
 * returns Some(k, bool) if the first k chars of text matches link definition
 * returns true if the chars are in <> and false if they are not
 */
// pub fn parse_link_destination(text: &[char]) -> Option<(usize, bool)> {
//     let mut offset = 0;
//     if text.len() <= offset {
//         return None;
//     }
//
//     if text[offset] == '<' {
//         offset += 1;
//         while offset < text.len() && text[offset] != '>' {
//             if text[offset] == '\\' {
//                 offset += 2;
//             } else if text[offset] == '<' {
//                 return None;
//             } else {
//                 offset += 1;
//             }
//         }
//         if offset < text.len() {
//             return Some((offset, true));
//         }
//         return None;
//     }
//
//     if text[offset].is_whitespace() || text[offset].is_ascii_control() {
//         return None;
//     }
//     let mut paren_stack = 0;
//     while offset < text.len()
//         && !(text[offset].is_whitespace())
//         && !(text[offset].is_ascii_control())
//     {
//         if text[offset] == '(' {
//             paren_stack += 1;
//         } else if text[offset] == ')' {
//             if paren_stack > 0 {
//                 paren_stack -= 1;
//             } else {
//                 return None;
//             }
//         } else if text[offset] == '\\'
//             && text.len() > offset + 1
//             && !text[offset + 1].is_whitespace()
//         {
//             offset += 1
//         }
//         offset += 1;
//     }
//
//     if paren_stack == 0 {
//         return Some((offset, false));
//     }
//     None
// }

// to get the actual text from the link title return Some(x).
// it is text[1..x] (gets rid of delimiters)
pub fn parse_link_title(char_iter: &mut PeekableCharIndices) -> Option<usize> {
    let (_, c_0) = char_iter.next()?;

    if !"\"\'(".contains(c_0) {
        return None;
    }
    let matching_delimiter = if c_0 == '(' { ')' } else { c_0 };
    while let Some((_, c)) = char_iter.next_if(|(_, c)| c != matching_delimiter) {
        if c == c_0 {
            return None;
        }
        if c == '\\' {
            char_iter.next();
        }
    }
    let (_, c_closing) = char_iter.next()?;
    if c_closing == matching_delimiter {
        return Some(char_iter.offset());
    }
    None
}

//For purposes of this spec, a scheme is any sequence of 2–32 characters beginning with an ASCII letter and followed by any combination
//of ASCII letters, digits, or the symbols plus (“+”), period (“.”), or hyphen (“-”).
//returns Some(x) if the first x characters of the text are a scheme, otherwise None.
pub fn parse_scheme(char_iter: &mut PeekableCharIndices) -> Option<usize> {
    let mut char_count = 0;
    while char_count <= 32
        && char_iter
            .next_if(|(_, c)| c.is_ascii_alphanumeric() || "+.-".contains(c))
            .is_some()
    {
        char_count += 1;
    }
    if 2 <= char_count && char_count <= 32 {
        return Some(char_iter.offset());
    }
    None
}

//For purposes of this spec, a scheme is any sequence of 2–32 characters beginning with an ASCII letter and followed by any combination
//of ASCII letters, digits, or the symbols plus (“+”), period (“.”), or hyphen (“-”).
//returns Some(x) if the first x characters of the text are a scheme, otherwise None.
// pub fn parse_scheme(text: &[char]) -> Option<usize> {
//     let mut char_count = 0;
//     while char_count < text.len() && char_count <= 32 && (text[char_count].is_ascii_alphanumeric() || "+.-".contains(text[char_count])) {
//         char_count += 1;
//     }
//     if 2 <= char_count && char_count <= 32 {
//         return Some(char_count);
//     }
//     None
// }

pub fn parse_uri_autolink(char_iter: &mut PeekableCharIndices) -> Option<usize> {
    // let (_, c_0) = char_iter.next()?;
    // if c_0 != '<' {
    //     return None;
    // }

    parse_scheme(char_iter)?;
    let (_, c_after_scheme) = char_iter.next()?;
    if c_after_scheme != ':' {
        return None;
    }

    while let Some((ci, c)) = char_iter.next() {
        if c.is_ascii_control() || " \n<".contains(c) {
            return None;
        }
        if c == '>' {
            return Some(ci);
        }
    }
    None
}

// pub fn parse_uri_autolink(text: &[char]) -> Option<usize> {
//     let mut offset = 0;
//     if text.len() > 0 && text[offset] == '<'{
//         offset += 1;
//     } else {
//         return None;
//     }
//     if let Some(scheme_char_count) = parse_scheme(&text[offset..]) {
//         if scheme_char_count + 1 < text.len() && text[scheme_char_count + 1] == ':' {
//             offset += scheme_char_count + 2;
//         } else {
//             return None;
//         }
//     } else {
//         return None;
//     }
//     while offset < text.len() && !text[offset].is_ascii_control() && !(" \n<>".contains(text[offset])) {
//         offset += 1;
//     }
//     if offset < text.len() && text[offset] == '>' {
//         return Some(offset + 1);
//     }
//     None
// }

//An email address, for these purposes, is anything that matches the non-normative regex from the HTML5 spec:
// /^[a-zA-Z0-9.!#$%&'*+/=?^_`{|}~-]+@[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?
// (?:\.[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)*$/
pub fn parse_email(char_iter: &mut PeekableCharIndices) -> Option<usize> {
    // let (_, c_0) = char_iter.next()?;
    // if c_0 != '<' {
    //     return None;
    // }

    char_iter
        .consume_while(|(_, c)| c.is_ascii_alphanumeric() || ".!#$%&'*+/=?^_`{|}~-".contains(c));

    let (_, c_0) = char_iter.next()?;
    if c_0 != '@' {
        return None;
    }

    let parse_label = |char_iter: &mut PeekableCharIndices| {
        let (_, c_0) = char_iter.next()?;
        if !c_0.is_ascii_alphanumeric() {
            return None;
        }
        let mut char_count = 1;
        let mut last_is_hyphen = false;
        while let Some((_, c)) = char_iter.next_if(|(_, c)| c.is_ascii_alphanumeric() || c == '-')
            && char_count <= 63
        {
            last_is_hyphen = c == '-';
            char_count += 1;
        }
        if char_count <= 63 && !last_is_hyphen {
            return Some(char_iter.offset());
        }
        None
    };
    parse_label(char_iter)?;
    while char_iter.next_if(|(_, c)| c == '.').is_some() {
        parse_label(char_iter)?;
    }

    char_iter.next_if(|(_, c)| c == '>').map(|(i, _)| i)
}
// pub fn parse_email(text: &[char]) -> Option<usize> {
//     let is_atext_or_dot = |c: char| {
//         c.is_ascii_alphanumeric() || ".!#$%&'*+/=?^_`{|}~-".contains(c)
//     };
//     let mut offset = 0;
//     if text.len() > 0 && text[offset] == '<'{
//         offset += 1;
//     } else {
//         return None;
//     }
//     while offset < text.len() && is_atext_or_dot(text[offset]) {
//         offset += 1;
//     }
//     if offset >= 1 && offset < text.len() && text[offset] == '@' {
//         offset += 1;
//     } else {
//         return None;
//     }
//
//     let parse_label = |text: &[char]| {
//         let mut char_count = 0;
//         if char_count < text.len() && text[char_count].is_ascii_alphanumeric() {
//             char_count += 1;
//         } else {
//             return None;
//         };
//         let mut last_is_hyphen = false;
//         while char_count < text.len() && char_count <= 63 && text[char_count].is_ascii_alphanumeric() || text[char_count] == '-' {
//             last_is_hyphen = text[char_count] == '-';
//             char_count +=1;
//         }
//         if char_count <=63 && !last_is_hyphen {
//             return Some(char_count);
//         }
//         None
//     };
//
//     if let Some(chars_eaten) = parse_label(&text[offset..]) {
//         offset += chars_eaten
//     } else {
//         return None;
//     }
//     while offset < text.len() && text[offset] == '.' {
//         offset += 1;
//         if let Some(chars_eaten) = parse_label(&text[offset..]) {
//             offset += chars_eaten
//         } else {
//             return None;
//         }
//     }
//     if offset < text.len() && text[offset] == '>' {
//         return Some(offset + 1);
//     }
//     None
// }

pub fn parse_autolink(char_iter: &mut PeekableCharIndices) -> Option<(usize, bool)> {
    let mut al_iter = char_iter.clone();
    let mut em_iter = char_iter.clone();
    if let Some(index_of_last_char) = parse_uri_autolink(&mut al_iter) {
        *char_iter = al_iter;
        return Some((index_of_last_char, false));
    } else if let Some(index_of_last_char) = parse_email(&mut em_iter) {
        *char_iter = em_iter;
        return Some((index_of_last_char, true));
    } else {
        return None;
    }
}

pub fn parse_html_tag(char_iter: &mut PeekableCharIndices) -> Option<usize> {
    // let (_, c_0) = char_iter.next()?;
    // if c_0 != '<' {
    //     return None;
    // }

    return match char_iter.next()?.1 {
        '/' => parse_closing_tag(char_iter),
        '?' => parse_processing_instruction(char_iter),
        '!' => match char_iter.peek()?.1 {
            '-' => parse_html_comment(char_iter),
            '[' => parse_cdata_section(char_iter),
            _ => parse_declaration(char_iter),
        },
        c => {
            if c.is_ascii_alphabetic() {
                parse_opening_tag(char_iter)
            } else {
                None
            }
        }
    };
}

// These functions are called based on lookahead by callers, so opening brackets are consumed
// closing brackets are not though.
pub fn parse_opening_tag(char_iter: &mut PeekableCharIndices) -> Option<usize> {
    // this check is made by caller
    // let (_,c) = char_iter.next()?;
    // if !c.is_ascii_alphabetic() {return None;}
    char_iter.consume_while(|(_, c)| c.is_ascii_alphanumeric() || c == '-');

    while parse_attribute(char_iter).is_some() {}

    char_iter.consume_while(|(_, c)| " \t\n".contains(c));

    char_iter.next_if(|(_, c)| c == '/');

    char_iter
        .next_if(|(_, c)| c == '>')
        .map(|_| char_iter.offset())
}

pub fn parse_closing_tag(char_iter: &mut PeekableCharIndices) -> Option<usize> {
    let (_, c_0) = char_iter.next()?;
    if !c_0.is_ascii_alphabetic() {
        return None;
    }
    char_iter.consume_while(|(_, c)| c.is_ascii_alphanumeric() || c == '-');

    char_iter.consume_while(|(_, c)| " \t\n".contains(c));

    char_iter
        .next_if(|(_, c)| c == '>')
        .map(|_| char_iter.offset())
}

pub fn parse_attribute(char_iter: &mut PeekableCharIndices) -> Option<usize> {
    let mut ws_seen = false;
    //initial_white_space
    while char_iter.next_if(|(_, c)| " \t\n".contains(c)).is_some() {
        ws_seen = true;
    }
    //attribute name
    //must have one or more space seperating attributes.
    let (_, c_post_ws) = char_iter.next()?;
    if !ws_seen && !(c_post_ws.is_ascii_alphabetic() || "_:".contains(c_post_ws)) {
        return None;
    }

    char_iter.consume_while(|(_, c)| "_.:-".contains(c) || c.is_ascii_alphanumeric());
    //state of iter after reading name
    let pre_val_iter = char_iter.clone();

    // attribute value specification.
    char_iter.consume_while(|(_, c)| " \t\n".contains(c));

    if !char_iter.next_if(|(_, c)| c == '=').is_some() {
        *char_iter = pre_val_iter;
        return Some(char_iter.offset());
    }

    // post equals ws
    char_iter.consume_while(|(_, c)| " \t\n".contains(c));
    let (_, first_c_of_val) = char_iter.next()?;
    //quoted attribute
    if "\"\'".contains(first_c_of_val) {
        while let Some((ci, c)) = char_iter.next() {
            if c == first_c_of_val {
                return Some(ci);
            }
        }
        *char_iter = pre_val_iter;
        return Some(char_iter.offset());
    }
    //unquoted attribute
    while char_iter
        .peek()
        .is_some_and(|(_, c)| !" \t\n\"\'=<>`".contains(c))
    {
        char_iter.next();
    }
    Some(char_iter.offset())
}
// pub fn parse_attribute (text: &[char]) -> Option<usize> {
//     let mut offset = 0;
//     //initial_white_space
//     while " \t\n".contains(text[offset]) {
//         offset += 1;
//     }
//     //attribute name
//     //must have one or more space seperating attributes.
//     if offset >= text.len()
//         || offset == 0
//         || !("_:".contains(text[offset]) || text[offset].is_alphabetic())
//     // must start with letter, _, or :
//     {
//         return None;
//     }
//     offset += 1;
//     while text.len() > offset
//         && ("_.:-".contains(text[offset]) || text[offset].is_ascii_alphanumeric())
//     {
//         offset += 1;
//     }
//     let pre_val_offset = offset;
//
//     // attribute value specification.
//     while text.len() > offset && " \t".contains(text[offset]) {
//         offset += 1;
//     }
//     if text.len() <= offset || text[offset] != '=' {
//         return Some(pre_val_offset);
//     }
//     offset += 1;
//     while text.len() > offset && " \t".contains(text[offset]) {
//         offset += 1;
//     }
//     if text.len() <= offset {
//         return Some(pre_val_offset);
//     }
//     let c = text[offset];
//     //quoted attribute
//     if "\"\'".contains(c) {
//         offset += 1;
//         while text.len() > offset && text[offset] != c {
//             offset += 1;
//         }
//         offset += 1;
//         return Some(offset);
//     }
//     //unquoted attribute
//     while text.len() > offset && !" \t\n\"\'=<>`".contains(text[offset]) {
//         offset += 1;
//     }
//     Some(offset)
// }

pub fn parse_html_comment(char_iter: &mut PeekableCharIndices) -> Option<usize> {
    //consume the opening "--"
    let (_, c_0) = char_iter.next()?;
    let (_, c_1) = char_iter.next()?;
    if c_0 != '-' || c_1 != '-' {
        return None;
    }

    let mut dash_count = 0;
    let (index_0, char_0) = char_iter.next()?;
    if char_0 == '>' {
        return Some(index_0);
    } else if char_0 == '-' {
        dash_count += 1;
        let (_, char_1) = char_iter.next()?;
        if char_1 == '>' {
            return Some(char_iter.offset());
        } else if char_1 == '-' {
            dash_count += 1;
        }
    }
    while let Some((_, c)) = char_iter.next() {
        if c == '-' {
            dash_count += 1;
        } else if c == '>' && dash_count >= 2 {
            return Some(char_iter.offset());
        } else {
            dash_count = 0;
        }
    }
    None
}

pub fn parse_processing_instruction(char_iter: &mut PeekableCharIndices) -> Option<usize> {
    let mut seen_qm = false;
    while let Some((index, c)) = char_iter.next() {
        if c == '?' {
            seen_qm = true;
        } else if c == '>' && seen_qm {
            return Some(index);
        } else {
            seen_qm = false;
        }
    }
    None
}

pub fn parse_declaration(char_iter: &mut PeekableCharIndices) -> Option<usize> {
    let (_, char_0) = char_iter.next()?;
    if !char_0.is_ascii_alphabetic() {
        return None;
    }
    while let Some((index, c)) = char_iter.next() {
        if c == '>' {
            return Some(index);
        }
    }
    None
}

pub fn parse_cdata_section(char_iter: &mut PeekableCharIndices) -> Option<usize> {
    // also do the "CDATA[" string recognition.
    let to_match = "[CDATA[";
    while let Some(((_, c_in), c)) = char_iter.take(to_match.len()).zip(to_match.chars()).next() {
        if c != c_in {
            return None;
        }
    }

    let mut r_brack_count = 0;
    while let Some((current_ind, c)) = char_iter.next() {
        if c == ']' {
            r_brack_count += 1;
        } else if c == '>' && r_brack_count >= 2 {
            return Some(current_ind);
        } else {
            r_brack_count = 0;
        }
    }
    None
}

#[derive(Debug, PartialEq, Eq, Clone)]
enum InlineTextComponent {
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
}

impl InlineTextComponent {
    fn to_text_comp(self) -> InlineTextComponent {
        match self {
            Unds(total, consumed, ..) | Asts(total, consumed, ..) => {
                TextualContent(total - consumed)
            }
            BackTick(count) => TextualContent(count),
            ImgOpen(_) => TextualContent(2),
            TextualContent(_) | CompletedContent(_) => self,
            _ => TextualContent(1),
        }
    }
    fn to_inline_content(&mut self, char_offset: usize) -> InlineContent {
        match self {
            Unds(total, consumed, ..) | Asts(total, consumed, ..) => {
                Text(char_offset, char_offset + (*total - *consumed))
            }
            BackTick(count) | TextualContent(count) => Text(char_offset, char_offset + *count),
            ImgOpen(_) => Text(char_offset, char_offset + 2),
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
    //this indexes into the string at the start of a char, the design of the program should
    //guarantee that this doesnt panic. Namely by only considering usizes that come
    //immediately from a char_indices() and only subtracting from that offset when the character before that is known.
    beginning_char_index: usize,
    inline_component: InlineTextComponent,
    index_of_prev: Option<usize>,
    index_of_next: Option<usize>,
    index_of_this: usize, // this is helpful for getting around borrow checker shenanigans
}

impl DLLnode {
    fn new(
        begin_index: usize,
        component: InlineTextComponent,
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
    fn push_back(&mut self, begin_index: usize, dl: InlineTextComponent) {
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

    fn get_first_mut(&mut self) -> Option<&mut DLLnode> {
        self.initial_index.map(|i| &mut self.dl_stack[i])
    }

    fn get_last(&self) -> Option<&DLLnode> {
        self.final_index.map(|i| &self.dl_stack[i])
    }

    fn get_last_mut(&mut self) -> Option<&mut DLLnode> {
        self.final_index.map(|i| &mut self.dl_stack[i])
    }

    fn get(&self, index: usize) -> &DLLnode {
        &self.dl_stack[index]
    }

    fn get_mut(&mut self, index: usize) -> &mut DLLnode {
        &mut self.dl_stack[index]
    }

    fn get_next(&self, node: &DLLnode) -> Option<&DLLnode> {
        node.index_of_next.map(|i| &self.dl_stack[i])
    }

    fn get_next_mut(&mut self, node: &DLLnode) -> Option<&mut DLLnode> {
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
        item: InlineTextComponent,
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
        item: InlineTextComponent,
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
    collection: &'a FakeDelimiterDLL,
}

impl<'a> Iterator for FakeDLLIter<'a> {
    type Item = &'a DLLnode;

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
pub fn parse_inline(
    chars: &[char],
    inline_str: &str,
    lrd_table: &HashMap<Vec<char>, (Vec<char>, Vec<char>)>,
) -> Vec<InlineContent> {
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
    while let Some((char_index, c)) = char_iter.next() {
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
                if let Some((_, c)) = char_iter.peek() {
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
                add_text_to_stack(&mut delimit_stack, text_begin, char_index);

                char_iter.consume_while_char_eq('`');
                let initial_tick_count = char_iter.offset() - char_index;

                let mut matching_tick_count_search_iter = char_iter.clone();
                let mut code_completed = false;
                'search_for_matching_tc: while let Some((start_pos, c)) =
                    matching_tick_count_search_iter.next()
                {
                    if c == '`' {
                        matching_tick_count_search_iter.consume_while_char_eq('`');
                        let closing_tick_count =
                            matching_tick_count_search_iter.offset() - start_pos;

                        if closing_tick_count == initial_tick_count {
                            char_iter = matching_tick_count_search_iter;
                            delimit_stack.push_back(
                                char_index,
                                CompletedContent(Code((
                                    char_index + initial_tick_count,
                                    start_pos,
                                ))),
                            );
                            code_completed = true;
                            text_begin = start_pos + closing_tick_count;
                            break 'search_for_matching_tc;
                        }
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
                let following_char = char_iter.peek().unwrap_or((0, ' ')).1;

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
                delimit_stack.push_back(char_index, BrackClose);
                text_begin = char_index + 1;
            }
            '<' => {
                //eagerly try to make autolink or html,
                dbg!("trying to make new angle bracket thing");
                dbg!(&char_index);
                add_text_to_stack(&mut delimit_stack, text_begin, char_index);
                let mut autolink_iter = char_iter.clone();
                let mut html_iter = char_iter.clone();
                if let Some((chars_eaten, is_email)) = parse_autolink(&mut autolink_iter) {
                    delimit_stack.push_back(
                        char_index,
                        CompletedContent(AutoLink(
                            (char_index + 1, char_index + chars_eaten - 1),
                            is_email,
                        )),
                    );
                    char_iter = autolink_iter;
                    text_begin = char_iter.offset();
                    preceding_char = '>'
                } else if let Some(chars_eaten) = parse_html_tag(&mut html_iter) {
                    delimit_stack.push_back(
                        char_index,
                        CompletedContent(HTMLTag((char_index, char_index + chars_eaten))),
                    );
                    char_iter = html_iter;
                    text_begin = char_index + chars_eaten;
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
    add_text_to_stack(&mut delimit_stack, text_begin, chars.len() - space_count);

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
    // dbg!(&out);
    out
}
