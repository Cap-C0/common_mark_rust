use phf::phf_map;

use crate::peekable_char_indices::*;

pub trait UnicodeCategory {
    fn is_unicode_punctuation(&self) -> bool;
    fn is_unicode_symbol(&self) -> bool;
}
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
        // TODO: fix this (not doing omlats right?)
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
                std::char::from_digit(0xc | (xs >> 2), 16)
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
                std::char::from_digit(0x8 | (ys & 0x3), 16)
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
                std::char::from_digit(0xc | (xs >> 2), 16)
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
                std::char::from_digit(0x8 | (ys & 0x3), 16)
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
                std::char::from_digit(0xc | (xs >> 2), 16)
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
                std::char::from_digit(0x8 | (ys & 0x3), 16)
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

type CharPusher = dyn FnMut(char, &mut String);
pub fn push_chars_with_entities_and_bs<Offset>(
    peekable_char_indices: &impl PeekableCharIndices<Offset>,
    string_builder: &mut String,
    in_url: bool,
) {
    let mut char_push_fn: Box<CharPusher> = if in_url {
        Box::new(push_character_in_uri)
    } else {
        Box::new(push_html_reserved_char)
    };
    let mut peekable_char_indices = peekable_char_indices.clone();
    while let Some(c) = peekable_char_indices.next() {
        match c {
            '%' => {
                let mut percent_iter = peekable_char_indices.clone();
                if in_url
                    && let Some(hex_char_0) = percent_iter.next_if(|c| c.is_ascii_hexdigit())
                    && let Some(hex_char_1) = percent_iter.next_if(|c| c.is_ascii_hexdigit())
                {
                    string_builder.push('%');
                    string_builder.push(hex_char_0);
                    string_builder.push(hex_char_1);
                    peekable_char_indices = percent_iter;
                } else {
                    char_push_fn(c, string_builder);
                }
            }
            '\\' => {
                if peekable_char_indices
                    .peek()
                    .is_none_or(|c| !c.is_ascii_punctuation())
                {
                    char_push_fn('\\', string_builder);
                }
            }
            '&' => {
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
                                if let Some(final_trie) = current_trie.get_child(c_nxt)
                                    && let Some(chars) = final_trie.get_value()
                                {
                                    char_result = ResultChar::Array(chars);
                                }
                                break 'trie_crawl;
                            }
                            current_trie_op = current_trie.get_child(c_nxt);
                        }
                    }
                }
                match char_result {
                    ResultChar::Array(chars_out) => {
                        for c in chars_out {
                            char_push_fn(*c, string_builder);
                        }
                    }
                    ResultChar::Single(c) => char_push_fn(c, string_builder),
                    ResultChar::Fail => {
                        char_push_fn('&', string_builder);
                        peekable_char_indices = amp_iter;
                    }
                }
            }
            _ => {
                char_push_fn(c, string_builder);
            }
        }
    }
}

pub static REPLACEMENT_CHARACTER: char = char::from_u32(0xFFFD).unwrap();
