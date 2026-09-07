//!  The contract for calling parsers is generally like so:
//!  - the callee can modify the incoming iterator whether it succeeds or fails. This means that it is
//!    the callers responsibility to clone the iterator and "backtrack" in the case of failure.

use crate::{
    parsers::ReferenceLinkType::{Collapsed, Full},
    peekable_char_indices::*,
};
use std::ops::Range;

pub type InlineLinkSuffix<T> = (Option<Range<T>>, Option<Range<T>>);
pub fn parse_inline_suffix<T: Eq>(
    char_iter: &mut impl PeekableCharIndices<Offset = T>,
) -> Option<InlineLinkSuffix<T>> {
    char_iter.next_if_char_eq('(')?;

    char_iter.consume_while(|c| " \t\n".contains(c));
    if char_iter.next_if_char_eq(')').is_some() {
        return Some((None, None));
    }

    let dest_range = Some(parse_link_destination(char_iter)?);
    let post_dest_offset = char_iter.offset();
    char_iter.consume_while(|c| " \t\n".contains(c));

    if char_iter.next_if_char_eq(')').is_some() {
        return Some((dest_range, None));
    }

    //dest and title must have spaces seperating them
    if char_iter.offset() == post_dest_offset {
        return None;
    }

    let tit_range = Some(parse_link_title(char_iter)?);

    char_iter.consume_while(|c| " \t\n".contains(c));

    char_iter.next_if_char_eq(')')?;

    Some((dest_range, tit_range))
}

#[derive(Debug)]
pub enum ReferenceLinkType<T> {
    Full(Range<T>),
    Collapsed,
}

pub fn parse_reference_link<T>(
    char_iter: &mut impl PeekableCharIndices<Offset = T>,
) -> Option<ReferenceLinkType<T>> {
    let mut check_for_collapsed_iter = char_iter.clone();
    if check_for_collapsed_iter.next_if_char_eq('[').is_some()
        && check_for_collapsed_iter.next_if_char_eq(']').is_some()
    {
        *char_iter = check_for_collapsed_iter;
        return Some(Collapsed);
    }
    let lab_range = parse_link_label(char_iter)?;
    Some(Full(lab_range))
}

pub fn parse_link_label<T>(
    char_iter: &mut impl PeekableCharIndices<Offset = T>,
) -> Option<Range<T>> {
    // let (_, c) = char_iter.next()?;
    // if c != '[' {
    //     return None;
    // }
    let range_bottom = char_iter.offset();
    char_iter.next_if_char_eq('[')?;

    let mut non_space_encountered = false;
    let mut char_count = 0;
    while char_count <= 1000
        && let Some(c) = char_iter.next()
    {
        match c {
            '\\' => {
                char_count += 2;
                char_iter.next();
            }
            '[' => return None,
            ']' => {
                if non_space_encountered {
                    return Some(range_bottom..char_iter.offset());
                } else {
                    return None;
                }
            }
            _ => char_count += 1,
        }
        non_space_encountered |= !c.is_whitespace();
    }
    None
}

pub fn parse_link_destination<T>(
    char_iter: &mut impl PeekableCharIndices<Offset = T>,
) -> Option<Range<T>> {
    let c_0 = char_iter.peek()?;
    if c_0.is_whitespace() || c_0.is_ascii_control() {
        return None;
    }
    let mut range_bottom = char_iter.offset();

    if c_0 == '<' {
        char_iter.next();
        range_bottom = char_iter.offset();
        while let Some((ci, c)) = char_iter.next_and_index() {
            if c == '\\' {
                if matches!(char_iter.peek(), Some('\n')) {
                    return None;
                }
                char_iter.next();
            } else if "<\n".contains(c) {
                return None;
            } else if c == '>' {
                return Some(range_bottom..ci);
            }
        }
        return None;
    }

    let mut paren_stack = 0;
    while let Some(c) = char_iter
        .next_if(|c| !" \t".contains(c) && !c.is_ascii_control() && (c != ')' || paren_stack > 0))
    {
        if c == '(' {
            paren_stack += 1;
        } else if c == ')' {
            paren_stack -= 1;
            // if paren_stack > 0 {
            //     paren_stack -= 1;
            // } else {
            //     panic!()
            // }
        } else if c == '\\'
            && let Some(cpbs) = char_iter.peek()
        {
            if cpbs.is_whitespace() || cpbs.is_ascii_control() {
                return Some(range_bottom..char_iter.offset());
            }
            char_iter.next();
        }
    }
    if paren_stack == 0 {
        return Some(range_bottom..char_iter.offset());
    }
    None
}

// to get the actual text from the link title return Some(x).
// it is text[1..x] (gets rid of delimiters)
pub fn parse_link_title<T>(
    char_iter: &mut impl PeekableCharIndices<Offset = T>,
) -> Option<Range<T>> {
    let c_0 = char_iter.next()?;

    if !"\"\'(".contains(c_0) {
        return None;
    }
    let range_bottom = char_iter.offset();

    let matching_delimiter = if c_0 == '(' { ')' } else { c_0 };
    while let Some(c) = char_iter.next_if(|c| c != matching_delimiter) {
        if c == c_0 {
            return None;
        }
        if c == '\\' {
            char_iter.next();
        }
    }
    let range_top = char_iter.offset();
    let c_closing = char_iter.next()?;
    if c_closing == matching_delimiter {
        return Some(range_bottom..range_top);
    }
    None
}

//For purposes of this spec, a scheme is any sequence of 2–32 characters beginning with an ASCII letter and followed by any combination
//of ASCII letters, digits, or the symbols plus (“+”), period (“.”), or hyphen (“-”).
//returns Some(x) if the first x characters of the text are a scheme, otherwise None.
pub fn parse_scheme<T>(char_iter: &mut impl PeekableCharIndices<Offset = T>) -> Option<Range<T>> {
    let range_bottom = char_iter.offset();
    char_iter.next_if(|c| c.is_ascii_alphabetic())?;
    let mut char_count = 1;
    while char_count <= 32
        && char_iter
            .next_if(|c| c.is_ascii_alphanumeric() || "+.-".contains(c))
            .is_some()
    {
        char_count += 1;
    }
    if (2..=32).contains(&char_count) {
        return Some(range_bottom..char_iter.offset());
    }
    None
}

pub fn parse_uri_autolink<T>(
    char_iter: &mut impl PeekableCharIndices<Offset = T>,
) -> Option<Range<T>> {
    let range_bottom = char_iter.offset();
    parse_scheme(char_iter)?;
    char_iter.next_if_char_eq(':')?;

    while let Some((ci, c)) = char_iter.next_and_index() {
        if c.is_ascii_control() || " \n<".contains(c) {
            return None;
        }
        if c == '>' {
            return Some(range_bottom..ci);
        }
    }
    None
}

//An email address, for these purposes, is anything that matches the non-normative regex from the HTML5 spec:
// /^[a-zA-Z0-9.!#$%&'*+/=?^_`{|}~-]+@[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?
// (?:\.[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)*$/
pub fn parse_email<T, P: PeekableCharIndices<Offset = T>>(char_iter: &mut P) -> Option<Range<T>> {
    // let (_, c_0) = char_iter.next()?;
    // if c_0 != '<' {
    //     return None;
    // }

    let range_start = char_iter.offset();
    char_iter.consume_while(|c| c.is_ascii_alphanumeric() || ".!#$%&'*+/=?^_`{|}~-".contains(c));

    let c_0 = char_iter.next()?;
    if c_0 != '@' {
        return None;
    }

    let parse_label = |char_iter: &mut P| -> Option<T> {
        let c_0 = char_iter.next()?;
        if !c_0.is_ascii_alphanumeric() {
            return None;
        }
        let mut char_count = 1;
        let mut last_is_hyphen = false;
        while let Some(c) = char_iter.next_if(|c| c.is_ascii_alphanumeric() || c == '-')
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
    while char_iter.next_if(|c| c == '.').is_some() {
        parse_label(char_iter)?;
    }
    let range_end = char_iter.offset();

    char_iter
        .next_if(|c| c == '>')
        .map(|_| range_start..range_end)
}

pub fn parse_autolink<T>(
    char_iter: &mut impl PeekableCharIndices<Offset = T>,
) -> Option<(Range<T>, bool)> {
    let mut al_iter = char_iter.clone();
    let mut em_iter = char_iter.clone();
    if let Some(al_range) = parse_uri_autolink(&mut al_iter) {
        *char_iter = al_iter;
        Some((al_range, false))
    } else {
        let email_range = parse_email(&mut em_iter)?;
        *char_iter = em_iter;
        Some((email_range, true))
    }
}

// Returns the html tag *after* the first 2 chars,
// caller must go back 2 chars to get the "real" start
pub fn parse_html_tag<T>(char_iter: &mut impl PeekableCharIndices<Offset = T>) -> Option<Range<T>> {
    match char_iter.next()? {
        '/' => parse_closing_tag(char_iter),
        '?' => parse_processing_instruction(char_iter),
        '!' => match char_iter.peek()? {
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
    }
}

// These functions are called based on lookahead by callers, so opening brackets are consumed
// closing brackets are not though.
pub fn parse_opening_tag<T>(
    char_iter: &mut impl PeekableCharIndices<Offset = T>,
) -> Option<Range<T>> {
    let range_bottom = char_iter.offset();
    char_iter.consume_while(|c| c.is_ascii_alphanumeric() || c == '-');

    let mut attribute_iter = char_iter.clone();
    while parse_attribute(&mut attribute_iter).is_some() {
        *char_iter = attribute_iter.clone();
    }

    // (&char_iter);
    char_iter.consume_while(|c| " \t\n".contains(c));

    //optional
    char_iter.next_if_char_eq('/');

    // Not sure if this is called lazily or not
    char_iter
        .next_if_char_eq('>')
        .map(|_| range_bottom..char_iter.offset())
}

pub fn parse_closing_tag<T>(
    char_iter: &mut impl PeekableCharIndices<Offset = T>,
) -> Option<Range<T>> {
    let range_bottom = char_iter.offset();
    char_iter.consume_while(|c| c.is_ascii_alphanumeric() || c == '-');

    char_iter.consume_while(|c| " \t\n".contains(c));

    char_iter
        .next_if(|c| c == '>')
        .map(|_| range_bottom..char_iter.offset())
}

//dont actually need parsing information
pub fn parse_attribute<T>(char_iter: &mut impl PeekableCharIndices<Offset = T>) -> Option<T> {
    let mut ws_seen = false;
    //initial_white_space
    while char_iter.next_if(|c| " \t\n".contains(c)).is_some() {
        ws_seen = true;
    }
    //attribute name
    //must have one or more space seperating attributes.
    let c_post_ws = char_iter.next()?;
    if !ws_seen || !(c_post_ws.is_ascii_alphabetic() || "_:".contains(c_post_ws)) {
        return None;
    }

    char_iter.consume_while(|c| "_.:-".contains(c) || c.is_ascii_alphanumeric());
    //state of iter after reading name
    let pre_val_iter = char_iter.clone();

    // attribute value specification.
    char_iter.consume_while(|c| " \t\n".contains(c));

    if !char_iter.next_if(|c| c == '=').is_some() {
        *char_iter = pre_val_iter;
        return Some(char_iter.offset());
    }

    // post equals ws
    char_iter.consume_while(|c| " \t\n".contains(c));
    let first_c_of_val = char_iter.peek()?;
    //quoted attribute
    if "\"\'".contains(first_c_of_val) {
        char_iter.next();
        while let Some((ci, c)) = char_iter.next_and_index() {
            if c == first_c_of_val {
                // (&char_iter);
                return Some(ci);
            }
        }
        *char_iter = pre_val_iter;
        return Some(char_iter.offset());
    }
    //unquoted attribute
    char_iter.consume_while(|c| !" \t\n\"\'=<>`".contains(c));
    Some(char_iter.offset())
}

pub fn parse_html_comment<T>(
    char_iter: &mut impl PeekableCharIndices<Offset = T>,
) -> Option<Range<T>> {
    //consume the opening "--"
    let range_bottom = char_iter.offset();
    let c_0 = char_iter.next()?;
    let c_1 = char_iter.next()?;
    if c_0 != '-' || c_1 != '-' {
        return None;
    }

    let mut dash_count = 0;
    // let (index_0, char_0) = char_iter.next_and_index()?;
    if char_iter.next_if_char_eq('>').is_some() {
        return Some(range_bottom..char_iter.offset());
    }
    if char_iter.next_if_char_eq('-').is_some() {
        dash_count += 1;
        if char_iter.next_if_char_eq('>').is_some() {
            return Some(range_bottom..char_iter.offset());
        }
    }
    // if char_0 == '>' {
    //     return Some(index_0);
    // } else if char_0 == '-' {
    //     dash_count += 1;
    //     let char_1 = char_iter.next()?;
    //     if char_1 == '>' {
    //         return Some(char_iter.offset());
    //     } else if char_1 == '-' {
    //         dash_count += 1;
    //     }
    // }

    while let Some(c) = char_iter.next() {
        if c == '-' {
            dash_count += 1;
        } else if c == '>' && dash_count >= 2 {
            return Some(range_bottom..char_iter.offset());
        } else {
            dash_count = 0;
        }
    }
    None
}

pub fn parse_processing_instruction<T>(
    char_iter: &mut impl PeekableCharIndices<Offset = T>,
) -> Option<Range<T>> {
    let range_bottom = char_iter.offset();
    let mut seen_qm = false;
    while let Some(c) = char_iter.next() {
        if c == '?' {
            seen_qm = true;
        } else if c == '>' && seen_qm {
            return Some(range_bottom..char_iter.offset());
        } else {
            seen_qm = false;
        }
    }
    None
}

//offset not actually used, dont need to track range
pub fn parse_declaration<T>(
    char_iter: &mut impl PeekableCharIndices<Offset = T>,
) -> Option<Range<T>> {
    let range_bottom = char_iter.offset();
    let char_0 = char_iter.next()?;
    if !char_0.is_ascii_alphabetic() {
        return None;
    }
    while let Some(c) = char_iter.next() {
        if c == '>' {
            return Some(range_bottom..char_iter.offset());
        }
    }
    None
}

pub fn parse_cdata_section<T>(
    char_iter: &mut impl PeekableCharIndices<Offset = T>,
) -> Option<Range<T>> {
    // also do the "CDATA[" string recognition.
    let range_bottom = char_iter.offset();
    let to_match = "[CDATA[";
    // if current_iteration_iter.next_if_char_eq('[').is_none() {
    //     break 'collect_lrds;
    // }
    let zip_iter = char_iter.take(to_match.len()).zip(to_match.chars());

    for (c_in, c_to_match) in zip_iter {
        if (c_in) != (c_to_match) {
            return None;
        }
    }

    let mut r_brack_count = 0;
    while let Some(c) = char_iter.next() {
        if c == ']' {
            r_brack_count += 1;
        } else if c == '>' && r_brack_count >= 2 {
            return Some(range_bottom..char_iter.offset());
        } else {
            r_brack_count = 0;
        }
    }
    None
}
