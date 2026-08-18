use phf::phf_map;

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

pub fn push_chars_with_entities_and_bs(chars: &[char], string_builder: &mut String) {
    let mut char_index = 0;
    while char_index < chars.len() {
        if chars[char_index] == '\\' {
            if char_index + 1 < chars.len() && chars[char_index + 1].is_ascii_punctuation() {
                push_html_reserved_char(chars[char_index + 1], string_builder);
            } else {
                push_html_reserved_char('\\', string_builder);
                if char_index + 1 < chars.len() {
                    push_html_reserved_char(chars[char_index + 1], string_builder);
                }
            }
            char_index += 2;
        } else if chars[char_index] == '&' {
            enum ResultChar<'a> {
                Single(char),
                Array(&'a [char]),
                Fail,
            }
            let amp_index = char_index;
            let mut char_result = ResultChar::Fail;
            if char_index + 1 < chars.len() && chars[char_index + 1] == '#' {
                char_index += 1;
                if char_index + 1 < chars.len() && "Xx".contains(chars[char_index + 1]) {
                    //hexadecimal
                    char_index += 2;
                    let mut hex_char_count = 0;
                    'hex_loop: while hex_char_count < 7 && char_index < chars.len() {
                        if chars[char_index] == ';' {
                            if hex_char_count == 0 {
                                break;
                            }
                            let mut out: u32 = 0;
                            for i in amp_index + 3..char_index {
                                out *= 16;
                                out += chars[i].to_digit(16).unwrap();
                            }
                            if out != 0
                                && let Some(char_out) = char::from_u32(out)
                            {
                                char_result = ResultChar::Single(char_out);
                            } else {
                                char_result = ResultChar::Single(REPLACEMENT_CHARACTER);
                            }
                            char_index += 1; // for the next iteration.
                            break 'hex_loop;
                        } else if chars[char_index].is_ascii_hexdigit() {
                            hex_char_count += 1;
                            char_index += 1;
                        } else {
                            break 'hex_loop;
                        }
                    }
                } else {
                    //decimal
                    char_index += 1;
                    let mut dec_char_count = 0;
                    'dec_loop: while dec_char_count < 8 && char_index < chars.len() {
                        dbg!(&dec_char_count);
                        if chars[char_index] == ';' {
                            if dec_char_count == 0 {
                                break 'dec_loop;
                            }
                            let mut out: u32 = 0;
                            for i in amp_index + 2..char_index {
                                out *= 10;
                                out += chars[i].to_digit(10).unwrap();
                            }
                            if out != 0
                                && let Some(char_out) = char::from_u32(out)
                            {
                                char_result = ResultChar::Single(char_out);
                            } else {
                                char_result = ResultChar::Single(REPLACEMENT_CHARACTER);
                            }
                            char_index += 1; // for the next iteration.
                            break 'dec_loop;
                        } else if chars[char_index].is_ascii_digit() {
                            dec_char_count += 1;
                            char_index += 1;
                        } else {
                            break 'dec_loop;
                        }
                    }
                }
                //do the decimal thing
                // 1-7 digits or
            } else {
                let prev_trie: &StaticTrieNode = &HTML_ENTITIES;
                let mut current_trie_op = prev_trie.get_child('&');
                char_index += 1;
                'trie_crawl: {
                    while let Some(current_trie) = current_trie_op
                        && char_index <= chars.len()
                    {
                        if char_index == chars.len()
                            || (current_trie.get_child((chars[(char_index)])).is_none())
                        {
                            if let Some(ent_chars) = current_trie.get_value() {
                                char_result = ResultChar::Array(ent_chars);
                            }
                            break 'trie_crawl;
                        }
                        current_trie_op = current_trie.get_child(chars[char_index]);
                        char_index += 1;
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
                    for i in amp_index..char_index {
                        push_html_reserved_char(chars[i], string_builder);
                    }
                }
            }
        } else {
            push_html_reserved_char(chars[char_index], string_builder);
            char_index += 1;
        }
    }
}

pub static REPLACEMENT_CHARACTER: char = char::from_u32(0xFFFD).unwrap();
