//# serde_json = "*"
//# serde = {version = "*", features = ["derive"] }

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::str::Chars;

#[derive(Debug, Serialize, Deserialize)]
struct CharacterInfo {
    codepoints: Vec<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TrieNode {
    children: HashMap<char, TrieNode>,
    value: Option<Vec<char>>,
}

impl TrieNode {
    pub fn add_str(&mut self, mut string_in: Chars, value: Vec<char>) {
        let mut current: &mut Self = self;
        while let Some(nxt_char) = string_in.next() {
            if current.children.get(&nxt_char).is_none() {
                let new_trie = TrieNode {
                    children: HashMap::new(),
                    value: None,
                };
                current.children.insert(nxt_char, new_trie);
            }
            current = current.children.get_mut(&nxt_char).unwrap();
        }
        current.value = Some(value);
    }

    pub fn get_child(&self, c: char) -> Option<&Self> {
        self.children.get(c)
    }
}

fn main() -> () {
    let char_maps: HashMap<String, CharacterInfo> =
        serde_json::from_str(&fs::read_to_string("entities.json").unwrap()).unwrap();
    // let key_val: Vec<(String, Vec<u32>)> = char_maps
    //     .into_iter()
    //     .filter(|(s, _v)| s.contains(';'))
    //     .map(|(s, v)| (s, v.codepoints))
    //     .collect();
    let mut base_trie = TrieNode {
        children: HashMap::new(),
        value: None,
    };
    for (string, vec) in char_maps
        .into_iter()
        .filter(|(s, _v)| s.contains(';'))
        .map(|(s, v)| (s, v.codepoints))
    {
        base_trie.add_str(
            string.chars(),
            vec.into_iter()
                .map(|c| char::from_u32(c).unwrap())
                .collect(),
        )
    }
    dbg!(&base_trie);
    // dbg!(&key_val);
    let mut c = "⫋︀".chars();
    dbg!(c.next().map(|x| x as u32));
    dbg!(c.next().map(|x| x as u32));
}
