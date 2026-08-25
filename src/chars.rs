use phf::phf_map;

use crate::peekable_char_indices::{self, PeekableCharIndices};

include!(concat!(env!("OUT_DIR"), "/html_entities.rs"));

// the way we construct this, all tries end in ';'
#[derive(Debug)]
pub struct StaticTrieNode {
    children: phf::Map<char, StaticTrieNode>,
    value: Option<&'static [char]>,
}

impl StaticTrieNode {
    pub fn get_child(&self, c: char) -> Option<&Self> {
        self.children.get(&c)
    }
    pub fn get_value(&self) -> Option<&[char]> {
        self.value
    }
}

pub fn push_character_in_uri(c: char, string_builder: &mut String) {
    if "!#$&'()*+,/:;=?@-._~".contains(c) || c.is_ascii_alphanumeric() {
        push_html_reserved_char(c, string_builder);
    } else {
        // it needs to be utf 8 encoded.
        let as_u32 = c as u32;
        if as_u32 <= 0x007F {
            let ys = as_u32 >> 4;
            let zs = as_u32 & 0xF;
            string_builder.push('%');
            string_builder.push(std::char::from_digit(ys, 16).unwrap().to_ascii_uppercase());
            string_builder.push(std::char::from_digit(zs, 16).unwrap().to_ascii_uppercase());
        } else if as_u32 <= 0x07FF {
            let xs = as_u32 >> 8;
            let ys = (as_u32 >> 4) & 0xF;
            let zs = as_u32 & 0xF;
            string_builder.push('%');
            string_builder.push(
                std::char::from_digit(0xb | (xs >> 2), 16)
                    .unwrap()
                    .to_ascii_uppercase(),
            );
            string_builder.push(
                std::char::from_digit(((xs & 0x3) << 2) | (ys >> 2), 16)
                    .unwrap()
                    .to_ascii_uppercase(),
            );
            string_builder.push('%');
            string_builder.push(
                std::char::from_digit(0x8 | (ys >> 2), 16)
                    .unwrap()
                    .to_ascii_uppercase(),
            );
            string_builder.push(std::char::from_digit(zs, 16).unwrap().to_ascii_uppercase());
        } else if as_u32 <= 0xFFFF {
            let ws = as_u32 >> 12;
            let xs = (as_u32 >> 8) & 0xF;
            let ys = (as_u32 >> 4) & 0xF;
            let zs = as_u32 & 0xF;
            string_builder.push('%');
            string_builder.push(std::char::from_digit(0xE, 16).unwrap().to_ascii_uppercase());
            string_builder.push(std::char::from_digit(ws, 16).unwrap().to_ascii_uppercase());
            string_builder.push('%');
            string_builder.push(
                std::char::from_digit(0xb | (xs >> 2), 16)
                    .unwrap()
                    .to_ascii_uppercase(),
            );
            string_builder.push(
                std::char::from_digit(((xs & 0x3) << 2) | (ys >> 2), 16)
                    .unwrap()
                    .to_ascii_uppercase(),
            );
            string_builder.push('%');
            string_builder.push(
                std::char::from_digit(0x8 | (ys >> 2), 16)
                    .unwrap()
                    .to_ascii_uppercase(),
            );
            string_builder.push(std::char::from_digit(zs, 16).unwrap().to_ascii_uppercase());
        } else if as_u32 <= 0x10FFFF {
            let us = as_u32 >> 20;
            let vs = (as_u32 >> 16) & 0xF;
            let ws = (as_u32 >> 12) & 0xF;
            let xs = (as_u32 >> 8) & 0xF;
            let ys = (as_u32 >> 4) & 0xF;
            let zs = as_u32 & 0xF;
            string_builder.push('%');
            string_builder.push(std::char::from_digit(0xF, 16).unwrap().to_ascii_uppercase());
            string_builder.push(
                std::char::from_digit(((us & 0x3) << 2) | (vs >> 2), 16)
                    .unwrap()
                    .to_ascii_uppercase(),
            );
            string_builder.push('%');
            string_builder.push(
                std::char::from_digit(0x8 | (vs & 0x3), 16)
                    .unwrap()
                    .to_ascii_uppercase(),
            );
            string_builder.push(std::char::from_digit(ws, 16).unwrap().to_ascii_uppercase());
            string_builder.push('%');
            string_builder.push(
                std::char::from_digit(0xb | (xs >> 2), 16)
                    .unwrap()
                    .to_ascii_uppercase(),
            );
            string_builder.push(
                std::char::from_digit(((xs & 0x3) << 2) | (ys >> 2), 16)
                    .unwrap()
                    .to_ascii_uppercase(),
            );
            string_builder.push('%');
            string_builder.push(
                std::char::from_digit(0x8 | (ys >> 2), 16)
                    .unwrap()
                    .to_ascii_uppercase(),
            );
            string_builder.push(std::char::from_digit(zs, 16).unwrap().to_ascii_uppercase());
        }
    }
}

pub fn push_html_reserved_char(c: char, string_builder: &mut String) {
    let x = match c {
        '<' => "&lt;",
        '>' => "&gt;",
        '&' => "&amp;",
        '"' => "&quot;",
        _ => &c.to_string(),
    };
    string_builder.push_str(x);
}

pub fn push_chars_with_entities_and_bs(str_in: &str, string_builder: &mut String) {
    let mut peekable_char_indices = PeekableCharIndices::new(str_in.char_indices());
    while let Some(c) = peekable_char_indices.next() {
        if c == '\\' {
            if let Some(punc) = peekable_char_indices.next_if(|c| c.is_ascii_punctuation()) {
                push_html_reserved_char(punc, string_builder);
            } else {
                push_html_reserved_char('\\', string_builder);
            }
        } else if c == '&' {
            enum ResultChar<'a> {
                Single(char),
                Array(&'a [char]),
                Fail,
            }
            let amp_iter = peekable_char_indices.clone();
            let mut char_result = ResultChar::Fail;
            if peekable_char_indices.next_if_char_eq('#').is_some() {
                if peekable_char_indices
                    .next_if(|c_nxt| "Xx".contains(c_nxt))
                    .is_some()
                {
                    //hexadecimal
                    let mut hex_char_count = 0;
                    let mut hex_acc = 0;
                    'hex_loop: while hex_char_count < 7
                        && let Some(c_nxt) = peekable_char_indices.next()
                    {
                        if c_nxt == ';' {
                            if hex_char_count == 0 {
                                break;
                            }
                            if hex_acc != 0
                                && let Some(char_out) = char::from_u32(hex_acc)
                            {
                                char_result = ResultChar::Single(char_out);
                            } else {
                                char_result = ResultChar::Single(REPLACEMENT_CHARACTER);
                            }
                            break 'hex_loop;
                        } else if c_nxt.is_ascii_hexdigit() {
                            hex_char_count += 1;
                            hex_acc *= 16;
                            hex_acc += c_nxt.to_digit(16).unwrap();
                        } else {
                            break 'hex_loop;
                        }
                    }
                } else {
                    //decimal
                    let mut dec_char_count = 0;
                    let mut dec_acc = 0;
                    'dec_loop: while dec_char_count < 8
                        && let Some(c_nxt) = peekable_char_indices.next()
                    {
                        // dbg!(&dec_char_count);
                        if c_nxt == ';' {
                            if dec_char_count == 0 {
                                break 'dec_loop;
                            }
                            if dec_acc != 0
                                && let Some(char_out) = char::from_u32(dec_acc)
                            {
                                char_result = ResultChar::Single(char_out);
                            } else {
                                char_result = ResultChar::Single(REPLACEMENT_CHARACTER);
                            }
                            break 'dec_loop;
                        } else if c_nxt.is_ascii_digit() {
                            dec_char_count += 1;
                            dec_acc *= 10;
                            dec_acc += c_nxt.to_digit(10).unwrap();
                        } else {
                            break 'dec_loop;
                        }
                    }
                }
                //do the decimal thing
                // 1-7 digits or
            } else {
                let mut current_trie_op = HTML_ENTITIES.get_child('&');
                'trie_crawl: {
                    while let Some(current_trie) = current_trie_op
                        && let Some(c_nxt) = peekable_char_indices.next()
                    {
                        if c_nxt == ';' {
                            match current_trie.get_child(c_nxt) {
                                Some(trie) => {
                                    if let Some(chars) = trie.get_value() {
                                        char_result = ResultChar::Array(chars);
                                    }
                                }
                                _ => (),
                            }
                            break 'trie_crawl;
                        }
                        current_trie_op = current_trie.get_child(c_nxt);
                    }
                }
                //char index is already at next char
            }
            match char_result {
                ResultChar::Array(chars_out) => {
                    for c in chars_out {
                        push_html_reserved_char(*c, string_builder);
                    }
                }
                ResultChar::Single(c) => push_html_reserved_char(c, string_builder),
                ResultChar::Fail => {
                    push_html_reserved_char('&', string_builder);
                    peekable_char_indices = amp_iter;
                }
            }
        } else {
            push_html_reserved_char(c, string_builder);
        }
    }
}

pub static REPLACEMENT_CHARACTER: char = char::from_u32(0xFFFD).unwrap();
