use crate::Block::*;
use crate::ListType::*;
use crate::ast_types::*;
use std::collections::HashMap;
use std::mem;
pub mod ast_types;

fn is_ascii_punctuation(c: char) -> bool {
    let n = c as u32;
    (0x21 <= n && n <= 0x25)
        || (0x31 <= n && n <= 0x40)
        || (0x5B <= n && n <= 0x60)
        || (0x7B <= n && n <= 0x7E)
}

fn is_ascii_control(c: char) -> bool {
    let n = c as u32;
    // since its u32, 0 <= n by definition
    (n <= 0x1F) || (0x7F <= n && n <= 0x7F)
}

pub fn markdown_to_html(markdown: &str) -> String {
    // first split into lines

    let lines: Vec<Vec<char>> = markdown.split('\n').map(|x| x.chars().collect()).collect();

    // 21-2F and 31-40 and 5B-60 and 7B-7E
    // let ascii_punctuation: Vec<char> = (0x21..0x25)
    //     .chain(0x31..0x40)
    //     .chain(0x5B..0x60)
    //     .chain(0x7B..0x7E)
    //     .map(|x| char::from_u32(x).unwrap())
    //     .collect();
    //
    // let ascii_control: Vec<char> = (0x00..0x1F)
    //     .chain(0x7F..0x7F)
    //     .map(|x| char::from_u32(x).unwrap())
    //     .collect();

    // 1st create block structure of document
    let mut document = Document(vec![]);
    let mut lrd_table: HashMap<Vec<char>, (Vec<char>, Vec<char>)> = HashMap::new();

    for line in lines.iter() {
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
            &mut lrd_table,
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
                let i = space_indent_count(line, *offset);
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
                let mut i = 0;
                'inner: while i < *indent_amount {
                    if line.len() <= *offset + i {
                        break 'inner;
                    }
                    if line[*offset + i] != ' ' {
                        break 'outer;
                    }
                    i += 1;
                }
                if is_blank_line(line, *offset + i) {
                    *offset = line.len(); // a blank line is auto allowed to match
                } else {
                    *offset += *indent_amount;
                }
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
                if *is_open {
                    *open_block_depth += 1;
                }
                break;
            }
            ThematicBreak => break,
            IndentedCodeBlock(_, _) => {
                let i = space_indent_count(line, *offset);
                if line.len() > *offset + i && i < 4 {
                    break;
                }
                *offset += std::cmp::min(4, line.len() - (*offset + i));
                *open_block_depth += 1;
                break;
            }
            FencedCodeBlock(_, is_open, _, _, _, _) => {
                if *is_open {
                    *open_block_depth += 1;
                }
                break;
            }
            HTMLBlock(_items) => todo!(),
        }
    }
}

fn is_blank_line(line: &Vec<char>, offset: usize) -> bool {
    line[..offset]
        .iter()
        .all(|c| *c == ' ' || *c == char::from_u32(0x09).unwrap())
}

#[cfg(test)]
mod cc_tests {
    use super::*;

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
}

fn space_indent_count(line: &Vec<char>, offset: usize) -> usize {
    line[..offset].iter().take_while(|c| **c == ' ').count()
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
                dbg!(&mut number_builder).push(line[offset + i + j]);
                continue;
            }
            if ".)".contains(line[offset + i + j]) {
                let following_space_count = space_indent_count(line, offset + i + j);
                if line.len() == offset + i + j + following_space_count {
                    return Some((
                        OrderedList(line[offset + i + j], number_builder.parse().unwrap()),
                        ListItem(vec![], i + j + 2),
                        i + j + 2,
                    ));
                }
                if 1 <= following_space_count && following_space_count <= 4 {
                    return Some((
                        OrderedList(line[offset + i + j], number_builder.parse().unwrap()),
                        ListItem(vec![], i + j + 1 + following_space_count),
                        i + j + 1 + following_space_count,
                    ));
                }
                if following_space_count > 4 {
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
        let following_space_count = line[offset + i..].iter().take_while(|c| **c == ' ').count();
        if line.len() == offset + i + following_space_count {
            // ie, the rest of the line is blank
            return Some((
                UnorderedList(line[offset + i]),
                ListItem(vec![], i + 2),
                i + 2,
            ));
        }
        if 1 <= following_space_count && following_space_count <= 4 {
            return Some((
                UnorderedList(line[offset + i]),
                ListItem(vec![], i + 1 + following_space_count),
                i + 1 + following_space_count,
            ));
        }
        if following_space_count > 4 {
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
        let tick_count = line[..offset + i].iter().take_while(|c| **c == t).count();
        if tick_count >= 3 {
            if t == '`' {
                if line[..offset + i + tick_count].iter().any(|c| *c == '`') {
                    return None;
                }
            }
            let info_string: Vec<char> = line[..offset + i + tick_count]
                .iter()
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
        let pound_count = line[..offset + i].iter().take_while(|c| **c == '#').count();
        if line.len() <= offset + i + pound_count {
            return Some(ATXHeading(vec![], pound_count));
        }
        if pound_count <= 6 && line[offset + i + pound_count] == ' ' {
            let mut inline_text: Vec<char> = vec![];
            dbg!(pound_count);
            line[..offset + i + pound_count].iter().fold(
                (0, 0, 0),
                |(pre_space, close_pounds, post_space), c| {
                    if *c == '#' {
                        if close_pounds == 0 && post_space > 0 {
                            return (post_space, 1, 0);
                        }
                        if close_pounds > 0 {
                            return (pre_space, close_pounds + 1, 0);
                        }
                        inline_text.push('#');
                        return (0, 0, 0);
                    }
                    if *c == ' ' {
                        return (pre_space, close_pounds, post_space + 1);
                    }
                    if inline_text.len() > 0 {
                        for _i in 0..pre_space {
                            inline_text.push(' ');
                        }
                    }
                    for _i in 0..close_pounds {
                        inline_text.push('#');
                    }
                    for _i in 0..post_space {
                        inline_text.push(' ');
                    }
                    inline_text.push(*c);
                    (0, 0, 0)
                },
            );
            return Some(ATXHeading(inline_text, pound_count));
        }
    }
    None
}

fn close_paragraph(
    document: &mut Block,
    open_par_above: &mut bool,
    open_par_exists: &mut bool,
    lrd_table: &mut HashMap<Vec<char>, (Vec<char>, Vec<char>)>,
) {
    if !*open_par_exists {
        *open_par_above = false;
        return;
    } //no paragraph to close, job done.

    match document.get_last_block() {
        Paragraph(chars, b @ true) => {
            let mut chars_iter = chars.iter().enumerate().peekable();
            let mut characters_eaten: usize = 0;
            'collect_lrds: loop {
                let (mut link_lab, mut link_dest, mut link_tit): (Vec<char>, Vec<char>, Vec<char>) =
                    (vec![], vec![], vec![]);
                let mut link_lab_found = false;
                if let Some((start_count, '[')) = chars_iter.next() {
                    // take white space
                    while let Some(&(i, &_c @ (' ' | '\n'))) = chars_iter.peek() {
                        if i - start_count <= 1000 {
                            let _ = chars_iter.next();
                        }
                    }
                    let _ = chars_iter.by_ref().take_while(|(i, c)| {
                        (**c == '\n' || **c == ' ') && i - start_count <= 1000
                    });
                    if let Some((i, &c)) = chars_iter.next() {
                        if c == ']' {
                            break 'collect_lrds;
                        }
                        if i - start_count >= 1000 {
                            break 'collect_lrds;
                        }
                        link_lab.push(c);
                    }
                    while let Some((i, &c)) = chars_iter.next() {
                        if i - start_count >= 1000 {
                            break 'collect_lrds;
                        }
                        if c == '\\' {
                            if i - (start_count + 1) >= 1000 {
                                break 'collect_lrds;
                            }
                            if let Some((i, &c_nxt)) = chars_iter.next() {
                                if i - start_count >= 1000 {
                                    break 'collect_lrds;
                                }
                                link_lab.push(c);
                                link_lab.push(c_nxt);
                                continue;
                            } else {
                                break 'collect_lrds;
                            }
                        }
                        if c == '[' {
                            break 'collect_lrds;
                        }
                        if c == ']' {
                            if let Some((_, &c_nxt)) = chars_iter.next() {
                                if c_nxt == ':' {
                                    link_lab_found = true;
                                    break;
                                }
                            }
                        }
                        link_lab.push(c);
                    }
                }

                if !link_lab_found {
                    break 'collect_lrds;
                }

                dbg!(&link_lab);

                while let Some(&(_i, &_c @ (' ' | '\n'))) = chars_iter.peek() {
                    let _ = chars_iter.next();
                }

                let mut link_dest_found = false;
                let mut valid_lrd_found = false;

                'match_link_dest: {
                    match chars_iter.by_ref().next() {
                        Some((_, &'<')) => {
                            while let Some((_, &c)) = chars_iter.next() {
                                if c == '\\' {
                                    if let Some((_, &c_nxt)) = chars_iter.next() {
                                        if c_nxt == '\n' {
                                            break;
                                        }
                                        if !is_ascii_punctuation(c_nxt) {
                                            link_dest.push(c);
                                        }
                                        link_dest.push(c_nxt);
                                        continue;
                                    } else {
                                        break 'collect_lrds;
                                    }
                                } else if c == '<' || c == '\n' {
                                    break;
                                } else if c == '>' {
                                    if let Some(&(_, &cnxt)) = chars_iter.peek() {
                                        if !"\n ".contains(cnxt) {
                                            break 'collect_lrds; // this means there are non space
                                            // seperated chars after the dest, so
                                            // immediate fail.
                                        }
                                    } else {
                                        valid_lrd_found = true; // line ends here, so can't be messed
                                        // up
                                    }
                                    link_dest_found = true;
                                    break;
                                } else {
                                    link_dest.push(c);
                                }
                            }
                        }
                        Some((_, &c)) => {
                            let mut p_stack = 0;
                            if is_ascii_control(c) {
                                break 'collect_lrds;
                            }
                            if c == ')' {
                                break 'collect_lrds;
                            }
                            if c == '(' {
                                p_stack += 1;
                            }
                            if c == '\\' {
                                if let Some((i, &c_nxt)) = chars_iter.next() {
                                    if is_ascii_control(c_nxt) {
                                        break 'collect_lrds;
                                    }
                                    if "\n ".contains(c_nxt) {
                                        link_dest.push(c);
                                        link_dest_found = true; // p_stack is irrelevent here
                                        if c_nxt == '\n' {
                                            valid_lrd_found = true;
                                            characters_eaten = i + 1;
                                            break 'match_link_dest;
                                        }
                                    }
                                    link_dest.push(c);
                                    link_dest.push(c_nxt);
                                } else {
                                    break 'collect_lrds;
                                }
                            }
                            link_dest.push(c);
                            while let Some((i, &c)) = chars_iter.next() {
                                if is_ascii_control(c) {
                                    break 'collect_lrds;
                                }
                                if c == ')' {
                                    if p_stack == 0 {
                                        break 'collect_lrds;
                                    }
                                    p_stack -= 1;
                                }
                                if c == '(' {
                                    p_stack += 1;
                                }
                                if c == '\\' {
                                    if let Some((i_nxt, &c_nxt)) = chars_iter.next() {
                                        if is_ascii_control(c_nxt) {
                                            break 'collect_lrds;
                                        }
                                        if "\n ".contains(c_nxt) {
                                            link_dest.push(c);
                                            link_dest_found = p_stack == 0;
                                            if c_nxt == '\n' {
                                                valid_lrd_found = link_dest_found;
                                                characters_eaten = i_nxt + 1;
                                            }
                                            break 'match_link_dest;
                                        }
                                        link_dest.push(c);

                                        link_dest.push(c_nxt);
                                    } else {
                                        break 'collect_lrds;
                                    }
                                } else if "\n ".contains(c) {
                                    link_dest_found = p_stack == 0;
                                    if c == '\n' {
                                        valid_lrd_found = link_dest_found;
                                        characters_eaten = i + 1;
                                    }
                                    break 'match_link_dest;
                                } else {
                                    link_dest.push(c);
                                }
                            }
                            // at this point, the chars must have ended
                            link_dest_found = p_stack == 0;
                            valid_lrd_found = link_dest_found;
                            characters_eaten = chars.len();
                        }
                        _ => break 'collect_lrds,
                    }
                }

                dbg!(&link_dest);
                if !link_dest_found {
                    break 'collect_lrds;
                }

                while let Some(&(_i, &_c @ ' ')) = chars_iter.peek() {
                    let _ = chars_iter.next();
                }
                let mut link_tit_found = false;

                'match_link_title: {
                    match chars_iter.next() {
                        Some((i, &c @ ('\'' | '\"' | '(' | '\n'))) => {
                            let mut dl = c;
                            if c == '\n' {
                                characters_eaten = dbg!(i + 1);
                                valid_lrd_found = true;
                                // even if the new line fails.
                                if let Some((_, &cnxt @ ('\'' | '\"' | '('))) = chars_iter.next() {
                                    dl = cnxt;
                                } else {
                                    break 'match_link_title;
                                }
                            }
                            let dlc = match dl {
                                '\'' => '\'',
                                '\"' => '\"',
                                '(' => ')',
                                _ => unreachable!(),
                            };
                            dbg!(dl);
                            while let Some((_i, &c)) = chars_iter.next() {
                                if c == '\\' {
                                    if let Some((_i, &c_nxt)) = chars_iter.next() {
                                        link_tit.push(c);
                                        link_tit.push(c_nxt);
                                        continue;
                                    } else {
                                        break 'match_link_title;
                                    }
                                } else if c == '(' && dl == '(' {
                                    break 'match_link_title; //cannot contain opening delimiter
                                //unescaped
                                } else if c == dlc {
                                    while let Some((i, &c_last)) = chars_iter.next() {
                                        if c_last == ' ' {
                                            continue;
                                        }
                                        if c_last == '\n' {
                                            link_tit_found = true;
                                            valid_lrd_found = true;
                                            characters_eaten = i + 1;
                                            dbg!("finished line good!");
                                            break 'match_link_title; //finished the line w/out
                                            //problem
                                        }
                                        break 'match_link_title; //found a bad character in line,
                                        //cant make title
                                    }
                                    // if we are here, it means we reached the end of the iterator
                                    link_tit_found = true;
                                    valid_lrd_found = true;
                                    characters_eaten = chars.len();
                                    break 'match_link_title; //found a bad character in line,
                                }
                                link_tit.push(c);
                            }
                        }
                        Some((_, _)) => break 'collect_lrds,
                        None => valid_lrd_found = true,
                    }
                }

                if valid_lrd_found {
                    lrd_table
                        .entry(normalize_label(link_lab))
                        .or_insert((link_dest, if link_tit_found { link_tit } else { vec![] }));
                    continue 'collect_lrds;
                }
                break 'collect_lrds;
            }

            if characters_eaten > 0 {
                let mut new_chars: Vec<char> =
                    chars[characters_eaten..].iter().map(|&c| c).collect();
                mem::swap(chars, &mut new_chars);
            }
            *b = false;
        }
        _ => unreachable!(),
    }

    *open_par_above = false;
    *open_par_exists = false;
}

fn normalize_label(label: Vec<char>) -> Vec<char> {
    let mut lab_out = vec![];
    let mut char_iter = label.iter();

    let mut seen_space = false;
    while let Some(&c) = char_iter.next() {
        if " \n".contains(c) {
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

#[cfg(test)]
mod cp_tests {
    use super::*;

    fn test_cp(
        ast: &mut Block,
        expected_ast: &mut Block,
        exp_table: &mut HashMap<Vec<char>, (Vec<char>, Vec<char>)>,
    ) {
        let (mut opa, mut ope) = (true, true);
        let mut lrd_table: HashMap<Vec<char>, (Vec<char>, Vec<char>)> = HashMap::new();
        close_paragraph(ast, &mut opa, &mut ope, &mut lrd_table);
        assert_eq!(
            (expected_ast, exp_table, false, false),
            (ast, &mut lrd_table, opa, ope)
        )
    }

    #[test]
    fn test_cp_no_def() {
        test_cp(
            &mut Document(vec![Paragraph("hello".chars().collect(), true)]),
            &mut Document(vec![Paragraph("hello".chars().collect(), false)]),
            &mut HashMap::new(),
        )
    }

    #[test]
    fn test_cp_no_title() {
        test_cp(
            &mut Document(vec![Paragraph("[hello]:link".chars().collect(), true)]),
            &mut Document(vec![Paragraph("".chars().collect(), false)]),
            &mut HashMap::from([(
                "hello".chars().collect(),
                ("link".chars().collect(), "".chars().collect()),
            )]),
        )
    }

    #[test]
    fn test_cp_title() {
        test_cp(
            &mut Document(vec![Paragraph(
                "[hello]:link (your_mom)".chars().collect(),
                true,
            )]),
            &mut Document(vec![Paragraph("".chars().collect(), false)]),
            &mut HashMap::from([(
                "hello".chars().collect(),
                ("link".chars().collect(), "your_mom".chars().collect()),
            )]),
        )
    }

    #[test]
    fn test_cp_multiline() {
        test_cp(
            &mut Document(vec![Paragraph(
                "[\nhel\nlo\n]:\nlink \n(your_mom)".chars().collect(),
                true,
            )]),
            &mut Document(vec![Paragraph("".chars().collect(), false)]),
            &mut HashMap::from([(
                "hel lo".chars().collect(),
                ("link".chars().collect(), "your_mom".chars().collect()),
            )]),
        )
    }
    #[test]
    fn test_cp_title_fail() {
        test_cp(
            &mut Document(vec![Paragraph(
                "[\nhel\nlo\n]:\nlink    \n(your_mom".chars().collect(),
                true,
            )]),
            &mut Document(vec![Paragraph("(your_mom".chars().collect(), false)]),
            &mut HashMap::from([(
                "hel lo".chars().collect(),
                ("link".chars().collect(), "".chars().collect()),
            )]),
        )
    }

    #[test]
    fn test_cp_link_fail_1() {
        test_cp(
            &mut Document(vec![Paragraph(
                "[\nhel\nlo\n]:\n)link    \n(your_mom".chars().collect(),
                true,
            )]),
            &mut Document(vec![Paragraph(
                "[\nhel\nlo\n]:\n)link    \n(your_mom".chars().collect(),
                false,
            )]),
            &mut HashMap::from([]),
        )
    }
    #[test]
    fn test_cp_link_fail_2() {
        test_cp(
            &mut Document(vec![Paragraph(
                "[\nhel\nlo\n]:\n(link(())    \n(your_mom".chars().collect(),
                true,
            )]),
            &mut Document(vec![Paragraph(
                "[\nhel\nlo\n]:\n(link(())    \n(your_mom".chars().collect(),
                false,
            )]),
            &mut HashMap::from([]),
        )
    }
    #[test]
    fn test_cp_more_paragraph() {
        test_cp(
            &mut Document(vec![Paragraph(
                "[\nhel\nlo\n]:\nlink    \n'your_mom'\nand theres more"
                    .chars()
                    .collect(),
                true,
            )]),
            &mut Document(vec![Paragraph("and theres more".chars().collect(), false)]),
            &mut HashMap::from([(
                "hel lo".chars().collect(),
                ("link".chars().collect(), "your_mom".chars().collect()),
            )]),
        )
    }
    #[test]
    fn test_cp_2_def() {
        test_cp(
            &mut Document(vec![Paragraph(
                "[\nhel\nlo\n]:\nlink    \n'your_mom'\n[def2]:link2 \"desc_2\""
                    .chars()
                    .collect(),
                true,
            )]),
            &mut Document(vec![Paragraph("".chars().collect(), false)]),
            &mut HashMap::from([
                (
                    "hel lo".chars().collect(),
                    ("link".chars().collect(), "your_mom".chars().collect()),
                ),
                (
                    "def2".chars().collect(),
                    ("link2".chars().collect(), "desc_2".chars().collect()),
                ),
            ]),
        )
    }
    #[test]
    fn test_cp_2_def_override() {
        test_cp(
            &mut Document(vec![Paragraph(
                "[\nhel\nlo\n]:\nlink    \n'your_mom'\n[hel    lo   ]:link2 \"desc_2\""
                    .chars()
                    .collect(),
                true,
            )]),
            &mut Document(vec![Paragraph("".chars().collect(), false)]),
            &mut HashMap::from([(
                "hel lo".chars().collect(),
                ("link".chars().collect(), "your_mom".chars().collect()),
            )]),
        )
    }
    #[test]
    fn test_cp_fails() {
        test_cp(
            &mut Document(vec![Paragraph(
                "[\nhel\nlo\n]:\n<link    \n'your_mom'\n[hel    lo   ]:link2 \"desc_2\""
                    .chars()
                    .collect(),
                true,
            )]),
            &mut Document(vec![Paragraph(
                "[\nhel\nlo\n]:\n<link    \n'your_mom'\n[hel    lo   ]:link2 \"desc_2\""
                    .chars()
                    .collect(),
                false,
            )]),
            &mut HashMap::from([]),
        )
    }
    #[test]
    fn test_cp_2_escapes() {
        test_cp(
            &mut Document(vec![Paragraph(
                "[\nh\\el\nlo\n]:\nlink    \n'your_mom'\n[hel    lo   ]:link2 \"desc_2\""
                    .chars()
                    .collect(),
                true,
            )]),
            &mut Document(vec![Paragraph("".chars().collect(), false)]),
            &mut HashMap::from([
                (
                    "h\\el lo".chars().collect(),
                    ("link".chars().collect(), "your_mom".chars().collect()),
                ),
                (
                    "hel lo".chars().collect(),
                    ("link2".chars().collect(), "desc_2".chars().collect()),
                ),
            ]),
        )
    }
}

fn create_new_block_starts(
    document: &mut Block,
    line: &Vec<char>,
    offset: &mut usize,
    obd: &mut usize,
    lrd_table: &mut HashMap<Vec<char>, (Vec<char>, Vec<char>)>,
) {
    let mut pre_space_count = space_indent_count(line, *offset);
    match document.get_block(*obd) {
        IndentedCodeBlock(chars, spaces) => {
            if is_blank_line(line, *offset) {
                spaces.push(line.len() - *offset);
            } else {
                for s in spaces {
                    chars.push('\n');
                    for _ in 0..*s {
                        chars.push(' ');
                    }
                }
                chars.push('\n');
                chars.append(&mut line[..*offset].iter().map(|c| *c).collect());
            }
        }
        FencedCodeBlock(chars, is_open @ true, marker, _, indent_count, marker_count) => {
            match fenced_code_block_encountered(line, *offset, pre_space_count) {
                Some(FencedCodeBlock(_, _, t, infstr, _, tc)) => {
                    if t == *marker && infstr.len() == 0 && tc >= *marker_count {
                        *is_open = false;
                        *offset = line.len();
                        *obd -= 1;
                        return;
                    }
                }
                _ => (),
            }
            chars.append(
                &mut line[..(*offset
                    + if pre_space_count > *indent_count {
                        *indent_count
                    } else {
                        pre_space_count
                    })]
                    .iter()
                    .map(|c| *c)
                    .collect(),
            );
        }
        _ => (),
    }
    let mut open_par_above = match document.get_block(*obd) {
        Paragraph(_, open) => *open,
        _b => false,
    };

    let mut open_par_exists = match document.get_last_block() {
        Paragraph(_, open) => *open,
        _ => false,
    };

    //now do things for when we have a blank line
    if line.len() <= *offset + pre_space_count {
        close_paragraph(
            document,
            &mut open_par_above,
            &mut open_par_exists,
            lrd_table,
        );
        // detighten lists
        if let (ListItem(_, _), list_item_depth) = document.get_general_container(*obd) {
            match document.get_block(list_item_depth - 1) {
                List(_, tight, _) => *tight = false,
                _ => unreachable!(),
            }
        }
        // make block quotes closed
        document.close_open_block();
        todo!();
    }

    // c is first non space character after offset
    let c = dbg!(line[*offset + pre_space_count]);

    // First check for SetextHeading
    if open_par_above && pre_space_count <= 3 {
        if "-=".contains(c) {
            // the line is of the form "[pre-matched-structure][1-3 space](-|=)*' '*"
            if line[..*offset + pre_space_count]
                .iter()
                .skip_while(|k| **k == c)
                .skip_while(|k| **k == ' ')
                .count()
                == 0
            {
                let mut h_text = vec![];
                mem::swap(
                    &mut h_text,
                    match document.get_block(*obd) {
                        Paragraph(inline, _) => inline,
                        _ => panic!(),
                    },
                );
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
    if "-*_".contains(c) && pre_space_count <= 3 {
        let mut non_space = line[..*offset].iter().filter(|k| **k != ' ');
        let count = non_space.clone().count();
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
        dbg!(last_block_list);
        match dbg!(list_item_encountered(line, *offset, pre_space_count)) {
            Some((lt, li, offset_dif)) => {
                if last_block_list.unwrap().same_list_eq(&dbg!(lt)) {
                    close_paragraph(
                        document,
                        &mut open_par_above,
                        &mut open_par_exists,
                        lrd_table,
                    );
                    match document.get_block(*obd) {
                        List(blocks, _, _) => blocks.push(li),
                        _ => unreachable!(),
                    }
                    *obd += 1;
                    pre_space_count = space_indent_count(line, *offset);
                    *offset += offset_dif;
                }
            }
            None => (),
        }
    }

    // keep looking for new block starts
    loop {
        if let Some((lt, list_item_block, offset_dif)) =
            list_item_encountered(line, *offset, pre_space_count)
        {
            if open_par_above {
                match lt {
                    OrderedList(_, 1) => (),
                    OrderedList(_, _) => break,
                    _ => (),
                }
            } // only ordered lists starting with 1 can interrupt paragraphs
            close_paragraph(
                document,
                &mut open_par_above,
                &mut open_par_exists,
                lrd_table,
            );
            let (parent, new_obd) = document.get_general_container(*obd);
            match parent {
                Document(blocks) | BlockQuote(blocks, _) | ListItem(blocks, _) => {
                    blocks.push(List(vec![list_item_block], true, lt));
                    *obd = new_obd + 2;
                    *offset += offset_dif;
                    pre_space_count = space_indent_count(line, *offset);
                    continue;
                }
                _ => unreachable!(),
            }
        }
        if let Some(offset_dif) = block_quote_encountered(line, *offset, pre_space_count) {
            close_paragraph(
                document,
                &mut open_par_above,
                &mut open_par_exists,
                lrd_table,
            );
            let (parent, new_obd) = document.get_general_container(*obd);
            match parent {
                Document(blocks) | BlockQuote(blocks, _) | ListItem(blocks, _) => {
                    blocks.push(BlockQuote(vec![], true));
                    *obd = new_obd + 1;
                    *offset += offset_dif;
                    pre_space_count = space_indent_count(line, *offset);
                    continue;
                }
                _ => unreachable!(),
            }
        }
        if let Some(fcb) = fenced_code_block_encountered(line, *offset, pre_space_count) {
            close_paragraph(
                document,
                &mut open_par_above,
                &mut open_par_exists,
                lrd_table,
            );
            let (parent, new_obd) = document.get_general_container(*obd);
            match parent {
                Document(blocks) | BlockQuote(blocks, _) | ListItem(blocks, _) => {
                    blocks.push(fcb);
                    *obd = new_obd + 1;
                    *offset = line.len();
                    return;
                }
                _ => unreachable!(),
            }
        }
        if let Some(atxh) = atx_heading_encountered(line, *offset, pre_space_count) {
            close_paragraph(
                document,
                &mut open_par_above,
                &mut open_par_exists,
                lrd_table,
            );
            let (parent, new_obd) = document.get_general_container(*obd);
            match parent {
                Document(blocks) | BlockQuote(blocks, _) | ListItem(blocks, _) => {
                    blocks.push(atxh);
                    *obd = new_obd + 1;
                    *offset = line.len();
                    return;
                }
                _ => unreachable!(),
            }
        }
        break;
    }

    // its a blank line from here on out
    if line.len() <= *offset + pre_space_count {
        *offset = line.len();
        return;
    }

    //now check if theres a paragraph we can continue lazily
    match document.get_last_block() {
        Paragraph(inline, true) => {
            inline.push('\n');
            inline.append(
                &mut line[..*offset + pre_space_count]
                    .iter()
                    .map(|c| *c)
                    .collect(),
            );
            *offset = line.len();
            return;
        }
        _ => (),
    }

    // if theres no paragraph to continue, check to create an indented code block
    if pre_space_count >= 4 {
        match document.get_general_container(*obd).0 {
            Document(blocks) | BlockQuote(blocks, _) | ListItem(blocks, _) => {
                blocks.push(IndentedCodeBlock(
                    line[..*offset + 4].iter().map(|c| *c).collect(),
                    vec![],
                ));
                *offset = line.len();
                return;
            }
            _ => unreachable!(),
        }
    }

    // finally, we can create a new paragraph with the line remaining
    match document.get_general_container(*obd).0 {
        Document(blocks) | BlockQuote(blocks, _) | ListItem(blocks, _) => {
            blocks.push(Paragraph(
                line[..*offset]
                    .iter()
                    .skip_while(|c| **c == ' ')
                    .map(|c| *c)
                    .collect(),
                true,
            ));
            *offset = line.len();
            return;
        }
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod cnbs_tests {
    use super::*;
    fn test_cnbs(ast: &mut Block, line: &str, expected_ast: &mut Block, exp_offset: usize) {
        let line: Vec<char> = line.chars().collect();
        // println!("{:?}", line);
        let mut obd = 0;
        let mut offset = 0;
        check_continuation_conditions(&ast, &line, &mut offset, &mut obd);
        create_new_block_starts(ast, &line, &mut offset, &mut obd, 0, &mut HashMap::new());
        assert_eq!((ast, offset), (expected_ast, exp_offset));
    }

    #[test]
    fn test_cnbs_1() {
        test_cnbs(
            &mut Document(vec![BlockQuote(
                vec![Paragraph(vec!['a', 'b'], true)],
                true,
            )]),
            ">-",
            &mut Document(vec![BlockQuote(
                vec![SetextHeading(vec!['a', 'b'], 2)],
                true,
            )]),
            1,
        );
    }

    #[test]
    fn test_cnbs_2() {
        test_cnbs(
            &mut Document(vec![Paragraph(vec!['a', 'b'], true)]),
            "-",
            &mut Document(vec![SetextHeading(vec!['a', 'b'], 2)]),
            0,
        );
    }

    #[test]
    fn test_cnbs_3() {
        test_cnbs(
            &mut Document(vec![Paragraph(vec!['a', 'b'], true)]),
            "=          ",
            &mut Document(vec![SetextHeading(vec!['a', 'b'], 1)]),
            10,
        );
    }

    #[test]
    fn test_cnbs_4() {
        test_cnbs(
            &mut Document(vec![Paragraph(vec!['a', 'b'], true)]),
            "- -",
            &mut Document(vec![
                Paragraph(vec!['a', 'b'], true),
                List(
                    vec![ListItem(
                        vec![List(vec![ListItem(vec![], 2)], true, UnorderedList('-'))],
                        2,
                    )],
                    true,
                    UnorderedList('-'),
                ),
            ]),
            3,
        );
    }

    #[test]
    fn test_cnbs_5() {
        test_cnbs(
            &mut Document(vec![BlockQuote(
                vec![Paragraph(vec!['a', 'b'], true)],
                false,
            )]),
            ">-",
            &mut Document(vec![
                BlockQuote(vec![Paragraph(vec!['a', 'b'], true)], false),
                BlockQuote(
                    vec![List(vec![ListItem(vec![], 2)], true, UnorderedList('-'))],
                    true,
                ),
            ]),
            2,
        );
    }

    #[test]
    fn test_cnbs_6() {
        test_cnbs(
            &mut Document(vec![BlockQuote(
                vec![Paragraph(vec!['a', 'b'], true)],
                true,
            )]),
            ">---",
            &mut Document(vec![BlockQuote(
                vec![SetextHeading(vec!['a', 'b'], 2)],
                true,
            )]),
            3,
        );
    }

    #[test]
    fn test_cnbs_7() {
        test_cnbs(
            &mut Document(vec![BlockQuote(
                vec![Paragraph(vec!['a', 'b'], true)],
                true,
            )]),
            ">***",
            &mut Document(vec![BlockQuote(
                vec![Paragraph(vec!['a', 'b'], true), ThematicBreak],
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

    #[test]
    fn test_cnbs_continue_list_item_1() {
        test_cnbs(
            &mut Document(vec![List(
                vec![ListItem(vec![ThematicBreak], 2)],
                true,
                UnorderedList('-'),
            )]),
            "-",
            &mut Document(vec![List(
                vec![ListItem(vec![ThematicBreak], 2), ListItem(vec![], 2)],
                true,
                UnorderedList('-'),
            )]),
            1,
        );
    }
    #[test]
    fn test_cnbs_continue_list_item_2() {
        test_cnbs(
            &mut Document(vec![List(
                vec![ListItem(vec![ThematicBreak], 2)],
                true,
                OrderedList('.', 2),
            )]),
            "123. ",
            &mut Document(vec![List(
                vec![ListItem(vec![ThematicBreak], 2), ListItem(vec![], 5)],
                true,
                OrderedList('.', 2),
            )]),
            5,
        );
    }

    #[test]
    fn test_cnbs_new_list_1() {
        test_cnbs(
            &mut Document(vec![]),
            "123. ",
            &mut Document(vec![List(
                vec![ListItem(vec![], 5)],
                true,
                OrderedList('.', 123),
            )]),
            5,
        );
    }

    #[test]
    fn test_cnbs_new_list_2() {
        test_cnbs(
            &mut Document(vec![List(
                vec![ListItem(vec![ThematicBreak], 2)],
                true,
                OrderedList('.', 2),
            )]),
            " -  ",
            &mut Document(vec![
                List(
                    vec![ListItem(vec![ThematicBreak], 2)],
                    true,
                    OrderedList('.', 2),
                ),
                List(vec![ListItem(vec![], 3)], true, UnorderedList('-')),
            ]),
            4,
        );
    }

    #[test]
    fn test_cnbs_atx_heading_1() {
        test_cnbs(
            &mut Document(vec![]),
            "#",
            &mut Document(vec![ATXHeading(vec![], 1)]),
            1,
        );
    }

    #[test]
    fn test_cnbs_atx_heading_2() {
        test_cnbs(
            &mut Document(vec![]),
            "  #           #       ",
            &mut Document(vec![ATXHeading(vec![], 1)]),
            22,
        );
    }

    #[test]
    fn test_cnbs_atx_heading_3() {
        test_cnbs(
            &mut Document(vec![]),
            "  #           #       hello # ",
            &mut Document(vec![ATXHeading("#       hello".chars().collect(), 1)]),
            30,
        );
    }
}
