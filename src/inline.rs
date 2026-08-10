use crate::Inline;
use crate::inline::Delimiter::*;
use crate::inline::{InlineContent::*};
use std::collections::{HashMap, VecDeque};

include!(concat!(env!("OUT_DIR"), "/unicode_categories.rs"));

// we do not need to enforce multiple new line requirements in these parsers as that will be enforced
// by paragraphs ending at new lines

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum InlineContent {
    Softbreak,
    Linebreak,
    Text(usize, usize),
    Emph(Vec<InlineContent>),
    Strong(Vec<InlineContent>),
    /// href, title, link_text
    Link((usize, usize), (usize, usize), Vec<InlineContent>),
    /// src, title, link_text
    Image((usize, usize), (usize, usize), Vec<InlineContent>),
    Code((usize,usize))
}
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


enum Delimiter {
    /// Count, potential_opener, potential_closer
    Asts(usize, bool,bool),
    /// Count, potential_opener, potential_closer
    Unds(usize,bool,bool),
    ImgOpen,
    /// active
    LinkOpen(bool),
    BrackClose,
    BackTick(usize),
    AngleOpen,
    AngleClose
}

// we will be approxiamating a double linked list in rust by having each item in the list keep
// track of the offset to the next item, so we can "delete" dls[j] by setting dls[j-1].otn = 2
// and dls[j+1].otp = 2. Also keep track of an initial offset for when we "delete" dls[0] or 
struct FakeDelimiterDLL {
    // char_offset, dl, index_of_prev, index_of_next
    dl_stack: Vec<(usize, Delimiter, Option<usize>,Option<usize>)>,
    // I think offset will be more efficent than Option<index> as you 
    // dont have to "set" the one behind you when you push to the stack.
    // the tradeoff is, you have to check against the stack size to see if 
    // a value is at the end or beginning instead of just seeing if it is None.
    // Since we only push to the dll at the beginning, we do not need to keep track of 
    // "freeing" things for later. (monotonic?)
    initial_index: Option<usize>,
    final_index: Option<usize>,
}

impl FakeDelimiterDLL {
    // this is only called in the first phase,
    // this means indexes are simply increasing by 1.
    fn push_back(&mut self,char_offset:usize, dl: Delimiter) {
        if self.initial_index.is_none() {
            self.initial_index = Some(0);
            self.final_index = Some(0);
            self.dl_stack.push((char_offset, dl, None, None));
        } else {
             self.dl_stack.push((char_offset, dl, self.final_index, None));
             self.dl_stack[self.final_index.unwrap()].3 = Some(self.dl_stack.len() - 1);
             self.final_index = Some(self.dl_stack.len() - 1);
        }
    }

    fn delete_while_char_offset_lt(&mut self, from_index: usize, min_char_off: usize) {
        let current_list_pos = from_index;
        let (..,mut next_index) = self.dl_stack[current_list_pos];
        while next_index.is_some_and(|i| self.dl_stack[i].0 < min_char_off) {
            next_index = self.dl_stack[next_index.unwrap()].2;
        }
        self.dl_stack[from_index].2 = next_index;
    }
}

// think about what functionality we will need as we go along stack

//start with just emphasis and code, then do links
//adjust the code to 2 phases, where we put all delimiters in the stack at once,
//then go forward through the stack to create
//
//BackTick ocde spans, auto links, raw_html >
//brackets in link text >
//emph markers.
//
//this means we create a call stack where our FIRST call is to emphasis,
//and then encountering a "tighter" binding opener makes us call that. st the
//"tighter" binding parser returns first!
pub fn parse_inline(inline: &Inline, lrd_table: &HashMap<Vec<char>, (Vec<char>, Vec<char>)> ) -> Vec<InlineContent> {
    // pointer, _, offset_to_next, offset_to_prev
    let mut delimit_stack = FakeDelimiterDLL{
        dl_stack: vec![],
        initial_index: None,
        final_index: None
    };
    let mut char_iter = inline.iter().enumerate().peekable();


    //construct entire dll
    while let Some((offset, &c)) = char_iter.next() {
        match c {
            '\\' => {
                if char_iter.peek().map_or(false, |&(_,&d)| d != '`'){
                    char_iter.next();
                }
            },
            '`' =>{
                let mut tick_count = 1;
                while char_iter.peek().map_or(false, |&(_, &c)| c == '`') {
                    tick_count += 1;
                    char_iter.next();
                }
                delimit_stack.push_back(offset, BackTick(tick_count));
            },
            '_'|'*' => {
                let mut dc = 1; //delimiter count
                while char_iter.peek().map_or(false, |&(_, &d)| d == c) {
                    dc += 1;
                    char_iter.next();
                }
                let punc_preceded = offset > 0 && inline[offset - 1].is_unicode_punctuation();
                let punc_followed = offset + dc < inline.len() && inline[offset  + dc].is_unicode_punctuation();
                let is_left_flanking: bool = offset + dc <= inline.len() &&    //not_whitespace_followed
                    !(inline[offset + dc].is_whitespace()) &&
                    (
                        !punc_followed ||
                        (punc_followed &&
                            (offset <= 0 || 
                            inline[offset - 1].is_whitespace() || 
                            punc_preceded)
                        )
                    ); // or followed by punc and preceded by ws or punc
                let is_right_flanking: bool = offset > 0 &&// not_whitespace_preceded
                    !(inline[offset - 1].is_whitespace()) &&
                    (
                        !punc_preceded ||
                        (punc_preceded && 
                            (offset + dc >= inline.len() ||
                            inline[offset + dc].is_whitespace() ||
                            punc_followed)
                        )
                    );
                delimit_stack.push_back(offset, 
                    match c {
                        '*' => {Asts(dc, is_left_flanking,is_right_flanking)}
                        '_' => {Unds(dc, is_left_flanking,is_right_flanking)},
                        _ => panic!()
                    }
                );
            },
            '!' => {
                    if char_iter.peek().map_or(false, |&(_,&d)| d == '[') {
                        char_iter.next();
                        delimit_stack.push_back(offset, ImgOpen);
                    }
                }
            _ => {
                let _ = match c {
                    ']' => Some(BrackClose),
                    '<' => Some(AngleOpen),
                    '>' => Some(AngleClose),
                    _ => None
                }.map(|dl| delimit_stack.push_back(offset, dl));
            },
        }

    }

    // fsub: forward search upper bound
    // deepest nested link has priority, boolean returns if a link was found.

    // now at end of line we look through our stacks
    todo!()
}

struct StackProcessContext<'a, 'b> {
    stack: &'a mut FakeDelimiterDLL,
    lrd_table: &'b HashMap<Vec<char>, (Vec<char>, Vec<char>)>,
}

// fsub: forward search upper bound
// boolean tells caller if it contains a link, deepest nested link has priority
fn process_emphasis(start_item_index:usize, fsub: Option<usize>, context: &mut StackProcessContext) -> (Vec<InlineContent>, bool, Option<usize>){
    let mut inside_content: Vec<(InlineContent,usize)> = vec![]
    let mut out = vec![];
    let mut current_index = start_item_index;
    //If one of the delimiters can both open and close emphasis, then the sum of the lengths
    //of the delimiter runs containing the opening and closing delimiters must not be a
    //multiple of 3 unless both lengths are multiples of 3.
    //Even though we conceptually "eat" them to match, that doesnt mean we actually eat them for
    //rule number 9
    // bool = pot_op && pot_clos
    // only set op_bot when we find a non_matched closer_delimiter
    let mut openers_bottom_and_closing: [Option<(usize,usize,bool)>;3] = [None;3];
    let mut openers_bottom_not_closing: [Option<(usize,usize,bool)>;3] = [None;3];
    let mut unds_stac:Vec<(usize,usize,bool)> = vec![];
    let mut asts_stack: Vec<(usize,usize) = 


    //the one that opens later takes precedence
    while fsub.map_or(true, |n| current_index < n) {
        match context.stack.dl_stack[current_index].1 {
            ImgOpen | LinkOpen(_) => {
                todo!()
            }
            BackTick(_) | AngleOpen => {
                todo!()
            }
            Asts(count, pot_op, pot_clos) => {
                let mut ast_stack_offset = asts_stack
                if pot_clos {
                    while !asts_stack.is_empty() && count > 0 {
                        let (&dl_stack_index, opener_ast_count, &op_and_close) = asts_stack.last().unwrap();
                        
                        
                    }
                }
            }
            Unds(count, pot_op, pot_clos) => {

            }
            BrackClose | AngleClose => {
                if let Some(nxt) = context.stack.dl_stack[current_index].3 {
                    current_index = nxt
                } else {
                    break;
                }
            }
        }
    }
    (out, true, None)
}

// option usize tells caller where to continue
fn process_link(start_item_index:usize, fsub: Option<usize>, context: &mut StackProcessContext) -> (Option<usize,Vec<InlineContent,bool>>){
    let mut out = vec![];
    let current_index = start_item_index;
    todo!()
}

fn process_code_als_html(start_item_index:usize, fsub: Option<usize>, context: &mut StackProcessContext) -> (Vec<InlineContent>, bool, Option<usize>){
    todo!()
}

