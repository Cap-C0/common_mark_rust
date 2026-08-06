// we do not need to enforce multiple new line requirements in these parsers as that will be enforced
// by paragraphs ending at new lines

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
