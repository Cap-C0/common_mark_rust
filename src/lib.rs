use crate::Block::*;
use crate::HTMLEndCondition::*;
use crate::ListType::*;
use crate::ast_types::*;
use crate::inline::*;
use std::collections::HashMap;
use std::mem;
pub mod ast_types;
pub mod inline;

pub fn markdown_to_html(markdown: &str) -> String {
    // first split into lines

    // Ok, there might be clever ways to not need to use lines as Vec<char>, but I can do that later
    // TODO figure out how not split lines
    // use raw stringes all the way!
    let mut lines: Vec<Vec<char>> = markdown.split('\n').map(|x| x.chars().collect()).collect();
    if lines.last().expect("lines is non empty").len() == 0 {
        lines.pop(); // an erroneous extra line is not needed
    }

    // 1st create block structure of document
    let mut document = Document(vec![]);
    let mut lrd_table: HashMap<Vec<char>, (Vec<char>, Vec<char>)> = HashMap::new();
    let mut blank_line_depth: Option<usize> = None;

    for (line_num, line) in lines.iter().enumerate() {
        println!("reading line: {}", line_num);
        // first check continuation conditions
        let mut char_offset: usize = 0;
        // if there are no tabs in the line, then effective_column_number should stay equal to
        // char_offset
        let mut effective_column_number: usize = 0;
        // similarly, no tabs should imply that additional_possible_spaces is always 0.
        let mut additional_possible_spaces: usize = 0;
        let mut open_block_depth: usize = 0;
        check_continuation_conditions(
            &document,
            &line,
            &mut char_offset,
            &mut effective_column_number,
            &mut additional_possible_spaces,
            &mut open_block_depth,
        );
        println!("checked ccs line {}!", line_num);

        // now we look for the start of any new structure
        create_new_block_starts(
            &mut document,
            &line,
            &mut char_offset,
            &mut effective_column_number,
            &mut additional_possible_spaces,
            &mut open_block_depth,
            &mut blank_line_depth,
            &mut lrd_table,
        );
    }

    let mut open_par_exists = match document.get_last_block() {
        Paragraph(_, open) => *open,
        _ => false,
    };
    close_paragraph(
        &mut document,
        &mut false,
        &mut open_par_exists,
        &mut lrd_table,
    );

    dbg!(&document);

    document.to_html()
}

#[allow(unreachable_code)]
fn check_continuation_conditions(
    document: &Block,
    line: &Vec<char>,
    char_offset: &mut usize,
    effective_column_number: &mut usize,
    additional_possible_spaces: &mut usize,
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
                let (char_offset_after_spaces, eff_col_num_after_space) =
                    consume_effective_indent(line, *char_offset, *effective_column_number);
                if (eff_col_num_after_space - *effective_column_number)
                    + *additional_possible_spaces
                    <= 3
                    && line.len() > char_offset_after_spaces
                {
                    match block_quote_encountered(
                        line,
                        char_offset_after_spaces,
                        eff_col_num_after_space,
                    ) {
                        Some((
                            new_char_offset,
                            new_eff_column_number,
                            new_additional_possible_spaces,
                        )) => {
                            *char_offset = new_char_offset;
                            *effective_column_number = new_eff_column_number;
                            *additional_possible_spaces = new_additional_possible_spaces;
                            *open_block_depth += 1;
                            match blocks.last() {
                                None => break 'outer,
                                Some(b) => {
                                    current_block = b;
                                    continue 'outer;
                                }
                            }
                        }
                        None => break 'outer,
                    }
                }
                break 'outer;
            }
            List(list_items, _, _, _) => {
                *open_block_depth += 1;
                // lists are guaranteed to have at least one item
                current_block = list_items.last().unwrap();
            }
            ListItem(blocks, continuable, indent_amount) => {
                if !continuable {
                    break;
                }
                let mut chars_eaten = 0;
                let mut sub_column_number = *effective_column_number;
                while line.len() > *char_offset + chars_eaten
                    && ((sub_column_number - *effective_column_number)
                        + *additional_possible_spaces)
                        < *indent_amount
                {
                    if line[*char_offset + chars_eaten] == ' ' {
                        sub_column_number += 1;
                    } else if line[*char_offset + chars_eaten] == '\t' {
                        sub_column_number += 4 - (sub_column_number % 4);
                    } else {
                        break 'outer;
                    }
                    chars_eaten += 1;
                }

                if is_blank_line(line, *char_offset + chars_eaten) {
                    *char_offset = line.len(); // a blank line is auto allowed to match
                } else {
                    *additional_possible_spaces = (sub_column_number - *effective_column_number)
                        + *additional_possible_spaces
                        - *indent_amount;
                    *char_offset += chars_eaten;
                }
                *open_block_depth += 1;
                match blocks.last() {
                    None => break 'outer,
                    Some(b) => {
                        current_block = b;
                    }
                }
            }
            Heading(_, _) => break,
            Paragraph(_, is_open) => {
                if *is_open {
                    *open_block_depth += 1;
                }
                break;
            }
            ThematicBreak => break,
            IndentedCodeBlock(_, _) => {
                let (post_space_char_offset, post_space_eff_col) =
                    consume_effective_indent(line, *char_offset, *effective_column_number);
                let i = *effective_column_number - post_space_eff_col;
                // ie, there are non space chars in the first 4 chars
                if line.len() > post_space_char_offset && i < 4 {
                    break;
                }
                *char_offset = post_space_char_offset;
                //dont eat blank characters before the first 4 spaces
                *open_block_depth += 1;
                break;
            }
            FencedCodeBlock(_, is_open, _, _, _, _) => {
                if *is_open {
                    *open_block_depth += 1;
                }
                break;
            }
            HTMLBlock(_, is_open, _) => {
                if *is_open {
                    *open_block_depth += 1;
                }
                break;
            }
        }
    }
}

fn is_blank_line(line: &Vec<char>, offset: usize) -> bool {
    line[offset..].iter().all(|c| *c == ' ' || *c == '\t')
}

#[cfg(test)]
mod cc_tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn test_cc(ast: &mut Block, line: &str, expected_block: &mut Block, exp_offset: usize) {
        let line: Vec<char> = line.chars().collect();
        // println!("{:?}", line);
        let mut char_offset: usize = 0;
        let mut effective_column_number: usize = 0;
        let mut additional_possible_spaces: usize = 0;
        let mut open_block_depth: usize = 0;
        check_continuation_conditions(
            &ast,
            &line,
            &mut char_offset,
            &mut effective_column_number,
            &mut additional_possible_spaces,
            &mut open_block_depth,
        );
        assert_eq!(
            (ast.get_block(open_block_depth), char_offset),
            (expected_block, exp_offset)
        );
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
            vec![ListItem(vec![], true, 2)],
            true,
            UnorderedList('*'),
            false,
        )]);
        let mut exp_tree = ListItem(vec![], true, 2);
        test_cc(&mut test_tree, "  >  hello", &mut exp_tree, 2);
    }

    #[test]
    fn test_cc_list_2() {
        let mut test_tree = Document(vec![List(
            vec![ListItem(vec![], true, 2)],
            true,
            UnorderedList('*'),
            false,
        )]);
        let mut exp_tree = List(
            vec![ListItem(vec![], true, 2)],
            true,
            UnorderedList('*'),
            false,
        );
        test_cc(&mut test_tree, " >  hello", &mut exp_tree, 0);
    }

    #[test]
    fn test_cc_both_3() {
        let mut test_tree = Document(vec![List(
            vec![ListItem(vec![BlockQuote(vec![], true)], true, 2)],
            true,
            UnorderedList('*'),
            false,
        )]);
        let mut exp_tree = BlockQuote(vec![], true);
        test_cc(&mut test_tree, "  > hello", &mut exp_tree, 4);
    }
}

/// char_offset: how far you have to literally offset the line to get to the space character
/// effective_offset: on what "column" this space is aligning
/// returns: (new_char_offset, new_effective_column_number)
/// This is hard to think about, no wonder they start with it in the spec, cause it would have been good to
/// design around tabs in the first place.
fn consume_effective_indent(
    line: &Vec<char>,
    mut char_offset: usize,
    mut effective_column_number: usize,
) -> (usize, usize) {
    //in contexts where spaces help to define block structure, tabs behave as if they were replaced by spaces with a tab stop of 4 characters.
    while line.len() > char_offset && " \t".contains(line[char_offset]) {
        if line[char_offset] == '\t' {
            effective_column_number += 4 - (effective_column_number % 4);
        } else {
            effective_column_number += 1;
        }
        char_offset += 1;
    }
    return (char_offset, effective_column_number);
}

fn thematic_break_encountered(line: &Vec<char>, char_offset: usize) -> bool {
    let c = line[char_offset];
    if "-*_".contains(c) {
        let mut non_space = line[char_offset..].iter().filter(|&&k| !"\t ".contains(k));
        let count = non_space.clone().count();
        return count >= 3 && non_space.all(|k| *k == c);
    }
    false
}

/// returns hypothetically, if you "ate" and matched this, where the offsets would end up
/// returns Opt(listtype, listitem, new_char_offset, new_eff_column_number,
/// new additional_possible_spaces)
/// bounds checking is expected by caller
/// preceding spaces should be dealt with by caller
/// char_offset should be set on the first non-space char
fn list_item_encountered(
    line: &Vec<char>,
    char_offset: usize,
    eff_column_number: usize,
    psc: usize,
) -> Option<(ListType, Block, usize, usize, usize)> {
    let c = dbg!(line[char_offset]);
    if c.is_numeric() {
        let mut number_builder = String::from(line[char_offset]);
        for j in 1..10 {
            if line.len() <= char_offset + j {
                return None;
            }
            if line[char_offset + j].is_numeric() {
                (&mut number_builder).push(line[char_offset + j]);
                continue;
            }
            if ".)".contains(line[char_offset + j]) {
                let (post_space_char_offset, post_space_eff_col_number) =
                    consume_effective_indent(line, char_offset + j + 1, eff_column_number + j + 1);

                if line.len() <= post_space_char_offset {
                    // ie, the rest of the line is blank
                    return Some((
                        OrderedList(line[char_offset + j], number_builder.parse().unwrap()),
                        ListItem(vec![], true, psc + j + 2),
                        post_space_char_offset,
                        post_space_eff_col_number,
                        0,
                    ));
                }
                let following_space_count = post_space_eff_col_number - eff_column_number - j - 1;
                if 1 <= following_space_count && following_space_count <= 4 {
                    return Some((
                        OrderedList(line[char_offset + j], number_builder.parse().unwrap()),
                        ListItem(vec![], true, psc + j + 1 + following_space_count),
                        post_space_char_offset,
                        post_space_eff_col_number,
                        0, // a tab character immediately following the list delimiter will be
                           // entirely consumed in this manner
                    ));
                }
                if following_space_count > 4 {
                    let (new_eff_col_number, possible_spaces) = if line[char_offset + j + 1] == '\t'
                    {
                        let tmp = eff_column_number + 1;
                        let dist_to_new_stop = 4 - (tmp % 4);
                        (tmp + dist_to_new_stop, dist_to_new_stop - 1)
                    } else {
                        (eff_column_number + j + 1, 0)
                    };
                    return Some((
                        OrderedList(line[char_offset + j], number_builder.parse().unwrap()),
                        ListItem(vec![], true, psc + j + 2),
                        char_offset + j + 2,
                        new_eff_col_number,
                        possible_spaces,
                    ));
                }
            }
            return None;
        }
    }

    if "-+*".contains(line[char_offset]) {
        let (post_space_char_offset, post_space_eff_col_number) =
            consume_effective_indent(line, char_offset + 1, eff_column_number + 1);

        if line.len() <= post_space_char_offset {
            // ie, the rest of the line is blank
            return Some((
                UnorderedList(line[char_offset]),
                ListItem(vec![], true, psc + 2),
                post_space_char_offset,
                post_space_eff_col_number,
                0,
            ));
        }
        let following_space_count = post_space_eff_col_number - eff_column_number - 1;
        if 1 <= following_space_count && following_space_count <= 4 {
            return Some((
                UnorderedList(line[char_offset]),
                ListItem(vec![], true, psc + 1 + following_space_count),
                post_space_char_offset,
                post_space_eff_col_number,
                0, // a tab character immediately following the list delimiter will be
                   // entirely consumed in this manner
            ));
        }
        if following_space_count > 4 {
            let (new_eff_col_number, possible_spaces) = if line[char_offset + 1] == '\t' {
                let tmp = eff_column_number + 1;
                let dist_to_new_stop = 4 - (tmp % 4);
                (tmp + dist_to_new_stop, dist_to_new_stop - 1)
            } else {
                (eff_column_number + 2, 0)
            };
            return Some((
                UnorderedList(line[char_offset]),
                ListItem(vec![], true, psc + 2),
                char_offset + 2,
                new_eff_col_number,
                possible_spaces,
            ));
        }
    }
    return None;
}

/// returns hypothetically, if you "ate" and matched this, where the offsets would end up
/// returns Opt(new_char_offset, new_eff_column_number, additional_possible_spaces)
/// bounds checking is expected by caller
/// preceding spaces should be dealt with by caller ie, line[char_offset] should be not space
/// char_offset should be set on the first non-space char
fn block_quote_encountered(
    line: &Vec<char>,
    char_offset_after_space: usize,
    eff_column_number: usize,
) -> Option<(usize, usize, usize)> {
    if line[char_offset_after_space] == '>' {
        if line.len() <= char_offset_after_space + 1 {
            return Some((char_offset_after_space + 1, eff_column_number + 1, 0));
        }
        if line[char_offset_after_space + 1] == ' ' {
            return Some((char_offset_after_space + 2, eff_column_number + 2, 0));
        } else if line[char_offset_after_space + 1] == '\t' {
            let new_eff_col = eff_column_number + 1; // since there is the > being consumed
            let dist_to_new_stop = 4 - (new_eff_col % 4);
            return Some((
                char_offset_after_space + 2,
                new_eff_col + dist_to_new_stop,
                dist_to_new_stop - 1,
            ));
            // dist_to_new_stop - 1 cause the block quote "eats" one of the spaces.
        } else {
            return Some((char_offset_after_space + 1, eff_column_number + 1, 0));
        }
    }
    None
}

fn fenced_code_block_encountered(
    line: &Vec<char>,
    char_offset_after_space: usize,
    psc: usize,
) -> Option<Block> {
    let t = line[char_offset_after_space];
    if "~`".contains(t) {
        let tick_count = line[char_offset_after_space..]
            .iter()
            .take_while(|c| **c == t)
            .count();
        if tick_count >= 3 {
            if t == '`' {
                if (&line[char_offset_after_space + tick_count..])
                    .iter()
                    .any(|c| *c == '`')
                {
                    return None;
                }
            }
            let info_string: Vec<char> = line[char_offset_after_space + tick_count..]
                .iter()
                .skip_while(|c| **c == ' ')
                .take_while(|c| **c != ' ')
                .map(|c| *c)
                .collect();
            return Some(FencedCodeBlock(
                vec![],
                true,
                t,
                info_string,
                psc,
                tick_count,
            ));
        }
    }
    return None;
}

fn atx_heading_encountered(line: &Vec<char>, char_offset_after_space: usize) -> Option<Block> {
    let t = line[char_offset_after_space];
    if t == '#' {
        let pound_count = line[char_offset_after_space..]
            .iter()
            .take_while(|c| **c == '#')
            .count();
        if line.len() <= char_offset_after_space + pound_count {
            return Some(Heading(vec![], pound_count));
        }
        if pound_count <= 6 && " \t".contains(line[char_offset_after_space + pound_count]) {
            // since there is no more structure matching, column number doesnt matter
            let (offset_after_space, _) =
                consume_effective_indent(line, char_offset_after_space + pound_count, 0);

            let mut inline_text: Vec<char> = vec![];
            line[offset_after_space..].iter().enumerate().fold(
                (None, None, None),
                |point_tup @ (pre_space_start, pounds_start, post_space_start), (o, &c)| {
                    if c == '#' {
                        if pounds_start.is_none()
                            && (post_space_start.is_some() || inline_text.is_empty())
                        {
                            return (
                                if inline_text.is_empty()
                                // ie, immediately see the pound after
                                // initial spaces
                                {
                                    None
                                } else {
                                    post_space_start
                                },
                                Some(offset_after_space + o),
                                None,
                            );
                        }
                        if pounds_start.is_some() {
                            return point_tup;
                        }
                    }
                    if " \t".contains(c) {
                        if post_space_start.is_none() {
                            return (pre_space_start, pounds_start, Some(offset_after_space + o));
                        }
                        return point_tup;
                    }
                    if let Some(pre_space) = pre_space_start {
                        for &spc_c in &line[pre_space..pounds_start.unwrap()] {
                            inline_text.push(spc_c);
                        }
                    }
                    if let Some(pounds) = pounds_start {
                        for &spc_c in
                            &line[pounds..post_space_start.unwrap_or(offset_after_space + o)]
                        {
                            inline_text.push(spc_c);
                        }
                    }
                    if let Some(post_space) = post_space_start {
                        for &spc_c in &line[post_space..offset_after_space + o] {
                            inline_text.push(spc_c);
                        }
                    }
                    inline_text.push(c);
                    return (None, None, None);
                },
            );

            return Some(Heading(inline_text, pound_count));
        }
    }
    None
}

fn html_start_encountered(
    line: &Vec<char>,
    char_offset_before_space: usize,
    open_par_above: bool,
) -> Option<Block> {
    let (char_offset_after_space, _) = consume_effective_indent(line, char_offset_before_space, 0);
    let simple_starts_with = |str: &'static str| -> Box<dyn Fn(&[char]) -> bool> {
        Box::new(move |line_in: &[char]| {
            return line_in.len() >= str.len()
                && line_in[..str.len()] == str.chars().collect::<Vec<char>>();
        })
    };
    let special_tag: Box<dyn Fn(&[char]) -> bool> = Box::new(|line_in: &[char]| -> bool {
        ["<pre", "<script", "<style", "<textarea"].iter().any(|s| {
            simple_starts_with(s)(line_in)
                && (line_in.len() == s.len() || " \t>".contains(line_in[s.len()]))
        })
    });
    let exclamation: Box<dyn Fn(&[char]) -> bool> = Box::new(|line_in: &[char]| -> bool {
        let start = "<!";
        return line_in.len() >= start.len() + 1
            && line_in[..start.len()] == start.chars().collect::<Vec<char>>()
            && line_in[2].is_alphabetic();
    });
    for (start_con, end_strs) in [
        (
            special_tag,
            vec!["</pre>", "</script>", "</style>", "</textarea>"],
        ),
        (simple_starts_with("<!--"), vec!["-->"]),
        (simple_starts_with("<?"), vec!["?>"]),
        (exclamation, vec![">"]),
        (simple_starts_with("<![CDATA["), vec!["]]>"]),
    ] {
        if start_con(&line[char_offset_after_space..]) {
            return Some(HTMLBlock(
                line[char_offset_before_space..]
                    .iter()
                    .map(|&c| c)
                    .collect(),
                !end_strs.iter().any(|s| {
                    line[char_offset_after_space..].windows(s.len()).any(|cs| {
                        cs.iter()
                            .map(|&c| c.to_ascii_lowercase())
                            .collect::<Vec<char>>()
                            == s.chars().collect::<Vec<char>>()
                    })
                }),
                ContainsStrings(end_strs.iter().map(|s| s.chars().collect()).collect()),
            ));
        }
    }

    let reserved_tags = [
        "address",
        "article",
        "aside",
        "base",
        "basefont",
        "blockquote",
        "body",
        "caption",
        "center",
        "col",
        "colgroup",
        "dd",
        "details",
        "dialog",
        "dir",
        "div",
        "dl",
        "dt",
        "fieldset",
        "figcaption",
        "figure",
        "footer",
        "form",
        "frame",
        "frameset",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "head",
        "header",
        "hr",
        "html",
        "iframe",
        "legend",
        "li",
        "link",
        "main",
        "menu",
        "menuitem",
        "nav",
        "noframes",
        "ol",
        "optgroup",
        "option",
        "p",
        "param",
        "search",
        "section",
        "summary",
        "table",
        "tbody",
        "td",
        "tfoot",
        "th",
        "thead",
        "title",
        "tr",
        "track",
        "ul",
    ];

    if line[char_offset_after_space] == '<' {
        let mut is_open_tag = true;
        let mut tag_offset = char_offset_after_space + 1;
        if line.len() > tag_offset && dbg!(line[tag_offset]) == '/' {
            tag_offset += 1;
            is_open_tag = false;
        }
        dbg!(tag_offset);
        for rt in reserved_tags {
            if simple_starts_with(rt)(&line[tag_offset..])
                && (line.len() == tag_offset + rt.len()
                    || " \t>".contains(line[tag_offset + rt.len()])
                    || simple_starts_with("/>")(&line[tag_offset + rt.len()..]))
            {
                return Some(HTMLBlock(
                    line[char_offset_before_space..]
                        .iter()
                        .map(|&c| c)
                        .collect(),
                    true,
                    BlankLine,
                ));
            }
        }

        let parse_attribute = |line_in: &[char]| -> Option<usize> {
            let mut offset = 0;
            while " \t".contains(line_in[offset]) {
                offset += 1;
            }
            if line_in.len() <= offset
                || !("_:".contains(line_in[offset]) || line_in[offset].is_alphabetic())
            {
                return None;
            }
            offset += 1;
            while line_in.len() > offset
                && ("_.:-".contains(line_in[offset]) || line_in[offset].is_ascii_alphanumeric())
            {
                offset += 1;
            }
            let pre_val_offset = offset;

            while line_in.len() > offset && " \t".contains(line_in[offset]) {
                offset += 1;
            }
            if line_in.len() <= offset || line_in[offset] != '=' {
                return Some(pre_val_offset);
            }
            offset += 1;
            while line_in.len() > offset && " \t".contains(line_in[offset]) {
                offset += 1;
            }
            if line_in.len() <= offset {
                return Some(pre_val_offset);
            }
            let c = line_in[offset];
            if "\"\'".contains(c) {
                offset += 1;
                while line_in.len() > offset && line_in[offset] != c {
                    offset += 1;
                }
                offset += 1;
                return Some(offset);
            }
            while line_in.len() > offset && !" \t\"\'=<>`".contains(line_in[offset]) {
                offset += 1;
            }
            Some(offset)
        };
        // non reserved (type 7) cant interrupt paragraph
        if open_par_above {
            return None;
        }

        //parse tag name
        if line.len() <= tag_offset || !line[tag_offset].is_ascii_alphabetic() {
            return None;
        }
        while line.len() > tag_offset
            && (line[tag_offset] == '-' || line[tag_offset].is_ascii_alphanumeric())
        {
            tag_offset += 1
        }
        dbg!(tag_offset);

        //optional attributes
        if is_open_tag {
            while let Some(x) = dbg!(parse_attribute(&line[tag_offset..])) {
                tag_offset += x;
                dbg!(&line[tag_offset..]);
            }
        }
        //trailing white space
        while line.len() > tag_offset && " \t".contains(line[tag_offset]) {
            tag_offset += 1;
        }

        if is_open_tag && line.len() > tag_offset && line[tag_offset] == '/' {
            tag_offset += 1;
        }

        if line.len() > tag_offset && line[tag_offset] == '>' {
            tag_offset += 1;
        } else {
            return None;
        }

        if line[tag_offset..].iter().all(|&c| " \t".contains(c)) {
            return Some(HTMLBlock(
                line[char_offset_before_space..]
                    .iter()
                    .map(|&c| c)
                    .collect(),
                true,
                BlankLine,
            ));
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
            let mut char_offset = 0;
            let mut chars_eaten = 0;
            'collect_lrds: loop {
                let link_lab: (usize, usize);
                if char_offset < chars.len()
                    && let Some(offset_dif) = parse_link_label(&chars[char_offset..])
                {
                    link_lab = (char_offset, char_offset + offset_dif);
                    char_offset += offset_dif;
                } else {
                    break 'collect_lrds;
                }

                if char_offset >= chars.len() || chars[char_offset] != ':' {
                    break 'collect_lrds;
                }
                char_offset += 1;

                //optional spaces or tabs
                while char_offset < chars.len() && chars[char_offset].is_whitespace() {
                    char_offset += 1;
                }

                let link_dest: (usize, usize);
                if char_offset < chars.len()
                    && let Some((offset_dif, in_brackets)) =
                        parse_link_destination(&chars[char_offset..])
                {
                    link_dest = if in_brackets {
                        (char_offset + 1, char_offset + offset_dif - 1)
                    } else {
                        (char_offset, char_offset + offset_dif)
                    };
                    char_offset += offset_dif;
                } else {
                    break 'collect_lrds;
                }

                let mut is_valid_lrd = false;
                while char_offset < chars.len() && " \t".contains(chars[char_offset]) {
                    char_offset += 1;
                }
                if char_offset >= chars.len() || chars[char_offset] == '\n' {
                    is_valid_lrd = true;
                    if char_offset < chars.len() {
                        char_offset += 1;
                    }
                }

                // the way we created paragraphs means that pre_whitespace in line is already stripped
                // so !c.is_whitespace() is true for all c that follow an \n
                let mut link_title: Option<(usize, usize)> = None;
                if char_offset < chars.len()
                    && let Some(offset_dif) = parse_link_title(&chars[char_offset..])
                {
                    // then it must be followed by whitespace until the end of its line.
                    let mut final_whitespace_pointer = char_offset + offset_dif;
                    while chars.len() > final_whitespace_pointer
                        && " \t".contains(chars[final_whitespace_pointer])
                    {
                        final_whitespace_pointer += 1;
                    }
                    if final_whitespace_pointer <= chars.len()
                        || chars[final_whitespace_pointer] == '\n'
                    {
                        link_title = Some((char_offset + 1, char_offset + offset_dif - 1));
                        char_offset = final_whitespace_pointer
                            + if final_whitespace_pointer >= chars.len() {
                                0
                            } else {
                                1
                            };
                        is_valid_lrd = true;
                    }
                }

                if is_valid_lrd {
                    lrd_table
                        .entry(normalize_label(
                            chars[link_lab.0..link_lab.1].iter().map(|&c| c).collect(),
                        ))
                        .or_insert((
                            chars[link_dest.0..link_dest.1].iter().map(|&c| c).collect(),
                            link_title.map_or(vec![], |(start, end)| {
                                chars[start..end].iter().map(|&c| c).collect()
                            }),
                        ));
                    chars_eaten = char_offset;
                    continue 'collect_lrds;
                }
                break 'collect_lrds;
            }

            if chars_eaten > 0 {
                let mut new_chars: Vec<char> = chars[chars_eaten..].iter().map(|&c| c).collect();
                mem::swap(chars, &mut new_chars);
            }
            *b = false;
        }
        _ => unreachable!(),
    }

    *open_par_above = false;
    *open_par_exists = false;
}

#[cfg(test)]
mod cp_tests {
    use super::*;
    use pretty_assertions::assert_eq;

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
    char_offset: &mut usize,
    effective_column_number: &mut usize,
    additional_possible_spaces: &mut usize,
    obd: &mut usize,
    blank_line_depth: &mut Option<usize>,
    lrd_table: &mut HashMap<Vec<char>, (Vec<char>, Vec<char>)>,
) {
    let (mut char_offset_after_spaces, mut eff_col_num_after_space) =
        consume_effective_indent(line, *char_offset, *effective_column_number);
    let mut pre_space_count =
        (eff_col_num_after_space - *effective_column_number) + *additional_possible_spaces;

    let mut open_par_above = match document.get_block(*obd) {
        Paragraph(_, open) => *open,
        _b => false,
    };

    let mut open_par_exists = match document.get_last_block() {
        Paragraph(_, open) => *open,
        _ => false,
    };

    //now do things for if we have a blank line
    'blank_line: {
        if line.len() <= char_offset_after_spaces {
            match document.get_block(*obd) {
                IndentedCodeBlock(_, unrealized_blanks) => {
                    let mut next_blank = vec![];
                    if line.len() > *char_offset {
                        next_blank = line[*char_offset..].iter().map(|&c| c).collect();
                    }
                    unrealized_blanks.push(next_blank);
                }
                FencedCodeBlock(..) => break 'blank_line,
                HTMLBlock(_, true, ContainsStrings(_)) => break 'blank_line,
                HTMLBlock(_, is_open @ true, BlankLine) => *is_open = false,
                ListItem(blocks, continuable, _) => {
                    if blocks.is_empty() {
                        *continuable = false;
                    }
                }
                _ => (),
            }
            close_paragraph(
                document,
                &mut open_par_above,
                &mut open_par_exists,
                lrd_table,
            );
            *blank_line_depth = Some(document.deepest_matched_blockquote(*obd));
            // make block quotes closed
            document.close_open_block(*obd as i32);
            return;
        }
    }

    match document.get_block(*obd) {
        IndentedCodeBlock(chars, spaces) => {
            for s in spaces.iter() {
                chars.push('\n');
                for c in s {
                    chars.push(*c);
                }
            }
            *spaces = vec![];
            chars.push('\n');
            chars.append(&mut line[*char_offset..].iter().map(|c| *c).collect());
            return;
        }
        FencedCodeBlock(chars, is_open @ true, marker, _, indent_count, marker_count) => {
            if pre_space_count <= 3 && line.len() > char_offset_after_spaces {
                match fenced_code_block_encountered(line, char_offset_after_spaces, pre_space_count)
                {
                    Some(FencedCodeBlock(_, _, t, infstr, _, tc)) => {
                        if t == *marker && infstr.len() == 0 && tc >= *marker_count {
                            *is_open = false;
                            *char_offset = line.len();
                            *obd -= 1;
                            return;
                        }
                    }
                    _ => (),
                }
            }
            println!("line: {:?} can be added!", &line[*char_offset..]);
            //TODO make this recognize tabs
            chars.append(
                &mut line[*char_offset + std::cmp::min(pre_space_count, *indent_count)..]
                    .iter()
                    .map(|c| *c)
                    .collect(),
            );
            chars.push('\n');
            return;
        }
        HTMLBlock(chars, is_open @ true, end_condition) => {
            dbg!("HTMLBlock parent matched!");
            if let ContainsStrings(strs) = end_condition {
                for s in strs {
                    if line[*char_offset..].windows(s.len()).any(|cs| {
                        cs.iter()
                            .map(|&c| c.to_ascii_lowercase())
                            .collect::<Vec<char>>()
                            == *s
                    }) {
                        *is_open = false;
                    }
                }
            }
            chars.push('\n');
            for &c in &line[*char_offset..] {
                chars.push(c);
            }
            return;
        }
        _ => (),
    }
    // c is first non space character after offset
    let c = line[char_offset_after_spaces];

    // First check for SetextHeading
    'setext_check: {
        if open_par_above && pre_space_count <= 3 {
            if "-=".contains(c) {
                // the line is of the form "[pre-matched-structure][1-3 space](-|=)*' '*"
                if line[*char_offset + pre_space_count..]
                    .iter()
                    .skip_while(|k| **k == c)
                    .skip_while(|k| " \t".contains(**k))
                    .count()
                    == 0
                {
                    close_paragraph(
                        document,
                        &mut open_par_above,
                        &mut open_par_exists,
                        lrd_table,
                    );
                    let mut h_text = vec![];
                    match document.get_block(*obd) {
                        Paragraph(inline, _) => {
                            //parsing tags can "empty" the paragraph
                            if inline.is_empty() {
                                break 'setext_check;
                            }
                            mem::swap(&mut h_text, inline)
                        }
                        _ => panic!(),
                    };
                    *blank_line_depth = None;
                    match document.get_block(*obd - 1) {
                    // by definition some container block
                    Document(blocks) |
                    BlockQuote(blocks, _) |
                    // List(blocks, _, list_type) => the only children of a list are list_blocks
                    ListItem(blocks,_, _) => {
                        *blocks.last_mut().unwrap() = Heading(h_text, if c == '=' {1} else {2});
                    }
                    _ => panic!("should be unreachable!"),
                }
                    *char_offset = line.len() - 1;
                    return;
                }
            }
        }
    }

    // obd is unmodified at this point, since we know we are not in a blank line at  this point,
    // that means that *something* will eventually get added to list item
    let mut added_to_list = None;
    if let (ListItem(_, _, _), depth) = document.get_general_container(*obd) {
        added_to_list = Some(depth - 1);
    }

    // next check for thematic break (before we add to list for priority reasons)
    // because it is established non empty, bounds check is not needed
    if pre_space_count <= 3 && thematic_break_encountered(line, *char_offset) {
        close_paragraph(
            document,
            &mut open_par_above,
            &mut open_par_exists,
            lrd_table,
        );
        if let Some(list_depth) = added_to_list {
            match document.get_block(list_depth) {
                List(_, is_tight, _, _blank_line_encountered) => {
                    *is_tight = blank_line_depth.map_or(true, |d| d > list_depth) && *is_tight
                }
                _ => unreachable!(),
            }
        }
        *blank_line_depth = None;
        match (document.get_general_container(*obd)).0 {
            Document(blocks) | BlockQuote(blocks, _) | ListItem(blocks, _, _) => {
                blocks.push(ThematicBreak);
                *char_offset = line.len() - 1;
                return;
            }
            _ => panic!("should only match on general container"),
        }
    }

    //now check if we can add a new list item to an existing list
    let last_block_list: Option<ListType> = match document.get_block(*obd) {
        List(_, _, lt, _) => Some(lt.clone()),
        _ => None,
    };

    if last_block_list.is_some() {
        dbg!(last_block_list);
        dbg!(pre_space_count);
        if pre_space_count <= 3 || line.len() == char_offset_after_spaces {
            match list_item_encountered(
                line,
                char_offset_after_spaces,
                *effective_column_number,
                pre_space_count,
            ) {
                Some((lt, li, new_char_offset, new_eff_column_number, new_aps)) => {
                    if last_block_list.unwrap().same_list_eq(&lt) {
                        close_paragraph(
                            document,
                            &mut open_par_above,
                            &mut open_par_exists,
                            lrd_table,
                        );
                        added_to_list = Some(*obd);
                        match document.get_block(*obd) {
                            List(blocks, _, _, _) => blocks.push(li),
                            _ => unreachable!(),
                        }
                        *obd += 1;
                        *char_offset = new_char_offset;
                        *effective_column_number = new_eff_column_number;
                        *additional_possible_spaces = new_aps;
                        (char_offset_after_spaces, eff_col_num_after_space) =
                            consume_effective_indent(line, *char_offset, *effective_column_number);

                        pre_space_count = (eff_col_num_after_space - *effective_column_number)
                            + *additional_possible_spaces;
                    }
                }
                None => (),
            }
        }
    }

    // at this point, if the last matched block is a list item, then we can let potentially
    // non-tight lists actually be non-tight
    if let Some(list_depth) = added_to_list {
        match document.get_block(list_depth) {
            List(_, is_tight, _, _blank_line_encountered) => {
                *is_tight = blank_line_depth.map_or(true, |d| d > list_depth) && *is_tight
            }
            _ => unreachable!(),
        }
    }
    *blank_line_depth = None;

    // keep looking for new block starts
    loop {
        // better to do one psc_check for all the structures than have each structure function do it
        if (eff_col_num_after_space - *effective_column_number) + *additional_possible_spaces > 3
            || line.len() == char_offset_after_spaces
        {
            break;
        }
        // next check for thematic break
        if thematic_break_encountered(line, *char_offset) {
            close_paragraph(
                document,
                &mut open_par_above,
                &mut open_par_exists,
                lrd_table,
            );
            match document.get_general_container(*obd).0 {
                Document(blocks) | BlockQuote(blocks, _) | ListItem(blocks, _, _) => {
                    blocks.push(ThematicBreak);
                    *char_offset = line.len() - 1;
                    return;
                }
                _ => panic!("should only match on general container"),
            }
        }
        if let Some((
            lt,
            list_item_block,
            new_char_offset,
            new_eff_column_number,
            new_additional_possible_spaces,
        )) = list_item_encountered(
            line,
            char_offset_after_spaces,
            pre_space_count,
            pre_space_count,
        ) {
            //TODO: make empty lists not interrupt either
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
                Document(blocks) | BlockQuote(blocks, _) | ListItem(blocks, _, _) => {
                    blocks.push(List(vec![list_item_block], true, lt, false));
                    *obd = new_obd + 2;
                    *char_offset = new_char_offset;
                    *effective_column_number = new_eff_column_number;
                    *additional_possible_spaces = new_additional_possible_spaces;
                    (char_offset_after_spaces, eff_col_num_after_space) =
                        consume_effective_indent(line, *char_offset, *effective_column_number);
                    pre_space_count = (eff_col_num_after_space - *effective_column_number)
                        + *additional_possible_spaces;
                    continue;
                }
                _ => unreachable!(),
            }
        }
        if let Some((new_char_offset, new_eff_column_number, new_additional_possible_spaces)) =
            block_quote_encountered(line, char_offset_after_spaces, eff_col_num_after_space)
        {
            close_paragraph(
                document,
                &mut open_par_above,
                &mut open_par_exists,
                lrd_table,
            );
            let (parent, new_obd) = document.get_general_container(*obd);
            match parent {
                Document(blocks) | BlockQuote(blocks, _) | ListItem(blocks, _, _) => {
                    blocks.push(BlockQuote(vec![], true));
                    *obd = new_obd + 1;
                    *char_offset = new_char_offset;
                    *effective_column_number = new_eff_column_number;
                    *additional_possible_spaces = new_additional_possible_spaces;
                    (char_offset_after_spaces, eff_col_num_after_space) =
                        consume_effective_indent(line, *char_offset, *effective_column_number);
                    pre_space_count = (eff_col_num_after_space - *effective_column_number)
                        + *additional_possible_spaces;
                    continue;
                }
                _ => unreachable!(),
            }
        }
        if let Some(fcb) = fenced_code_block_encountered(line, *char_offset, pre_space_count) {
            close_paragraph(
                document,
                &mut open_par_above,
                &mut open_par_exists,
                lrd_table,
            );
            let (parent, new_obd) = document.get_general_container(*obd);
            match parent {
                Document(blocks) | BlockQuote(blocks, _) | ListItem(blocks, _, _) => {
                    blocks.push(fcb);
                    *obd = new_obd + 1;
                    *char_offset = line.len();
                    return;
                }
                _ => unreachable!(),
            }
        }
        if let Some(atxh) = atx_heading_encountered(line, char_offset_after_spaces) {
            close_paragraph(
                document,
                &mut open_par_above,
                &mut open_par_exists,
                lrd_table,
            );
            let (parent, new_obd) = document.get_general_container(*obd);
            match parent {
                Document(blocks) | BlockQuote(blocks, _) | ListItem(blocks, _, _) => {
                    blocks.push(atxh);
                    *obd = new_obd + 1;
                    *char_offset = line.len();
                    return;
                }
                _ => unreachable!(),
            }
        }
        if let Some(html) = html_start_encountered(line, *char_offset, open_par_above) {
            close_paragraph(
                document,
                &mut open_par_above,
                &mut open_par_exists,
                lrd_table,
            );
            let (parent, new_obd) = document.get_general_container(*obd);
            match parent {
                Document(blocks) | BlockQuote(blocks, _) | ListItem(blocks, _, _) => {
                    blocks.push(html);
                    *obd = new_obd + 1;
                    *char_offset = line.len();
                    return;
                }
                _ => unreachable!(),
            }
        }
        break;
    }

    // its a blank line from here on out
    if line.len() <= char_offset_after_spaces {
        *char_offset = line.len();
        return;
    }

    //now check if theres a paragraph we can continue lazily
    match document.get_last_block() {
        Paragraph(inline, true) => {
            inline.push('\n');
            inline.append(
                &mut line[char_offset_after_spaces..]
                    .iter()
                    .map(|c| *c)
                    .collect(),
            );
            *char_offset = line.len();
            return;
        }
        _ => (),
    }

    // if theres no paragraph to continue, check to create an indented code block
    if pre_space_count >= 4 {
        match document.get_general_container(*obd).0 {
            Document(blocks) | BlockQuote(blocks, _) | ListItem(blocks, _, _) => {
                // "eat" the first four spaces
                let mut space_count = *additional_possible_spaces;
                let mut my_column_number = *effective_column_number;
                dbg!(space_count);
                let mut ffs_char_offset = 0; // first four spaces
                while space_count < 4 {
                    if line[*char_offset + ffs_char_offset] == ' ' {
                        space_count += 1;
                        my_column_number += 1;
                    } else if line[*char_offset + ffs_char_offset] == '\t' {
                        space_count += dbg!(4 - (my_column_number % 4));
                        my_column_number += 4 - my_column_number % 4;
                    } else {
                        panic!()
                    }
                    ffs_char_offset += 1;
                }
                dbg!(space_count);
                let mut line_start = vec![' '; space_count - 4];
                for c in &line[*char_offset + ffs_char_offset..] {
                    line_start.push(*c);
                }
                blocks.push(IndentedCodeBlock(line_start, vec![]));
                *char_offset = line.len();
                return;
            }
            _ => unreachable!(),
        }
    }

    // finally, we can create a new paragraph with the line remaining
    match document.get_general_container(*obd).0 {
        Document(blocks) | BlockQuote(blocks, _) | ListItem(blocks, _, _) => {
            blocks.push(Paragraph(
                line[*char_offset..]
                    .iter()
                    .skip_while(|c| **c == ' ')
                    .map(|c| *c)
                    .collect(),
                true,
            ));
            *char_offset = line.len();
            return;
        }
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod cnbs_tests {
    use super::*;
    use pretty_assertions::assert_eq;
    fn test_cnbs(ast: &mut Block, line: &str, expected_ast: &mut Block, exp_offset: usize) {
        let line: Vec<char> = line.chars().collect();
        // println!("{:?}", line);
        let mut char_offset: usize = 0;
        let mut effective_column_number: usize = 0;
        let mut additional_possible_spaces: usize = 0;
        let mut obd = 0;
        let mut blank_line_depth = None;
        check_continuation_conditions(
            &ast,
            &line,
            &mut char_offset,
            &mut effective_column_number,
            &mut additional_possible_spaces,
            &mut obd,
        );
        create_new_block_starts(
            ast,
            &line,
            &mut char_offset,
            &mut effective_column_number,
            &mut additional_possible_spaces,
            &mut obd,
            &mut blank_line_depth,
            &mut HashMap::new(),
        );
        assert_eq!((ast, char_offset), (expected_ast, exp_offset));
    }

    #[test]
    fn test_cnbs_1() {
        test_cnbs(
            &mut Document(vec![BlockQuote(
                vec![Paragraph(vec!['a', 'b'], true)],
                true,
            )]),
            ">-",
            &mut Document(vec![BlockQuote(vec![Heading(vec!['a', 'b'], 2)], true)]),
            1,
        );
    }

    #[test]
    fn test_cnbs_2() {
        test_cnbs(
            &mut Document(vec![Paragraph(vec!['a', 'b'], true)]),
            "-",
            &mut Document(vec![Heading(vec!['a', 'b'], 2)]),
            0,
        );
    }

    #[test]
    fn test_cnbs_3() {
        test_cnbs(
            &mut Document(vec![Paragraph(vec!['a', 'b'], true)]),
            "=          ",
            &mut Document(vec![Heading(vec!['a', 'b'], 1)]),
            10,
        );
    }

    #[test]
    fn test_cnbs_4() {
        test_cnbs(
            &mut Document(vec![Paragraph(vec!['a', 'b'], true)]),
            "- -",
            &mut Document(vec![
                Paragraph(vec!['a', 'b'], false),
                List(
                    vec![ListItem(
                        vec![List(
                            vec![ListItem(vec![], true, 2)],
                            true,
                            UnorderedList('-'),
                            false,
                        )],
                        true,
                        2,
                    )],
                    true,
                    UnorderedList('-'),
                    false,
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
                BlockQuote(vec![Paragraph(vec!['a', 'b'], false)], false),
                BlockQuote(
                    vec![List(
                        vec![ListItem(vec![], true, 2)],
                        true,
                        UnorderedList('-'),
                        false,
                    )],
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
            &mut Document(vec![BlockQuote(vec![Heading(vec!['a', 'b'], 2)], true)]),
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
                vec![Paragraph(vec!['a', 'b'], false), ThematicBreak],
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
                    vec![ListItem(vec![ThematicBreak], true, 2)],
                    true,
                    UnorderedList('*'),
                    false,
                )],
                true,
            )]),
            ">***",
            &mut Document(vec![BlockQuote(
                vec![
                    List(
                        vec![ListItem(vec![ThematicBreak], true, 2)],
                        true,
                        UnorderedList('*'),
                        false,
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
                    vec![ListItem(vec![ThematicBreak], true, 2)],
                    true,
                    UnorderedList('*'),
                    false,
                )],
                true,
            )]),
            ">   ***",
            &mut Document(vec![BlockQuote(
                vec![List(
                    vec![ListItem(vec![ThematicBreak, ThematicBreak], true, 2)],
                    true,
                    UnorderedList('*'),
                    false,
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
                vec![ListItem(vec![ThematicBreak], true, 2)],
                true,
                UnorderedList('-'),
                false,
            )]),
            "-",
            &mut Document(vec![List(
                vec![
                    ListItem(vec![ThematicBreak], true, 2),
                    ListItem(vec![], true, 2),
                ],
                true,
                UnorderedList('-'),
                false,
            )]),
            1,
        );
    }
    #[test]
    fn test_cnbs_continue_list_item_2() {
        test_cnbs(
            &mut Document(vec![List(
                vec![ListItem(vec![ThematicBreak], true, 2)],
                true,
                OrderedList('.', 2),
                false,
            )]),
            "123. ",
            &mut Document(vec![List(
                vec![
                    ListItem(vec![ThematicBreak], true, 2),
                    ListItem(vec![], true, 5),
                ],
                true,
                OrderedList('.', 2),
                false,
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
                vec![ListItem(vec![], true, 5)],
                true,
                OrderedList('.', 123),
                false,
            )]),
            5,
        );
    }

    #[test]
    fn test_cnbs_new_list_2() {
        test_cnbs(
            &mut Document(vec![List(
                vec![ListItem(vec![ThematicBreak], true, 2)],
                true,
                OrderedList('.', 2),
                false,
            )]),
            " -  ",
            &mut Document(vec![
                List(
                    vec![ListItem(vec![ThematicBreak], true, 2)],
                    true,
                    OrderedList('.', 2),
                    false,
                ),
                List(
                    vec![ListItem(vec![], true, 3)],
                    true,
                    UnorderedList('-'),
                    false,
                ),
            ]),
            4,
        );
    }

    #[test]
    fn test_cnbs_atx_heading_1() {
        test_cnbs(
            &mut Document(vec![]),
            "#",
            &mut Document(vec![Heading(vec![], 1)]),
            1,
        );
    }

    #[test]
    fn test_cnbs_atx_heading_2() {
        test_cnbs(
            &mut Document(vec![]),
            "  #           #       ",
            &mut Document(vec![Heading(vec![], 1)]),
            22,
        );
    }

    #[test]
    fn test_cnbs_atx_heading_3() {
        test_cnbs(
            &mut Document(vec![]),
            "  #           #       hello # ",
            &mut Document(vec![Heading("#       hello".chars().collect(), 1)]),
            30,
        );
    }
}
