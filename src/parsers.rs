use crate::peekable_char_indices::*;

pub fn parse_link_label(char_iter: &mut PeekableCharIndices) -> Option<usize> {
    // let (_, c) = char_iter.next()?;
    // if c != '[' {
    //     return None;
    // }

    let mut non_space_encountered = false;
    let mut char_count = 0;
    while char_count <= 1000
        && let Some((ci, c)) = char_iter.next_and_index()
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
            _ => char_count += 1,
        }
    }
    None
}

/*
 * returns Some(k) if text matches [.*(c| c != " \t\n")*.*]
 * to get the link label, from the return value, get text[1..k-1]*/
//TODO
pub fn normalize_label(label: &str) -> String {
    let mut lab_out = String::new();
    let mut char_iter = label[1..label.len() - 1].chars().peekable();
    //strip leading whitespace
    while char_iter.peek().unwrap().is_whitespace() {
        char_iter.next();
    }
    let mut seen_space = false;
    while let Some(c) = char_iter.next() {
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
    let c_0 = char_iter.peek()?;
    if c_0.is_whitespace() || c_0.is_ascii_control() {
        return None;
    }

    if c_0 == '<' {
        char_iter.next();
        while let Some((ci, c)) = char_iter.next_and_index() {
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
    while let Some(c) = char_iter.next_if(|c| !" \t".contains(c) && !c.is_ascii_control()) {
        if c == '(' {
            paren_stack += 1;
        } else if c == ')' {
            if paren_stack > 0 {
                paren_stack -= 1;
            } else {
                return None;
            }
        } else if c == '\\'
            && let Some(cpbs) = char_iter.peek()
        {
            if cpbs.is_whitespace() || cpbs.is_ascii_control() {
                return Some((char_iter.offset(), false));
            }
            char_iter.next();
        }
    }
    if paren_stack == 0 {
        return Some((char_iter.offset(), false));
    }
    None
}

// to get the actual text from the link title return Some(x).
// it is text[1..x] (gets rid of delimiters)
pub fn parse_link_title(char_iter: &mut PeekableCharIndices) -> Option<usize> {
    let c_0 = char_iter.next()?;

    if !"\"\'(".contains(c_0) {
        return None;
    }

    let matching_delimiter = if c_0 == '(' { ')' } else { c_0 };
    while let Some(c) = char_iter.next_if(|c| c != matching_delimiter) {
        if c == c_0 {
            return None;
        }
        if c == '\\' {
            char_iter.next();
        }
    }
    let c_closing = char_iter.next()?;
    if c_closing == matching_delimiter {
        return Some(char_iter.offset());
    }
    None
}

//For purposes of this spec, a scheme is any sequence of 2–32 characters beginning with an ASCII letter and followed by any combination
//of ASCII letters, digits, or the symbols plus (“+”), period (“.”), or hyphen (“-”).
//returns Some(x) if the first x characters of the text are a scheme, otherwise None.
pub fn parse_scheme(char_iter: &mut PeekableCharIndices) -> Option<usize> {
    char_iter.next_if(|c| c.is_ascii_alphabetic())?;
    let mut char_count = 1;
    while char_count <= 32
        && char_iter
            .next_if(|c| c.is_ascii_alphanumeric() || "+.-".contains(c))
            .is_some()
    {
        char_count += 1;
    }
    if 2 <= char_count && char_count <= 32 {
        return Some(char_iter.offset());
    }
    None
}

pub fn parse_uri_autolink(char_iter: &mut PeekableCharIndices) -> Option<usize> {
    parse_scheme(char_iter)?;
    char_iter.next_if_char_eq(':')?;

    while let Some((_ci, c)) = char_iter.next_and_index() {
        if c.is_ascii_control() || " \n<".contains(c) {
            return None;
        }
        if c == '>' {
            return Some(char_iter.offset());
        }
    }
    None
}

//An email address, for these purposes, is anything that matches the non-normative regex from the HTML5 spec:
// /^[a-zA-Z0-9.!#$%&'*+/=?^_`{|}~-]+@[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?
// (?:\.[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)*$/
pub fn parse_email(char_iter: &mut PeekableCharIndices) -> Option<usize> {
    // let (_, c_0) = char_iter.next()?;
    // if c_0 != '<' {
    //     return None;
    // }

    char_iter.consume_while(|c| c.is_ascii_alphanumeric() || ".!#$%&'*+/=?^_`{|}~-".contains(c));

    let c_0 = char_iter.next()?;
    if c_0 != '@' {
        return None;
    }

    let parse_label = |char_iter: &mut PeekableCharIndices| {
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

    char_iter.next_if(|c| c == '>').map(|_| char_iter.offset())
}

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

    return match char_iter.next()? {
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
    };
}

// These functions are called based on lookahead by callers, so opening brackets are consumed
// closing brackets are not though.
pub fn parse_opening_tag(char_iter: &mut PeekableCharIndices) -> Option<usize> {
    char_iter.consume_while(|c| c.is_ascii_alphanumeric() || c == '-');

    let mut attribute_iter = char_iter.clone();
    while parse_attribute(&mut attribute_iter).is_some() {
        *char_iter = attribute_iter.clone();
    }

    dbg!(&char_iter);
    char_iter.consume_while(|c| " \t\n".contains(c));

    //optional
    char_iter.next_if_char_eq('/');

    // Not sure if this is called lazily or not
    char_iter.next_if_char_eq('>').map(|_| char_iter.offset())
}

pub fn parse_closing_tag(char_iter: &mut PeekableCharIndices) -> Option<usize> {
    char_iter.consume_while(|c| c.is_ascii_alphanumeric() || c == '-');

    char_iter.consume_while(|c| " \t\n".contains(c));

    char_iter.next_if(|c| c == '>').map(|_| char_iter.offset())
}

pub fn parse_attribute(char_iter: &mut PeekableCharIndices) -> Option<usize> {
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
    if dbg!("\"\'".contains(dbg!(first_c_of_val))) {
        char_iter.next();
        while let Some((ci, c)) = char_iter.next_and_index() {
            dbg!(c);
            if c == first_c_of_val {
                dbg!(&char_iter);
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

pub fn parse_html_comment(char_iter: &mut PeekableCharIndices) -> Option<usize> {
    //consume the opening "--"
    let c_0 = char_iter.next()?;
    let c_1 = char_iter.next()?;
    if c_0 != '-' || c_1 != '-' {
        return None;
    }

    let mut dash_count = 0;
    // let (index_0, char_0) = char_iter.next_and_index()?;
    if char_iter.next_if_char_eq('>').is_some() {
        return Some(char_iter.offset());
    }
    if char_iter.next_if_char_eq('-').is_some() {
        dash_count += 1;
        if char_iter.next_if_char_eq('>').is_some() {
            return Some(char_iter.offset());
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
            return Some(char_iter.offset());
        } else {
            dash_count = 0;
        }
    }
    None
}

pub fn parse_processing_instruction(char_iter: &mut PeekableCharIndices) -> Option<usize> {
    let mut seen_qm = false;
    while let Some(c) = char_iter.next() {
        if c == '?' {
            seen_qm = true;
        } else if c == '>' && seen_qm {
            return Some(char_iter.offset());
        } else {
            seen_qm = false;
        }
    }
    None
}

pub fn parse_declaration(char_iter: &mut PeekableCharIndices) -> Option<usize> {
    let char_0 = char_iter.next()?;
    if !char_0.is_ascii_alphabetic() {
        return None;
    }
    while let Some(c) = char_iter.next() {
        if c == '>' {
            return Some(char_iter.offset());
        }
    }
    None
}

pub fn parse_cdata_section(char_iter: &mut PeekableCharIndices) -> Option<usize> {
    // also do the "CDATA[" string recognition.
    let to_match = "[CDATA[";
    // if current_iteration_iter.next_if_char_eq('[').is_none() {
    //     break 'collect_lrds;
    // }
    let mut zip_iter = char_iter.take(to_match.len()).zip(to_match.chars());

    while let Some((c_in, c_to_match)) = zip_iter.next() {
        if dbg!(c_in) != dbg!(c_to_match) {
            return None;
        }
    }

    let mut r_brack_count = 0;
    while let Some(c) = char_iter.next() {
        if c == ']' {
            r_brack_count += 1;
        } else if c == '>' && r_brack_count >= 2 {
            return Some(char_iter.offset());
        } else {
            r_brack_count = 0;
        }
    }
    None
}
