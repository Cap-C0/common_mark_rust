use crate::Block::*;
use crate::Inline;
use crate::ListType::*;
use crate::ast_types::InlineContent::Text;
use crate::ast_types::*;
use std::hint::black_box;
use std::mem;
use std::sync::TryLockError::Poisoned;
pub mod ast_types;

pub fn markdown_to_html(markdown: &str) -> String {
    // first split into lines

    let lines: Vec<Vec<char>> = markdown.split('\n').map(|x| x.chars().collect()).collect();

    // 21-2F and 31-40 and 5B-60 and 7B-7E
    let ascii_punctuation: Vec<char> = (0x21..0x25)
        .chain(0x31..0x40)
        .chain(0x5B..0x60)
        .chain(0x7B..0x7E)
        .map(|x| char::from_u32(x).unwrap())
        .collect();

    let ascii_control: Vec<char> = (0x00..0x1F)
        .chain(0x7F..0x7F)
        .map(|x| char::from_u32(x).unwrap())
        .collect();

    // 1st create block structure of document
    let mut document = Document(vec![]);

    for (line_number, line) in lines.iter().enumerate() {
        // first check continuation conditions
        let mut offset: usize = 0;
        let mut open_block_depth: usize = 0;
        check_continuation_conditions(&document, &line, &mut offset, &mut open_block_depth);

        // now we look for the start of any new structure
        create_new_block_starts(
            &mut document,
            &line,
            &mut offset,
            &mut open_block_depth,
            line_number,
        );
    }

    // see if we can add a new item to the list
    todo!();
}

#[allow(unreachable_code)]
fn check_continuation_conditions(
    document: &Block,
    line: &Vec<char>,
    offset: &mut usize,
    open_block_depth: &mut usize,
) {
    let mut current_block = document;
    // println!("called ccc!");
    'outer: loop {
        match current_block {
            Document(blocks) => match blocks.last() {
                None => break,
                Some(b) => {
                    // println!("has children!");
                    current_block = b;
                }
            },
            BlockQuote(blocks, is_open) => {
                if !*is_open {
                    break 'outer;
                }
                let i = space_indent_count(line, offset);
                match block_quote_encountered(line, *offset, i) {
                    Some(offset_dif) => {
                        *offset += offset_dif;
                        *open_block_depth += 1;
                        match blocks.last() {
                            None => break 'outer,
                            Some(b) => current_block = b,
                        }
                    }
                    None => break 'outer,
                }
            }
            List(list_items, _, _) => {
                *open_block_depth += 1;
                // lists are guaranteed to have at least one item
                current_block = list_items.last().unwrap();
            }
            ListItem(blocks, indent_amount) => {
                for i in 0..*indent_amount {
                    if line.len() <= *offset + i || line[*offset + i] != ' ' {
                        break 'outer;
                    }
                }
                *offset += *indent_amount;
                *open_block_depth += 1;
                match blocks.last() {
                    None => break 'outer,
                    Some(b) => {
                        current_block = b;
                    }
                }
            }
            ATXHeading(_, _) => break, // no additional context on the newline after a heading should change the heading
            SetextHeading(_, _) => break,
            Paragraph(_, is_open) => {
                println!("{}", is_blank_line(line, *offset));
                if !is_blank_line(line, *offset) && *is_open {
                    println!("paragrah cc matched!");
                    *open_block_depth += 1;
                }
                break;
            }
            ThematicBreak => break,
            IndentedCodeBlock(_) => {
                let i = space_indent_count(line, offset);
                if line.len() <= *offset + i || i < 4 {
                    break;
                }
                *offset += 4;
                *open_block_depth += 1;
                break;
            }
            FencedCodeBlock(_, is_open, _, _, _, _) => {
                if *is_open {
                    *open_block_depth += 1;
                }
                break;
            }
            HTMLBlock(items) => todo!(),
        }
    }
}

fn is_blank_line(line: &Vec<char>, offset: usize) -> bool {
    line.iter()
        .skip(offset)
        .all(|c| *c == ' ' || *c == char::from_u32(0x09).unwrap())
}

fn test_cc(ast: &mut Block, line: &str, expected_block: &mut Block, exp_offset: usize) {
    let line: Vec<char> = line.chars().collect();
    // println!("{:?}", line);
    let mut obd = 0;
    let mut offset = 0;
    check_continuation_conditions(&ast, &line, &mut offset, &mut obd);
    println!("{}", line[offset]);
    assert_eq!((ast.get_block(obd), offset), (expected_block, exp_offset));
}

#[test]
fn test_cc_1() {
    let mut test_tree = Document(vec![]);
    test_cc(&mut test_tree, "> abc", &mut Document(vec![]), 0);
}

#[test]
fn test_cc_qb_1() {
    let mut test_tree = Document(vec![BlockQuote(vec![ThematicBreak], true)]);
    let mut exp_tree = BlockQuote(vec![ThematicBreak], true);
    test_cc(&mut test_tree, ">hello", &mut exp_tree, 1);
}

#[test]
fn test_cc_qb_2() {
    let mut test_tree = Document(vec![BlockQuote(vec![ThematicBreak], true)]);
    let mut exp_tree = BlockQuote(vec![ThematicBreak], true);
    test_cc(&mut test_tree, "> hello", &mut exp_tree, 2);
}

#[test]
fn test_cc_qb_3() {
    let mut test_tree = Document(vec![BlockQuote(vec![ThematicBreak], true)]);
    let mut exp_tree = BlockQuote(vec![ThematicBreak], true);
    test_cc(&mut test_tree, "   > hello", &mut exp_tree, 5);
}

#[test]
fn test_cc_qb_4() {
    let mut test_tree = Document(vec![BlockQuote(vec![ThematicBreak], true)]);
    let mut exp_tree = BlockQuote(vec![ThematicBreak], true);
    test_cc(&mut test_tree, " > hello", &mut exp_tree, 3);
}

#[test]
fn test_cc_qb_5() {
    let mut test_tree = Document(vec![BlockQuote(vec![ThematicBreak], true)]);
    let mut exp_tree = BlockQuote(vec![ThematicBreak], true);
    test_cc(&mut test_tree, ">  hello", &mut exp_tree, 2);
}

#[test]
fn test_cc_qb_6() {
    let mut test_tree = Document(vec![BlockQuote(vec![ThematicBreak], false)]);
    let mut exp_tree = test_tree.clone();
    test_cc(&mut test_tree, ">  hello", &mut exp_tree, 0);
}

#[test]
fn test_cc_list_1() {
    let mut test_tree = Document(vec![List(
        vec![ListItem(vec![], 2)],
        true,
        UnorderedList('*'),
    )]);
    let mut exp_tree = ListItem(vec![], 2);
    test_cc(&mut test_tree, "  >  hello", &mut exp_tree, 2);
}

#[test]
fn test_cc_list_2() {
    let mut test_tree = Document(vec![List(
        vec![ListItem(vec![], 2)],
        true,
        UnorderedList('*'),
    )]);
    let mut exp_tree = List(vec![ListItem(vec![], 2)], true, UnorderedList('*'));
    test_cc(&mut test_tree, " >  hello", &mut exp_tree, 0);
}

#[test]
fn test_cc_both_3() {
    let mut test_tree = Document(vec![List(
        vec![ListItem(vec![BlockQuote(vec![], true)], 2)],
        true,
        UnorderedList('*'),
    )]);
    let mut exp_tree = BlockQuote(vec![], true);
    test_cc(&mut test_tree, "  > hello", &mut exp_tree, 4);
}

fn space_indent_count(line: &Vec<char>, offset: &usize) -> usize {
    line.iter().skip(*offset).take_while(|c| **c == ' ').count()
}

fn list_item_encountered(
    line: &Vec<char>,
    offset: usize,
    i: usize,
) -> Option<(ListType, Block, usize)> {
    if line.len() <= offset + i || i > 3 {
        return None;
    }
    if line[offset + i].is_numeric() {
        let mut number_builder = String::from(line[offset + i]);
        for j in 1..10 {
            if line.len() <= offset + i + j {
                return None;
            }
            if line[offset + i + j].is_numeric() {
                number_builder.push(line[offset + i + j]);
                continue;
            }
            if ".)".contains(line[offset + i + j]) || line[offset + i + j + 1] == ' ' {
                if line.len() == offset + i + j + 1 {
                    return Some((
                        OrderedList(line[offset + i + j], number_builder.parse().unwrap()),
                        ListItem(vec![], i + j + 2),
                        i + j + 2,
                    ));
                }
            }
            return None;
        }
    }
    if "-+*".contains(line[offset + i]) {
        if line.len() == offset + i + 1 || line[offset + i + 1] == ' ' {
            return Some((
                UnorderedList(line[offset + i]),
                ListItem(vec![], i + 2),
                i + 2,
            ));
        }
        return None;
    }
    None
}

fn block_quote_encountered(line: &Vec<char>, offset: usize, i: usize) -> Option<usize> {
    if line.len() <= offset + i || i > 3 {
        return None;
    }
    if line[offset + i] == '>' {
        //bounds check
        if line.len() <= offset + i + 1 {
            return Some(i + 1);
        }
        if line[offset + i + 1] == ' ' {
            return Some(i + 2);
        } else {
            return Some(i + 1);
        }
    }
    None
}

fn fenced_code_block_encountered(line: &Vec<char>, offset: usize, i: usize) -> Option<Block> {
    if line.len() <= offset + i + 2 || i > 3 {
        return None;
    }
    let t = line[offset + i];
    if "~`".contains(t) {
        let tick_count = line
            .iter()
            .skip(offset + i)
            .take_while(|c| **c == t)
            .count();
        if tick_count >= 3 {
            if t == '`' {
                if line.iter().skip(offset + i + tick_count).any(|c| *c == '`') {
                    return None;
                }
            }
            let info_string: Vec<char> = line
                .iter()
                .skip(offset + i + tick_count)
                .skip_while(|c| **c == ' ')
                .take_while(|c| **c != ' ')
                .map(|c| *c)
                .collect();
            return Some(FencedCodeBlock(vec![], true, t, info_string, i, tick_count));
        }
    }
    return None;
}

fn atx_heading_encountered(line: &Vec<char>, offset: usize, i: usize) -> Option<Block> {
    if line.len() <= offset + i || i > 3 {
        return None;
    }
    let t = line[offset + i];
    if t == '#' {
        let pound_count = line
            .iter()
            .skip(offset + i)
            .take_while(|c| **c == '#')
            .count();
        if line.len() <= offset + i + pound_count {
            return Some(ATXHeading(vec![], pound_count));
        }
        if pound_count <= 6 && line[offset + i + pound_count] == ' ' {
            return Some(ATXHeading(
                vec![parse_inline(line, offset + i + pound_count)],
                pound_count,
            ));
        }
    }
    None
}

fn close_unmatched_and_paragraph(block: &mut Block, line_number: usize) {
    todo!()
}

// gets rid of initial and final spaces
fn parse_inline(line: &Vec<char>, offset: usize) -> InlineContent {
    let mut text: Vec<char> = vec![];
    line.iter()
        .take(offset)
        .take_while(|c| **c == ' ')
        .fold(0, |space_count, c| {
            if *c != ' ' {
                for i in 0..space_count {
                    text.push(' ');
                }
                text.push(*c);
                return 0;
            }
            return space_count + 1;
        });
    Text(text)
}

fn create_new_block_starts(
    document: &mut Block,
    line: &Vec<char>,
    offset: &mut usize,
    obd: &mut usize,
    line_number: usize,
) {
    match document.get_block(*obd) {
        IndentedCodeBlock(_) => return, // go to parse this as plain text, no changes
        _ => (),
    }

    let mut i = space_indent_count(line, offset);
    dbg!(&offset);
    dbg!(i);
    if line.len() <= *offset + i {
        todo!();
    }
    let mut unmatched_closed = false;
    // first check if we have a paragragh above for:
    // - setext heading creation
    // - lazy continuation
    // also check for if it matches a leaf node, which indicates that the last matched
    // Container node is the leaf-1
    let (open_par_above, ends_in_leaf): (bool, bool) = match document.get_block(*obd) {
        Paragraph(_, open) => (*open, true),
        b => (false, b.is_leaf()),
    };
    println!("open_par_above: {}", open_par_above);

    // c is first non space character after offset
    let mut c = dbg!(line[*offset + i]);

    // First check for SetextHeading
    if open_par_above && i <= 3 {
        if "-=".contains(c) {
            // the line is of the form "[pre-matched-structure][1-3 space]c*' '*"
            dbg!(line);
            if dbg!(
                line.iter()
                    .skip(*offset + i)
                    .skip_while(|k| **k == c)
                    .skip_while(|k| **k == ' ')
                    .count()
            ) == 0
            {
                let mut h_text = vec![];
                mem::swap(
                    &mut h_text,
                    match document.get_block(*obd) {
                        Paragraph(inline, _) => inline,
                        _ => panic!(),
                    },
                );
                println!("htext: {:?}", h_text);
                match document.get_block(*obd - 1) {
                    // by definition some container block
                    Document(blocks) |
                    BlockQuote(blocks, _) |
                    // List(blocks, _, list_type) => the only children of a list are list_blocks
                    ListItem(blocks, _) => {
                        *blocks.last_mut().unwrap() = SetextHeading(h_text, if c == '=' {1} else {2});
                    }
                    _ => panic!("should be unreachable!"),
                }
                *offset = line.len() - 1;
                return;
            }
        }
    }

    // next check for thematic break
    if "-*_".contains(c) && i <= 3 {
        let mut non_space = line.iter().skip(*offset).filter(|k| **k != ' ');
        let count = non_space.clone().count();
        dbg!(*obd);
        if count >= 3 && non_space.all(|k| *k == c) {
            match dbg!(document.get_general_container(*obd)).0 {
                Document(blocks) | BlockQuote(blocks, _) | ListItem(blocks, _) => {
                    blocks.push(ThematicBreak);
                    *offset = line.len() - 1;
                    return;
                }
                _ => panic!("should only match on general container"),
            }
        }
    }

    //now check if we can add a new list item to an existing list
    let last_block_list: Option<ListType> = match document.get_block(*obd) {
        List(_, _, lt) => Some(lt.clone()),
        _ => None,
    };

    if last_block_list.is_some() {
        match list_item_encountered(line, *offset, i) {
            Some((lt, li, offset_dif)) => {
                if last_block_list.unwrap().same_list_eq(&lt) {
                    close_unmatched_and_paragraph(document, 0);
                    match document.get_block(*obd) {
                        List(blocks, _, _) => blocks.push(li),
                        _ => unreachable!(),
                    }
                    *obd += 1;
                    i = space_indent_count(line, offset);
                    *offset += offset_dif;
                    unmatched_closed = true;
                }
            }
            None => (),
        }
    }

    // keep looking for new block starts
    loop {
        if let Some((lt, list_item_block, offset_dif)) = list_item_encountered(line, *offset, i) {
            if open_par_above {
                match lt {
                    OrderedList(_, 1) => (),
                    OrderedList(_, _) => break,
                    _ => (),
                }
            } // only ordered lists starting with 1 can interrupt paragraphs
            close_unmatched_and_paragraph(document, line_number);
            let (parent, new_obd) = document.get_general_container(*obd);
            match parent {
                Document(blocks) | BlockQuote(blocks, _) | ListItem(blocks, _) => {
                    blocks.push(List(vec![list_item_block], true, lt));
                    *obd = new_obd + 2;
                    *offset += offset_dif;
                    i = space_indent_count(line, offset);
                    continue;
                }
                _ => unreachable!(),
            }
        }
        if let Some(offset_dif) = block_quote_encountered(line, *offset, i) {
            close_unmatched_and_paragraph(document, line_number);
            let (parent, new_obd) = document.get_general_container(*obd);
            match parent {
                Document(blocks) | BlockQuote(blocks, _) | ListItem(blocks, _) => {
                    blocks.push(BlockQuote(vec![], true));
                    *obd = new_obd + 1;
                    *offset += offset_dif;
                    i = space_indent_count(line, offset);
                    continue;
                }
                _ => unreachable!(),
            }
        }
        if let Some(fcb) = fenced_code_block_encountered(line, *offset, i) {
            close_unmatched_and_paragraph(document, line_number);
            let (parent, new_obd) = document.get_general_container(*obd);
            match parent {
                Document(blocks) | BlockQuote(blocks, _) | ListItem(blocks, _) => {
                    blocks.push(fcb);
                    *obd = new_obd + 1;
                    return;
                }
                _ => unreachable!(),
            }
        }
        if let Some(atxh) = atx_heading_encountered(line, *offset, i) {
            close_unmatched_and_paragraph(document, line_number);
            let (parent, new_obd) = document.get_general_container(*obd);
            match parent {
                Document(blocks) | BlockQuote(blocks, _) | ListItem(blocks, _) => {
                    blocks.push(atxh);
                    *obd = new_obd + 1;
                    return;
                }
                _ => unreachable!(),
            }
        }
        break;
    }

    //now check if theres a paragrah we can continue lazily
    let mut continuable_paragraph = match document.get_last_block() {
        Paragraph(_, true) => true,
        _ => false,
    };
    if !open_par_above && i > 3 {
        // 4 lines of whitespace and no open paragrah above to "continue" with it
    }

    // first check if can create ThematicBreak (this is because "- \n - - -" is a thematic break,
    // not a new list item)
}

fn test_cnbs(ast: &mut Block, line: &str, expected_ast: &mut Block, exp_offset: usize) {
    let line: Vec<char> = line.chars().collect();
    // println!("{:?}", line);
    let mut obd = 0;
    let mut offset = 0;
    check_continuation_conditions(&ast, &line, &mut offset, &mut obd);
    println!("{:?}", obd);
    create_new_block_starts(ast, &line, &mut offset, &mut obd, 0);
    assert_eq!((ast, offset), (expected_ast, exp_offset));
}

#[test]
fn test_cnbs_1() {
    test_cnbs(
        &mut Document(vec![BlockQuote(
            vec![Paragraph(vec![Text(vec!['a', 'b'])], true)],
            true,
        )]),
        ">-",
        &mut Document(vec![BlockQuote(
            vec![SetextHeading(vec![Text(vec!['a', 'b'])], 2)],
            true,
        )]),
        1,
    );
}

#[test]
fn test_cnbs_2() {
    test_cnbs(
        &mut Document(vec![Paragraph(vec![Text(vec!['a', 'b'])], true)]),
        "-",
        &mut Document(vec![SetextHeading(vec![Text(vec!['a', 'b'])], 2)]),
        0,
    );
}

#[test]
fn test_cnbs_3() {
    test_cnbs(
        &mut Document(vec![Paragraph(vec![Text(vec!['a', 'b'])], true)]),
        "=          ",
        &mut Document(vec![SetextHeading(vec![Text(vec!['a', 'b'])], 1)]),
        10,
    );
}

#[test]
fn test_cnbs_4() {
    test_cnbs(
        &mut Document(vec![Paragraph(vec![Text(vec!['a', 'b'])], true)]),
        "- -",
        &mut Document(vec![Paragraph(vec![Text(vec!['a', 'b'])], true)]),
        0,
    );
}

#[test]
fn test_cnbs_5() {
    test_cnbs(
        &mut Document(vec![BlockQuote(
            vec![Paragraph(vec![Text(vec!['a', 'b'])], true)],
            false,
        )]),
        ">-",
        &mut Document(vec![BlockQuote(
            vec![Paragraph(vec![Text(vec!['a', 'b'])], true)],
            false,
        )]),
        0,
    );
}

#[test]
fn test_cnbs_6() {
    test_cnbs(
        &mut Document(vec![BlockQuote(
            vec![Paragraph(vec![Text(vec!['a', 'b'])], true)],
            true,
        )]),
        ">---",
        &mut Document(vec![BlockQuote(
            vec![SetextHeading(vec![Text(vec!['a', 'b'])], 2)],
            true,
        )]),
        3,
    );
}

#[test]
fn test_cnbs_7() {
    test_cnbs(
        &mut Document(vec![BlockQuote(
            vec![Paragraph(vec![Text(vec!['a', 'b'])], true)],
            true,
        )]),
        ">***",
        &mut Document(vec![BlockQuote(
            vec![Paragraph(vec![Text(vec!['a', 'b'])], true), ThematicBreak],
            true,
        )]),
        3,
    );
}

#[test]
fn test_cnbs_8() {
    test_cnbs(
        &mut Document(vec![BlockQuote(
            vec![List(
                vec![ListItem(vec![ThematicBreak], 2)],
                true,
                UnorderedList('*'),
            )],
            true,
        )]),
        ">***",
        &mut Document(vec![BlockQuote(
            vec![
                List(
                    vec![ListItem(vec![ThematicBreak], 2)],
                    true,
                    UnorderedList('*'),
                ),
                ThematicBreak,
            ],
            true,
        )]),
        3,
    );
}

#[test]
fn test_cnbs_9() {
    test_cnbs(
        &mut Document(vec![BlockQuote(
            vec![List(
                vec![ListItem(vec![ThematicBreak], 2)],
                true,
                UnorderedList('*'),
            )],
            true,
        )]),
        ">   ***",
        &mut Document(vec![BlockQuote(
            vec![List(
                vec![ListItem(vec![ThematicBreak, ThematicBreak], 2)],
                true,
                UnorderedList('*'),
            )],
            true,
        )]),
        6,
    );
}
