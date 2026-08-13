//# serde_json = "*"
//# serde = {version = "*", features = ["derive"] }
//# phf = {version = "*", features = ["macros"] }

use phf::phf_map;
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

#[derive(Debug)]
struct StaticTrieNode {
    children: phf::Map<char, StaticTrieNode>,
    value: Option<&'static [char]>,
}

static MY_TRIE: StaticTrieNode = StaticTrieNode {
    children: phf_map! {
    'c' => StaticTrieNode{
            children: phf_map!{},
            value: Some(&['x']),
        },
    },
    value: None,
};

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
        self.children.get(&c)
    }

    pub fn to_phf_code(&self, string_builder: &mut String) {
        string_builder.push_str(
            "StaticTrieNode{
            children: phf_map!{",
        );
        for (character, child) in self.children.iter() {
            string_builder.push_str(&format!("'{}' => ", character));
            child.to_phf_code(string_builder);
            string_builder.push_str(",");
        }
        string_builder.push_str("},\n");
        string_builder.push_str("value: ");
        self.value
            .clone()
            .map_or("None".to_string(), |chars| format!("Some(&{:?})", chars))
            .chars()
            .for_each(|c| string_builder.push(c));
        string_builder.push_str(
            ",
            }",
        );
    }
}

static MY_SMALL_TRIE: StaticTrieNode = StaticTrieNode {
    children: phf_map! {'b' => StaticTrieNode{
                children: phf_map!{'a' => StaticTrieNode{
                children: phf_map!{'r' => StaticTrieNode{
                children: phf_map!{},
    value: Some(&['3']),
                },},
    value: None,
                },},
    value: None,
                },'c' => StaticTrieNode{
                children: phf_map!{'a' => StaticTrieNode{
                children: phf_map!{'r' => StaticTrieNode{
                children: phf_map!{},
    value: Some(&['2']),
                },'t' => StaticTrieNode{
                children: phf_map!{},
    value: Some(&['1']),
                },},
    value: None,
                },},
    value: None,
                },},
    value: None,
};

fn main() -> () {
    // let mut simple_tree = TrieNode {
    //     children: HashMap::new(),
    //     value: None,
    // };
    // simple_tree.add_str("cat".chars(), vec!['1']);
    // simple_tree.add_str("car".chars(), vec!['2']);
    // simple_tree.add_str("bar".chars(), vec!['3']);
    // let mut string_builder = String::new();
    // simple_tree.to_phf_code(&mut string_builder);
    // println!("{}", string_builder);
    dbg!(&MY_SMALL_TRIE);
    dbg!(MY_SMALL_TRIE.children.get(&'x'));
    dbg!(MY_SMALL_TRIE.children.get(&'c'));
}
