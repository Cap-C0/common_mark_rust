use quote::{format_ident, quote};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fmt::Debug;
use std::fs;
use std::fs::File;
use std::io;
use std::io::BufRead;
use std::path::Path;
use std::process::Command;
use std::str::Chars;

#[derive(Serialize, Deserialize)]
struct TestCase {
    markdown: String,
    html: String,
    example: u32,
    section: String,
}

#[derive(Serialize, Deserialize)]
struct CharacterInfo {
    codepoints: Vec<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TrieNode {
    children: HashMap<char, TrieNode>,
    value: Option<Vec<char>>,
}

impl TrieNode {
    pub fn add_str(&mut self, string_in: Chars, value: Vec<char>) {
        let mut current: &mut Self = self;
        for nxt_char in string_in {
            if current.children.contains_key(&nxt_char) {
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

    pub fn to_phf_code(&self, string_builder: &mut String) {
        string_builder.push_str("StaticTrieNode{\nchildren: phf_map!{");
        for (character, child) in self.children.iter() {
            string_builder.push_str(&format!("'{}' => ", character));
            child.to_phf_code(string_builder);
            string_builder.push(',');
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

// struct StaticTrieNode {
//     children: phf::Map<char, StaticTrieNode>,
//     value: Option<&'static [char]>,
// }

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();

    /* TESTS FROM COMMON MARK */
    println!("cargo::rerun-if-changed=spec.json");
    let spec_tests_path = Path::new(&out_dir).join("spec_tests.rs");
    let spec_json_path = Path::new("spec.json");
    get_file_if_doesnt_exist(
        spec_json_path,
        "https://spec.commonmark.org/0.31.2/spec.json",
    );

    if needs_regen(&spec_tests_path, spec_json_path) {
        let cases: Vec<TestCase> =
            serde_json::from_str(&fs::read_to_string(spec_json_path).unwrap()).unwrap();

        let tests = cases.into_iter().map(|case| {
            let fn_name = format_ident!(
                "{}_{}",
                &case.section.replace(' ', "_").to_lowercase(),
                &case.example
            );
            let md_input = case.markdown;
            let html_out = case.html;

            quote! {
                #[test]
                #[timeout(50)]
                fn #fn_name() {
                    println!("input:\n{}",#md_input);
                    assert_eq!(markdown_to_html(#md_input), #html_out);
                }
            }
        });

        let expanded = quote! {
            #(#tests)*
        };

        fs::write(spec_tests_path, expanded.to_string()).unwrap();
    }

    /* UNICODE DATA (for punctuations and symbols)*/
    println!("cargo::rerun-if-changed=UnicodeData.txt");
    let unicode_categories_funs_path = Path::new(&out_dir).join("unicode_categories.rs");
    let unicode_data = Path::new("UnicodeData.txt");
    get_file_if_doesnt_exist(
        unicode_data,
        "https://www.unicode.org/Public/UCD/latest/ucd/UnicodeData.txt",
    );

    if needs_regen(unicode_data, &unicode_categories_funs_path) {
        let lines = io::BufReader::new(File::open(unicode_data).unwrap()).lines();
        let mut punctuations: Vec<(char, char)> = vec![];
        let mut symbols: Vec<(char, char)> = vec![];

        let mut punc_range: (u32, u32) = (0, 0);
        let mut sym_range: (u32, u32) = (0, 0);

        for line in lines {
            let line = line.unwrap();
            let mut split_line = line.split(';');
            let num = u32::from_str_radix(split_line.next().unwrap(), 16).unwrap();
            let cat = split_line.nth(1).unwrap();
            match &cat.chars().next() {
                Some('P') => {
                    if punc_range.0 == 0 {
                        punc_range.0 = num;
                    }
                    punc_range.1 = num;
                    if sym_range.0 != 0 {
                        symbols.push((
                            char::from_u32(sym_range.0).unwrap(),
                            char::from_u32(sym_range.1).unwrap(),
                        ));
                        sym_range.0 = 0;
                    }
                }
                Some('S') => {
                    if sym_range.0 == 0 {
                        sym_range.0 = num;
                    }
                    sym_range.1 = num;
                    if punc_range.0 != 0 {
                        punctuations.push((
                            char::from_u32(punc_range.0).unwrap(),
                            char::from_u32(punc_range.1).unwrap(),
                        ));
                        punc_range.0 = 0;
                    }
                }
                _ => {
                    if punc_range.0 != 0 {
                        punctuations.push((
                            char::from_u32(punc_range.0).unwrap(),
                            char::from_u32(punc_range.1).unwrap(),
                        ));
                        punc_range.0 = 0;
                    } else if sym_range.0 != 0 {
                        symbols.push((
                            char::from_u32(sym_range.0).unwrap(),
                            char::from_u32(sym_range.1).unwrap(),
                        ));
                        sym_range.0 = 0;
                    }
                }
            }
        }

        let punc_ranges = punctuations.iter().map(|(c1, c2)| {
            quote! {(#c1 <= *self && *self <= #c2) ||}
        });
        let sym_ranges = symbols.iter().map(|(c1, c2)| {
            quote! {(#c1 <= *self && *self <= #c2) ||}
        });

        let quoted_code = quote! {

            impl UnicodeCategory for char {
                fn is_unicode_punctuation(&self) -> bool {
                    #(#punc_ranges)* false
                }

                fn is_unicode_symbol(&self) -> bool {
                    #(#sym_ranges)* false
                }
            }
        };

        fs::write(unicode_categories_funs_path, quoted_code.to_string()).unwrap();
    }

    /* ENTITY REFERENCES */
    println!("cargo::rerun-if-changed=entities.json");
    let html_entities_funs = Path::new(&out_dir).join("html_entities.rs");
    let html_entities = Path::new("entities.json");
    get_file_if_doesnt_exist(html_entities, "https://html.spec.whatwg.org/entities.json");
    if needs_regen(html_entities, &html_entities_funs) {
        let char_maps: HashMap<String, CharacterInfo> =
            serde_json::from_str(&fs::read_to_string(html_entities).unwrap()).unwrap();
        // because we are doing a char by char reading, Tries are better than str -> char map. (fail sooner)
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
        let mut string_builder = String::new();

        // let pre_structure = quote! {
        //     use phf::phf_map;
        //
        //     #[derive(Debug)]
        //     struct StaticTrieNode {
        //         children: phf::Map<char, StaticTrieNode>,
        //         value: Option<&'static [char]>,
        //     }
        // };
        //
        // string_builder.push_str(&pre_structure.to_string());

        string_builder.push_str("pub static HTML_ENTITIES: StaticTrieNode = ");
        base_trie.to_phf_code(&mut string_builder);
        string_builder.push_str(";\n");
        fs::write(html_entities_funs, string_builder).unwrap();
    }

    /* UNICODE CASEFOLD DATA (for punctuations and symbols)*/
    println!("cargo::rerun-if-changed=UnicodeData.txt");
    let unicode_case_funs = Path::new(&out_dir).join("unicode_casefold.rs");
    let unicode_case_data = Path::new("CaseFolding.txt");
    get_file_if_doesnt_exist(
        unicode_case_data,
        "https://www.unicode.org/Public/UCD/latest/ucd/CaseFolding.txt",
    );

    if needs_regen(unicode_case_data, &unicode_case_funs) {
        let lines = io::BufReader::new(File::open(unicode_case_data).unwrap()).lines();

        let mut mapping: Vec<(char, Vec<char>)> = vec![];

        for line in lines {
            let line = line.unwrap();
            if line.is_empty() || matches!(line.get(0..1), Some("#")) {
                continue;
            }
            let mut split_line = line.split("; ");
            let uppercase_char =
                char::from_u32(u32::from_str_radix(split_line.next().unwrap(), 16).unwrap())
                    .unwrap();
            let status = split_line.next().unwrap();
            if !"CF".contains(status) {
                continue;
            }
            let chars_to = split_line
                .next()
                .unwrap()
                .split(' ')
                .map(|hex_cs| char::from_u32(u32::from_str_radix(hex_cs, 16).unwrap()).unwrap())
                .collect();
            mapping.push((uppercase_char, chars_to));
        }
        let match_cases = mapping.iter().map(|(uppercase, lowercase)| {
            let mut lowercase_iter = lowercase.iter();
            let char_0 = lowercase_iter.next().unwrap();
            let char_1 = lowercase_iter.next().unwrap_or(&' ');
            let char_2 = lowercase_iter.next().unwrap_or(&' ');
            let char_count = lowercase.len();

            quote! {#uppercase =>
                    UnicodeCaseFoldIter{
                        chars: [
                            #char_0,
                            #char_1,
                            #char_2
                        ],
                        char_count: #char_count,
                        current_offset: 0,
                    },
            }
        });

        let quoted_code = quote! {
            // #[derive(Clone)]
            // struct UnicodeCaseFoldIter {
            //     chars: [char; 3],
            //     char_count: usize,
            //     current_offset: usize,
            // }
            //
            // impl Iterator for UnicodeCaseFoldIter {
            //     type Item = char;
            //
            //     fn next(&mut self) -> Option<Self::Item> {
            //         if self.current_offset >= self.char_count {
            //             None
            //         } else {
            //             let out = self.chars[self.current_offset];
            //             self.current_offset += 1;
            //             Some(out)
            //         }
            //     }
            // }
            //
            // trait UnicodeCaseFold {
            //     fn unicode_case_fold(&self) -> UnicodeCaseFoldIter;
            // }

            impl UnicodeCaseFold for char {
                fn unicode_case_fold(&self) -> UnicodeCaseFoldIter {
                    match self {
                        #(#match_cases)*
                        c => UnicodeCaseFoldIter{
                            chars:[*c,' ', ' '],
                            char_count: 1,
                            current_offset: 0,
                        }
                    }
                }
            }
        };

        fs::write(unicode_case_funs, quoted_code.to_string()).unwrap();
    }
}

fn get_file_if_doesnt_exist(path: &Path, url: &str) {
    if !path.exists() {
        let output = Command::new("curl")
            .args(["-O", url])
            .output()
            .expect("Failed to curl spec");
        if !output.status.success() {
            panic!("Failed to fetch test cases json")
        }
    }
}

/// assumes both paths exist
fn needs_regen(input_path: &Path, output_path: &Path) -> bool {
    let input_mtime = fs::metadata(input_path).and_then(|m| m.modified()).ok();
    let output_mtime = fs::metadata(output_path).and_then(|m| m.modified()).ok();

    match (input_mtime, output_mtime) {
        (Some(i), Some(o)) => i > o, //
        _ => true,                   // otherwise maybe output doesnt exist yet so do run it
    }
}
