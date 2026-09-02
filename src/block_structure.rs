// This should be more ignorant of the implementation of the AST
//
use std::assert_matches;
use std::collections::HashMap;
use std::mem;

use crate::Block::*;
use crate::HTMLEndCondition::*;
use crate::ListType::*;
use crate::ast_types::*;
use crate::inline_str_collection::LeafContainerInline;
use crate::lrd_table::LRDTable;
use crate::parsers::*;
use crate::peekable_char_indices::*;
use crate::string_normalize::normalize_label;

pub fn create_block_structure<'a>(
    markdown: &'a str,
) -> (AbstractSyntaxTree<String, String>, LRDTable<'a, String>) {
    let mut lines_list: Vec<&str> = markdown.split('\n').collect();
    if matches!(lines_list.last(), Some(&"")) {
        lines_list.pop();
    }
    let mut lines = lines_list.iter().peekable();

    let mut lrd_table: LRDTable<'a, String> = HashMap::new();
    let mut tree: AbstractSyntaxTree<String, String> = AbstractSyntaxTree::default();

    if lines.peek().is_none() {
        return (tree, lrd_table);
    };
    // dbg!(line_0);
    let start_id = tree.get_head_id();
    let mut parsing_state = ParsingState {
        open_block_id: start_id,
        open_par_above: false,
        lcpi_op: None,
        blank_line_depth: None,
        deepest_quote: 0,
        snmoqio: None,
        dmgci: tree.get_head_id(),
        line_number: 0,
    };

    for line in lines {
        let mut line_state = LineState::new(line);
        // dbg!(&parsing_state);
        println!("==============================================");
        dbg!(parsing_state.line_number);
        check_tree_state(&tree, &mut parsing_state, &mut Some(&mut line_state));
        create_new_block_starts(&mut tree, &mut parsing_state, line_state, &mut lrd_table);
        dbg!(parsing_state.blank_line_depth);
        // dbg!(line);
    }
    // dbg!(&parsing_state);
    //TODO: figure out finding the last paragraph with no line state.
    check_tree_state(&tree, &mut parsing_state, &mut None);
    close_paragraph(&mut tree, &mut parsing_state, &mut lrd_table);
    (tree, lrd_table)
}

/************************* HELPFUL STRUCTS ********************************/
#[derive(Debug)]
struct ParsingState {
    // current_line_number: usize,
    // abstract_syntax_tree: AbstractSyntaxTree<String>,
    open_block_id: NodeId,
    open_par_above: bool,
    // lazily_continuable_par_index: Option<NodeId>,
    lcpi_op: Option<NodeId>,
    // at what "level" was a blank line seen, important
    // for telling what which list block gets loosened in nested lists.
    blank_line_depth: Option<usize>,
    // quotes have open/closed state. If we want to close a "chain" (upon encountering a blank_line) of lists,
    // we just need to close the shallowest one
    /// shallowest_non_matched_open_quote_index_op: Option<NodeId>,
    snmoqio: Option<NodeId>,
    /// deepest_matched_general_container_id: NodeId,
    dmgci: NodeId,
    deepest_quote: usize,
    line_number: usize,
}

#[derive(Debug, Clone)]
struct LineState<'a> {
    base_line: &'a str,
    char_iter: BorrowedStringPCI<'a>,
    effective_column_number: usize,
    space_from_last_structure: usize,
}

impl<'a> Iterator for LineState<'a> {
    type Item = char;

    fn next(&mut self) -> Option<char> {
        self.next_and_index().map(|(_, c)| c)
    }
}

impl<'a> PeekableCharIndices for LineState<'a> {
    type Offset = usize;
    fn next_and_index(&mut self) -> Option<(usize, char)> {
        let out = self.char_iter.next_and_index()?;
        if out.1 == '\t' {
            self.space_from_last_structure += 4 - (self.effective_column_number % 4);
            self.effective_column_number += 4 - (self.effective_column_number % 4);
        } else if out.1 == ' ' {
            self.space_from_last_structure += 1;
            self.effective_column_number += 1;
        } else {
            self.space_from_last_structure = 0;
            self.effective_column_number += 1;
        }
        // dbg!(&self);
        Some(out)
    }
    fn peek_and_index(&mut self) -> Option<(usize, char)> {
        self.char_iter.peek_and_index()
    }

    fn offset(&self) -> usize {
        self.char_iter.offset()
    }
}

impl<'a> LineState<'a> {
    fn new(line_in: &'a str) -> Self {
        LineState {
            base_line: line_in,
            char_iter: line_in.into(),
            effective_column_number: 0,
            space_from_last_structure: 0,
        }
    }
    fn consume_indent(&mut self) {
        self.consume_while(|c| " \t".contains(c));
    }

    fn consume_one_space(&mut self) -> bool {
        self.next_if(|c| " \t".contains(c)).is_some()
    }

    fn consume_indent_until_sfls_ge(&mut self, n: usize) {
        // dbg!("called consume_indent");
        while (self.space_from_last_structure) < n && self.next_if(|c| " \t".contains(c)).is_some()
        {
        }
    }

    fn is_blank_line(&self) -> bool {
        let mut self_clone = self.clone();
        self_clone.consume_indent();
        self_clone.is_empty()
    }

    fn get_remaining_str(&self) -> &'a str {
        &self.base_line[self.offset()..]
    }
}

#[cfg(test)]
mod test_ls {
    use super::*;

    #[test]
    fn test_ls_1() {
        let mut my_iter = LineState::new(" \ta");

        assert_eq!(my_iter.offset(), 0);
        assert_eq!(my_iter.peek(), Some(' '));
        assert_eq!(my_iter.offset(), 0);
        my_iter.consume_indent_until_sfls_ge(2);
        assert_eq!(my_iter.offset(), 2);
        assert_eq!(my_iter.effective_column_number, 4);
        assert_eq!(my_iter.space_from_last_structure, 4);
        assert_eq!(my_iter.peek(), Some('a'));
        assert_eq!(my_iter.next(), Some('a'));
        assert!(my_iter.is_blank_line());
        assert!(my_iter.is_empty());
    }
}

// this should:
// - check continuation_conditions for the tree above.
// - check for lazily continuable paragraphs.
fn check_tree_state<C: LeafContainerInline>(
    ast: &AbstractSyntaxTree<C, C>,
    parsing_state: &mut ParsingState,
    line_state_op: &mut Option<&mut LineState>,
) {
    // reset for a fresh check
    '_reset_parsing_state: {
        parsing_state.line_number += 1;
        parsing_state.open_par_above = false;
        parsing_state.lcpi_op = None;
        parsing_state.deepest_quote = 0;
        parsing_state.snmoqio = None;
        parsing_state.open_block_id = ast.get_head_id();
        parsing_state.dmgci = ast.get_head_id();
    }
    let mut current_block_id = ast.get_head_id();
    let mut ccs_still_satisfied = true;
    // println!("called ccc!");
    while let Some(next_block_id) = ast.get_last_child_id(current_block_id) {
        current_block_id = next_block_id;
        // dbg!(ast.get_block_ref(current_block_id));
        // as deref mut, very demur
        if let Some(line_state) = line_state_op.as_deref_mut()
            && ccs_still_satisfied
        {
            match ast.get_block_ref(current_block_id) {
                Document => unreachable!(),
                BlockQuote(is_open) => {
                    if !*is_open {
                        ccs_still_satisfied = false
                    } else {
                        line_state.consume_indent_until_sfls_ge(4);
                        //block_quote_encountered is reliant on a single char,
                        //so line space preserving isn't really needed
                        if line_state.space_from_last_structure <= 3
                            && block_quote_encountered(line_state)
                        {
                            parsing_state.open_block_id = current_block_id;
                            parsing_state.deepest_quote = ast.get_block_depth(current_block_id);
                            parsing_state.dmgci = current_block_id;
                        } else {
                            ccs_still_satisfied = false;
                            if parsing_state.snmoqio.is_none() {
                                parsing_state.snmoqio = Some(current_block_id);
                            }
                        }
                    }
                }
                List(..) => {
                    parsing_state.open_block_id = current_block_id;
                }
                ListItem(continuable, indent_amount) => {
                    if !*continuable {
                        ccs_still_satisfied = false;
                    } else {
                        let old_line_state = line_state.clone();
                        line_state.consume_indent_until_sfls_ge(*indent_amount);
                        if line_state.space_from_last_structure >= *indent_amount
                            || line_state.is_empty()
                        {
                            line_state.space_from_last_structure -=
                                std::cmp::min(*indent_amount, line_state.space_from_last_structure);
                            parsing_state.open_block_id = current_block_id;
                            parsing_state.dmgci = current_block_id;
                        } else {
                            ccs_still_satisfied = false;
                            *line_state = old_line_state;
                        }
                    }
                }
                Heading(_, _) => ccs_still_satisfied = false,
                Paragraph(_, is_open) => {
                    ccs_still_satisfied = false;
                    if *is_open {
                        parsing_state.open_block_id = current_block_id;
                        parsing_state.open_par_above = true;
                    }
                }
                ThematicBreak => ccs_still_satisfied = false,
                IndentedCodeBlock(_, _) => {
                    ccs_still_satisfied = false;
                    line_state.consume_indent_until_sfls_ge(4);
                    if dbg!(line_state.space_from_last_structure >= 4 || line_state.is_empty()) {
                        parsing_state.open_block_id = current_block_id;
                    }
                }
                FencedCodeBlock(_, is_open, _, _, _, _) | HTMLBlock(_, is_open, _) => {
                    ccs_still_satisfied = false;
                    if *is_open {
                        parsing_state.open_block_id = current_block_id;
                    }
                }
            }
        } else {
            // looking down the tree past when ccs are still satisfied.
            if parsing_state.snmoqio.is_none()
                && matches!(ast.get_block_ref(current_block_id), BlockQuote(true))
            {
                parsing_state.snmoqio = Some(current_block_id);
            }
        }
    }
    parsing_state.lcpi_op = if matches!(ast.get_block_ref(current_block_id), Paragraph(_, true)) {
        Some(current_block_id)
    } else {
        None
    };
}

fn thematic_break_encountered(char_iter: &mut LineState) -> bool {
    let Some(br_sym) = char_iter.next_if(|c| "-*_".contains(c)) else {
        return false;
    };
    let mut c_count = 1;
    while let Some(c) = char_iter.next_if(|c| " \t".contains(c) || c == br_sym) {
        if c == br_sym {
            c_count += 1
        }
    }
    if char_iter.is_empty() && c_count >= 3 {
        return true;
    }
    false
}

fn list_item_encountered<C: LeafContainerInline>(
    line_state: &mut LineState,
) -> Option<(ListType, Block<C, C>)> {
    let space_before = line_state.space_from_last_structure;
    let c_0 = line_state.next()?;
    if c_0.is_numeric() {
        let mut digit_count = 1;
        let mut count = c_0.to_digit(10).unwrap();
        while let Some(c) = line_state.next_if(|c| c.is_numeric())
            && digit_count <= 9
        {
            count *= 10;
            count += c.to_digit(10).unwrap();
            digit_count += 1;
        }
        if let Some(c) = line_state.next_if(|c| ".)".contains(c))
            && digit_count <= 9
        {
            let post_li_iter = line_state.clone();

            line_state.consume_indent_until_sfls_ge(5);

            if line_state.is_empty() {
                //dont need to update parsing state related to this in this situation. because
                //there is no more line.
                return Some((
                    OrderedList(c, count),
                    ListItem(true, space_before + digit_count + 2),
                ));
            }

            if 1 <= line_state.space_from_last_structure
                && line_state.space_from_last_structure <= 4
            {
                let space_after = line_state.space_from_last_structure;
                line_state.space_from_last_structure = 0;
                return Some((
                    OrderedList(c, count),
                    ListItem(true, space_before + digit_count + 1 + space_after),
                ));
            } else if line_state.space_from_last_structure > 4 {
                *line_state = post_li_iter;
                assert!(line_state.consume_one_space());
                line_state.space_from_last_structure -= 1;
                return Some((
                    OrderedList(c, count),
                    ListItem(true, space_before + digit_count + 2),
                ));
            }
        }
        return None;
    }

    if "-*+".contains(c_0) {
        let post_li_iter = line_state.clone();

        line_state.consume_indent_until_sfls_ge(5);
        if line_state.is_empty() {
            //dont need to update parsing state related to this in this situation. because
            //there is no more line.
            return Some((UnorderedList(c_0), ListItem(true, space_before + 2)));
        }

        if 1 <= line_state.space_from_last_structure && line_state.space_from_last_structure <= 4 {
            let space_after = line_state.space_from_last_structure;
            line_state.space_from_last_structure = 0;
            return Some((
                UnorderedList(c_0),
                ListItem(true, space_before + 1 + space_after),
            ));
        } else if line_state.space_from_last_structure > 4 {
            *line_state = post_li_iter;
            assert!(line_state.consume_one_space());
            line_state.space_from_last_structure -= 1;
            return Some((UnorderedList(c_0), ListItem(true, space_before + 2)));
        }
    }
    None
}

fn block_quote_encountered(line_state: &mut LineState) -> bool {
    if line_state.next_if_char_eq('>').is_none() {
        return false;
    }
    if line_state.consume_one_space() {
        line_state.space_from_last_structure -= 1;
    }
    dbg!(&line_state);
    true
}

fn fenced_code_block_encountered<C: LeafContainerInline>(
    line_state: &mut LineState,
) -> Option<Block<C, C>> {
    let pre_space_count = line_state.space_from_last_structure;
    let start_offset = line_state.offset();
    let t = line_state.next_if(|c| "`~".contains(c))?;

    line_state.consume_while_char_eq(t);
    let tick_count = line_state.offset() - start_offset;
    if tick_count < 3 {
        return None;
    }
    line_state.consume_indent();
    let inf_str_start = line_state.offset();
    let mut info_str = String::new();
    while let Some(c) = line_state.next_if(|c| !" \t".contains(c)) {
        info_str.push(c);
        if t == '`' && c == '`' {
            return None;
        }
    }
    let inf_str_end = line_state.offset();
    if t == '`' {
        while let Some(c) = line_state.next() {
            if c == '`' {
                return None;
            }
        }
    }
    Some(FencedCodeBlock(
        C::default(),
        true,
        t,
        C::from_extra_space_and_line(0, &line_state.base_line[inf_str_start..inf_str_end]),
        pre_space_count,
        tick_count,
    ))
}

fn atx_heading_encountered<C: LeafContainerInline + Default>(
    line_state: &mut LineState,
) -> Option<Block<C, C>> {
    let pound_count = line_state.consume_while_char_eq('#');
    if !(1..=6).contains(&pound_count) {
        return None;
    }

    if line_state.is_empty() {
        return Some(Heading(C::default(), pound_count));
    }

    if !line_state.consume_one_space() {
        return None;
    }

    line_state.consume_indent();

    let mut trailing_line_state = line_state.clone();
    // let offset_after_space = line_state.offset();

    let mut str_out = String::new();
    let start_offset = line_state.offset();
    let mut end_offset = line_state.offset();
    let mut pre_space_seen = true;
    // the offset at which the pounds start.
    let mut post_space_pounds = None;
    let mut post_pounds_space = false;

    while let Some((c_i, c)) = line_state.next_and_index() {
        if " \t".contains(c) && post_space_pounds.is_none() {
            pre_space_seen = true
        } else if c == '#' && pre_space_seen {
            if post_pounds_space {
                //post_pounds_space implies that post_space_pounds is some.
                end_offset = post_space_pounds.unwrap();
                post_pounds_space = false;
                post_space_pounds = Some(c_i);
            } else if post_space_pounds.is_none() {
                post_space_pounds = Some(c_i);
            }
        } else if " \t".contains(c) && post_space_pounds.is_some() {
            post_pounds_space = true;
        } else {
            pre_space_seen = false;
            post_space_pounds = None;
            post_pounds_space = false;
            end_offset = line_state.offset();
            while trailing_line_state.offset() <= c_i {
                str_out.push(trailing_line_state.next().unwrap());
            }
        }
    }
    let mut il_out = C::default();
    il_out.add_extra_space_and_line(0, &line_state.base_line[start_offset..end_offset]);

    Some(Heading(il_out, pound_count))
}

fn html_start_encountered<C: LeafContainerInline + Default>(
    line_state: &mut LineState,
    open_par_exists: bool,
    space_from_last_structure: usize,
) -> Option<Block<C, C>> {
    let simple_starts_with = |target_str: &'static str| -> Box<dyn Fn(&mut LineState) -> bool> {
        Box::new(move |char_iter: &mut LineState| -> bool {
            let mut chars_seen = 0;
            // need to prevent zip from calling the last (unsuccessful) next.
            for (c_in, c_target) in char_iter
                .by_ref()
                .take(target_str.len())
                .zip(target_str.chars())
            {
                if c_in.to_ascii_lowercase() != c_target {
                    return false;
                }
                chars_seen += 1;
            }
            // dbg!(target_str);
            chars_seen == target_str.len()
        })
    };
    let starts_with_match_case = |target_str: &'static str| -> Box<dyn Fn(&mut LineState) -> bool> {
        Box::new(move |char_iter: &mut LineState| -> bool {
            let mut chars_seen = 0;
            // need to prevent zip from calling the last (unsuccessful) next.
            for (c_in, c_target) in char_iter
                .by_ref()
                .take(target_str.len())
                .zip(target_str.chars())
            {
                if c_in != c_target {
                    return false;
                }
                chars_seen += 1;
            }
            chars_seen == target_str.len()
        })
    };
    let special_tag: Box<dyn Fn(&mut LineState) -> bool> =
        Box::new(|char_iter: &mut LineState| -> bool {
            for s in ["<pre", "<script", "<style", "<textarea"] {
                let mut new_iter = char_iter.clone();
                if simple_starts_with(s)(&mut new_iter)
                    && (new_iter.is_empty() || new_iter.next_if(|c| " \t>".contains(c)).is_some())
                {
                    return true;
                }
            }
            false
        });

    let exclamation: Box<dyn Fn(&mut LineState) -> bool> =
        Box::new(|char_iter: &mut LineState| -> bool {
            return simple_starts_with("<!")(char_iter)
                && char_iter.next_if(|c| c.is_ascii_alphabetic()).is_some();
        });
    for (start_con, end_strs) in [
        (
            special_tag,
            vec!["</pre>", "</script>", "</style>", "</textarea>"],
        ),
        (simple_starts_with("<!--"), vec!["-->"]),
        (simple_starts_with("<?"), vec!["?>"]),
        (exclamation, vec![">"]),
        (starts_with_match_case("<![CDATA["), vec!["]]>"]),
    ] {
        let mut line_state_clone = line_state.clone();
        if start_con(&mut line_state_clone) {
            let mut string_out: String = String::new();
            for _ in 0..space_from_last_structure {
                string_out.push(' ');
            }
            for c in &mut *line_state {
                string_out.push(c);
            }
            let mut il_out = C::default();
            il_out.add_extra_space_and_line(
                space_from_last_structure,
                line_state.get_remaining_str(),
            );

            string_out.push_str(&(line_state.collect::<String>()));
            let is_open = !end_strs.iter().any(|s| string_out.contains(s));
            return Some(HTMLBlock(
                il_out,
                is_open,
                ContainsStrings(end_strs.iter().map(|s| s.chars().collect()).collect()),
            ));
        }
    }

    //todo: use tries?
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

    let mut eat_lt_iter = line_state.clone();
    (eat_lt_iter.next_if_char_eq('<'))?;
    // dbg!("=========");
    // dbg!(&eat_lt_iter);
    // dbg!("=========");
    let is_closing = eat_lt_iter.next_if_char_eq('/').is_some();

    for rt in reserved_tags {
        let mut line_state_clone = eat_lt_iter.clone();
        if simple_starts_with(rt)(&mut line_state_clone)
            && (line_state_clone.is_empty()
                || line_state_clone.next_if(|c| " \t>".contains(c)).is_some()
                || (line_state_clone.next_if_char_eq('/').is_some()
                    && line_state_clone.next_if_char_eq('>').is_some()))
        {
            let mut il_out = C::default();
            il_out.add_extra_space_and_line(
                line_state.space_from_last_structure,
                line_state.get_remaining_str(),
            );
            return Some(HTMLBlock(il_out, true, BlankLine));
        }
    }

    // non reserved (type 7) cant interrupt paragraph
    if open_par_exists {
        return None;
    }

    let mut tag_finder = eat_lt_iter.char_iter.clone();
    if is_closing {
        parse_closing_tag(&mut tag_finder)?;
    } else if tag_finder.next_if(|c| c.is_ascii_alphabetic()).is_some() {
        parse_opening_tag(&mut tag_finder)?;
    } else {
        return None;
    }

    tag_finder.consume_while(|c| " \t".contains(c));
    if tag_finder.is_empty() {
        let mut il_out = C::default();
        il_out.add_extra_space_and_line(
            line_state.space_from_last_structure,
            line_state.get_remaining_str(),
        );
        return Some(HTMLBlock(il_out, true, BlankLine));
    } else {
        None
    }
}

fn close_paragraph<'a, C: LeafContainerInline>(
    ast: &'a mut AbstractSyntaxTree<C, C>,
    parsing_state: &mut ParsingState,
    lrd_table: &mut LRDTable<'a, C>,
) {
    let Some(open_par_id) = parsing_state.lcpi_op else {
        return;
    };

    let mut inline: C;
    let Paragraph(inline_in_par, true) = ast.take_block(open_par_id) else {
        panic!()
    };
    inline = inline_in_par;

    'collect_lrds: loop {
        // borrow checker did not like the possibility that I could reassign inline while
        // current_iteration_iter still had a borrow
        let (link_lab_range, link_dest_range, link_title_range_op, split_at) = {
            let mut current_iteration_iter = inline.as_pci(); // let mut link_lab_iter_start = current_iteration_iter.clone();

            let Some(link_lab_range) = parse_link_label(&mut current_iteration_iter) else {
                break 'collect_lrds;
            };
            // let link_lab_end_char_index = current_iteration_iter.offset();

            // dbg!(&current_iteration_iter);
            if current_iteration_iter.next_if_char_eq(':').is_none() {
                break 'collect_lrds;
            }

            //optional spaces or tabs
            current_iteration_iter.consume_while(|c| " \t\n".contains(c));

            let Some(link_dest_range) = parse_link_destination(&mut current_iteration_iter) else {
                break 'collect_lrds;
            };

            let offset_before_spaces = current_iteration_iter.offset();
            current_iteration_iter.consume_while(|c| " \t".contains(c));

            let is_valid_lrd = current_iteration_iter.next_if_char_eq('\n').is_some()
                || current_iteration_iter.is_empty();

            // the way we created paragraphs means that pre_whitespace in line is already stripped
            // so !c.is_whitespace() is true for all c that follow an \n
            // let mut link_title_iter_start = current_iteration_iter.clone();
            let mut find_link_title_iter = current_iteration_iter.clone();
            let link_title_range_op = if current_iteration_iter.offset() > offset_before_spaces
                && let Some(link_title_range) = dbg!(parse_link_title(&mut find_link_title_iter))
                && {
                    find_link_title_iter.consume_while(|c| " \t".contains(c));
                    match find_link_title_iter.peek() {
                        Some('\n') | None => {
                            find_link_title_iter.next();
                            true
                        }
                        _ => false,
                    }
                } {
                current_iteration_iter = find_link_title_iter;
                Some(link_title_range)
            } else {
                None
            };
            if !is_valid_lrd {
                break 'collect_lrds;
            }
            let split_at = current_iteration_iter.offset();
            (
                link_lab_range,
                link_dest_range,
                link_title_range_op,
                split_at,
            )
        };
        if let Some(link_title_range) = link_title_range_op {
            let (inline_chunk_with_values, new_inline) = inline.split(split_at);
            let link_lab_normal =
                normalize_label(&inline_chunk_with_values.chars_in_range(link_lab_range));
            lrd_table.entry(link_lab_normal).or_insert((
                inline_chunk_with_values,
                link_dest_range,
                if link_title_range.start != link_title_range.end {
                    Some(link_title_range)
                } else {
                    None
                },
            ));
            inline = new_inline;
        } else {
            let (inline_chunk_with_values, new_inline) = inline.split(split_at);
            let link_lab_normal =
                normalize_label(&inline_chunk_with_values.chars_in_range(link_lab_range));
            lrd_table.entry(link_lab_normal).or_insert((
                inline_chunk_with_values,
                link_dest_range,
                None,
            ));
            inline = new_inline;
        }
    }
    ast.replace_block(open_par_id, Paragraph(inline, false));
    parsing_state.lcpi_op = None;
    parsing_state.open_par_above = false;
}

// #[cfg(test)]
// mod cp_tests {
//     use super::*;
//     use pretty_assertions::assert_eq;
//
//     fn test_cp(ast: Block, expected_ast: Block, exp_table: LRDTable) {
//         let mut parsing_state = ParsingState {
//             abstract_syntax_tree: ast,
//             lrd_table: HashMap::new(),
//             line_state: LineState::new(""),
//             open_block_depth: 1,
//             open_par_above: true,
//             open_par_exists: true,
//             blank_line_depth: None,
//             line_number: 0,
//         };
//         close_paragraph(&mut parsing_state);
//         assert_eq!(
//             (expected_ast, exp_table, false, false),
//             (
//                 parsing_state.abstract_syntax_tree,
//                 parsing_state.lrd_table,
//                 parsing_state.open_par_above,
//                 parsing_state.open_par_exists
//             ),
//         )
//     }
//
//     #[test]
//     fn test_cp_no_def() {
//         test_cp(
//             Document(vec![Paragraph(
//                 Inline::new("hello".chars().collect()),
//                 true,
//             )]),
//             Document(vec![Paragraph(
//                 Inline::new("hello".chars().collect()),
//                 false,
//             )]),
//             HashMap::new(),
//         )
//     }
//
//     #[test]
//     fn test_cp_no_title() {
//         test_cp(
//             Document(vec![Paragraph(
//                 Inline::new("[hello]:link".chars().collect()),
//                 true,
//             )]),
//             Document(vec![Paragraph(Inline::new("".chars().collect()), false)]),
//             HashMap::from([("hello".chars().collect(), ("link".chars().collect(), None))]),
//         )
//     }
//
//     #[test]
//     fn test_cp_title() {
//         test_cp(
//             Document(vec![Paragraph(
//                 Inline::new("[hello]:link (your_mom)".chars().collect()),
//                 true,
//             )]),
//             Document(vec![Paragraph(Inline::new("".chars().collect()), false)]),
//             HashMap::from([(
//                 "hello".chars().collect(),
//                 ("link".chars().collect(), Some("your_mom".chars().collect())),
//             )]),
//         )
//     }
//
//     #[test]
//     fn test_cp_multiline() {
//         test_cp(
//             Document(vec![Paragraph(
//                 Inline::new("[\nhel\nlo\n]:\nlink \n(your_mom)".chars().collect()),
//                 true,
//             )]),
//             Document(vec![Paragraph(Inline::new("".chars().collect()), false)]),
//             HashMap::from([(
//                 "hel lo".chars().collect(),
//                 ("link".chars().collect(), Some("your_mom".chars().collect())),
//             )]),
//         )
//     }
//     #[test]
//     fn test_cp_title_fail() {
//         test_cp(
//             Document(vec![Paragraph(
//                 Inline::new("[\nhel\nlo\n]:\nlink    \n(your_mom".chars().collect()),
//                 true,
//             )]),
//             Document(vec![Paragraph(
//                 Inline::new("(your_mom".chars().collect()),
//                 false,
//             )]),
//             HashMap::from([("hel lo".chars().collect(), ("link".chars().collect(), None))]),
//         )
//     }
//
//     #[test]
//     fn test_cp_link_fail_1() {
//         test_cp(
//             Document(vec![Paragraph(
//                 Inline::new("[\nhel\nlo\n]:\n)link    \n(your_mom".chars().collect()),
//                 true,
//             )]),
//             Document(vec![Paragraph(
//                 Inline::new("[\nhel\nlo\n]:\n)link    \n(your_mom".chars().collect()),
//                 false,
//             )]),
//             HashMap::from([]),
//         )
//     }
//     #[test]
//     fn test_cp_link_fail_2() {
//         test_cp(
//             Document(vec![Paragraph(
//                 Inline::new("[\nhel\nlo\n]:\n(link(())    \n(your_mom".chars().collect()),
//                 true,
//             )]),
//             Document(vec![Paragraph(
//                 Inline::new("[\nhel\nlo\n]:\n(link(())    \n(your_mom".chars().collect()),
//                 false,
//             )]),
//             HashMap::from([]),
//         )
//     }
//     #[test]
//     fn test_cp_more_paragraph() {
//         test_cp(
//             Document(vec![Paragraph(
//                 Inline::new(
//                     "[\nhel\nlo\n]:\nlink    \n'your_mom'\nand theres more"
//                         .chars()
//                         .collect(),
//                 ),
//                 true,
//             )]),
//             Document(vec![Paragraph(
//                 Inline::new("and theres more".chars().collect()),
//                 false,
//             )]),
//             HashMap::from([(
//                 "hel lo".chars().collect(),
//                 ("link".chars().collect(), Some("your_mom".chars().collect())),
//             )]),
//         )
//     }
//     #[test]
//     fn test_cp_2_def() {
//         test_cp(
//             Document(vec![Paragraph(
//                 Inline::new(
//                     "[\nhel\nlo\n]:\nlink    \n'your_mom'\n[def2]:link2 \"desc_2\""
//                         .chars()
//                         .collect(),
//                 ),
//                 true,
//             )]),
//             Document(vec![Paragraph(Inline::new("".chars().collect()), false)]),
//             HashMap::from([
//                 (
//                     "hel lo".chars().collect(),
//                     ("link".chars().collect(), Some("your_mom".chars().collect())),
//                 ),
//                 (
//                     "def2".chars().collect(),
//                     ("link2".chars().collect(), Some("desc_2".chars().collect())),
//                 ),
//             ]),
//         )
//     }
//     #[test]
//     fn test_cp_2_def_override() {
//         test_cp(
//             Document(vec![Paragraph(
//                 Inline::new(
//                     "[\nhel\nlo\n]:\nlink    \n'your_mom'\n[hel    lo   ]:link2 \"desc_2\""
//                         .chars()
//                         .collect(),
//                 ),
//                 true,
//             )]),
//             Document(vec![Paragraph(Inline::new("".chars().collect()), false)]),
//             HashMap::from([(
//                 "hel lo".chars().collect(),
//                 ("link".chars().collect(), Some("your_mom".chars().collect())),
//             )]),
//         )
//     }
//     #[test]
//     fn test_cp_fails() {
//         test_cp(
//             Document(vec![Paragraph(
//                 Inline::new(
//                     "[\nhel\nlo\n]:\n<link    \n'your_mom'\n[hel    lo   ]:link2 \"desc_2\""
//                         .chars()
//                         .collect(),
//                 ),
//                 true,
//             )]),
//             Document(vec![Paragraph(
//                 Inline::new(
//                     "[\nhel\nlo\n]:\n<link    \n'your_mom'\n[hel    lo   ]:link2 \"desc_2\""
//                         .chars()
//                         .collect(),
//                 ),
//                 false,
//             )]),
//             HashMap::from([]),
//         )
//     }
//     #[test]
//     fn test_cp_2_escapes() {
//         test_cp(
//             Document(vec![Paragraph(
//                 Inline::new(
//                     "[\nh\\el\nlo\n]:\nlink    \n'your_mom'\n[hel    lo   ]:link2 \"desc_2\""
//                         .chars()
//                         .collect(),
//                 ),
//                 true,
//             )]),
//             Document(vec![Paragraph(Inline::new("".chars().collect()), false)]),
//             HashMap::from([
//                 (
//                     "h\\el lo".chars().collect(),
//                     ("link".chars().collect(), Some("your_mom".chars().collect())),
//                 ),
//                 (
//                     "hel lo".chars().collect(),
//                     ("link2".chars().collect(), Some("desc_2".chars().collect())),
//                 ),
//             ]),
//         )
//     }
// }

fn create_new_block_starts<'a, C: LeafContainerInline>(
    ast: &mut AbstractSyntaxTree<C, C>,
    parsing_state: &mut ParsingState,
    mut line_state: LineState,
    lrd_table: &mut LRDTable<'a, C>,
) {
    'blank_line: {
        if line_state.is_blank_line() {
            let has_no_children = ast.has_no_children(parsing_state.open_block_id);
            match ast.get_block(parsing_state.open_block_id) {
                IndentedCodeBlock(_, unrealized_blanks) => {
                    line_state.consume_indent_until_sfls_ge(4);
                    let sfls = line_state.space_from_last_structure;
                    unrealized_blanks.add_extra_space_and_line(
                        sfls - std::cmp::min(4, sfls),
                        line_state.get_remaining_str(),
                    );
                }
                FencedCodeBlock(..) => break 'blank_line,
                HTMLBlock(_, true, ContainsStrings(_)) => break 'blank_line,
                HTMLBlock(_, is_open @ true, BlankLine) => *is_open = false,
                ListItem(continuable, _) if has_no_children => *continuable = false,
                _ => (),
            }
            close_paragraph(ast, parsing_state, lrd_table);
            parsing_state.blank_line_depth = Some(parsing_state.deepest_quote);
            // make block quotes closed
            if let Some(shallowest_block_quote_index) = parsing_state.snmoqio
                && let BlockQuote(b @ true) = ast.get_block(shallowest_block_quote_index)
            {
                *b = false;
            }
            return;
        }
    }

    match ast.get_block(parsing_state.open_block_id) {
        IndentedCodeBlock(chars, spaces) => {
            // None Blank Line!
            let mut new_spaces = C::default();
            mem::swap(spaces, &mut new_spaces);
            let sfls = line_state.space_from_last_structure;
            chars.add_extra_space_and_line(
                sfls - std::cmp::min(4, sfls),
                line_state.get_remaining_str(),
            );
            return;
        }
        FencedCodeBlock(chars, is_open @ true, marker, _, indent_count, marker_count) => {
            let mut try_to_close_chars = line_state.clone();
            try_to_close_chars.consume_indent_until_sfls_ge(4);
            if try_to_close_chars.space_from_last_structure <= 3
                && let Some(FencedCodeBlock(_, _, t, infstr, _, tc)) =
                    fenced_code_block_encountered::<C>(&mut try_to_close_chars)
                && t == *marker
                && infstr.is_empty()
                && tc >= *marker_count
            {
                *is_open = false;
                return;
            }
            // println!("line: {:?} can be added!", &line[*char_offset..]);
            let sfls = line_state.space_from_last_structure;
            chars.add_extra_space_and_line(
                sfls - std::cmp::min(*indent_count, sfls),
                line_state.get_remaining_str(),
            );

            return;
        }
        HTMLBlock(chars, is_open @ true, end_condition) => {
            dbg!("HTMLBlock parent matched!");
            let iter_clone = line_state.clone();
            let current_line_as_str: String = iter_clone.collect();
            if let ContainsStrings(strs) = end_condition {
                for s in strs {
                    if current_line_as_str.contains(s.as_str()) {
                        *is_open = false;
                    }
                }
            }
            chars.add_extra_space_and_line(0, line_state.get_remaining_str());
            return;
        }
        _ => (),
    }

    // we do not want to consume more than 4 for the case that we are creating an indented code
    // block in which case we have to copy in those spaces
    // let mut pre_space_line = parsing_state.line_state.clone();
    line_state.consume_indent_until_sfls_ge(4);
    // obd is unmodified at this point, since we know we are not in a blank line at  this point,
    // that means that *something* will eventually get added to list item
    // dbg!(parsing_state.open_block_id);
    // dbg!(&ast);
    // dbg!(ast.get_block_ref(parsing_state.open_block_id));
    let mut list_being_added_to_id_op: Option<NodeId> =
        if matches!(*ast.get_block_ref(parsing_state.dmgci), ListItem(..)) {
            dbg!(ast.get_parent_id(parsing_state.dmgci))
        } else {
            None
        };
    dbg!(&list_being_added_to_id_op);

    let detighten_list = |list_being_added_to_id_op: Option<NodeId>,
                          parsing_state: &mut ParsingState,
                          ast: &mut AbstractSyntaxTree<C>| {
        if let Some(list_block_id) = list_being_added_to_id_op {
            let block_depth = ast.get_block_depth(list_block_id);

            if let List(is_tight, ..) = ast.get_block(list_block_id) {
                *is_tight &= parsing_state
                    .blank_line_depth
                    .is_none_or(|d| block_depth < d);
            }
        }
        parsing_state.blank_line_depth = None;
    };

    if line_state.space_from_last_structure <= 3 {
        // First check for SetextHeading
        // c_0 is first non space character after offset
        // dbg!(&parsing_state.line_state);
        let c_0 = line_state.peek().unwrap();
        'setext_check: {
            if parsing_state.open_par_above && "-=".contains(c_0) {
                let mut try_to_setext_iter = line_state.clone();
                // the line is of the form "[pre-matched-structure][1-3 space](-|=)*' '*"
                try_to_setext_iter.consume_while_char_eq(c_0);

                // let dash_count = try_to_setext_iter.offset() - pre_dash_offset;

                if try_to_setext_iter.is_blank_line() {
                    close_paragraph(ast, parsing_state, lrd_table);
                    let mut setext_inline = C::default();
                    match ast.get_block(parsing_state.open_block_id) {
                        Paragraph(il, _) => {
                            if il.is_empty() {
                                break 'setext_check;
                            }
                            il.remove_final_spaces();
                            mem::swap(&mut setext_inline, il);
                        }
                        _ => panic!(),
                    }
                    ast.replace_block(
                        parsing_state.open_block_id,
                        Heading(setext_inline, if c_0 == '=' { 1 } else { 2 }),
                    );
                    return;
                }
            }
        }

        // next check for thematic break (before we add to list for priority reasons)
        // because it is established non empty, bounds check is not needed
        let mut thematic_break_line = line_state.clone();
        if thematic_break_encountered(&mut thematic_break_line) {
            close_paragraph(ast, parsing_state, lrd_table);
            detighten_list(list_being_added_to_id_op, parsing_state, ast);
            ast.add_new_node(parsing_state.dmgci, ThematicBreak);
            return;
        }

        //now check if we can add a new list item to an existing list
        let mut list_item_iter = line_state.clone();
        if let List(_, existing_lt, ..) = (ast.get_block(parsing_state.open_block_id))
            && let Some((new_lt, li)) = (list_item_encountered(&mut list_item_iter))
            && existing_lt.same_list_eq(&new_lt)
        {
            list_being_added_to_id_op = Some(parsing_state.open_block_id);
            detighten_list(list_being_added_to_id_op, parsing_state, ast);
            close_paragraph(ast, parsing_state, lrd_table);
            parsing_state.open_block_id = ast.add_new_node(parsing_state.open_block_id, li);
            parsing_state.dmgci = parsing_state.open_block_id;
            line_state = list_item_iter;
        }

        // keep looking for new block starts
        'look_for_new_block_starts: while line_state.space_from_last_structure <= 3 {
            let mut them_break_iter = line_state.clone();
            if thematic_break_encountered(&mut them_break_iter) {
                detighten_list(list_being_added_to_id_op, parsing_state, ast);
                close_paragraph(ast, parsing_state, lrd_table);
                ast.add_new_node(parsing_state.dmgci, ThematicBreak);
                return;
            }

            let mut list_item_iter = line_state.clone();
            if let Some((lt, b)) = list_item_encountered(&mut list_item_iter) {
                if parsing_state.open_par_above {
                    match lt {
                        OrderedList(_, 1) | UnorderedList(_) => {
                            if list_item_iter.is_blank_line() {
                                break;
                            }
                        }
                        OrderedList(_, _) => break,
                    }
                } //only ordered lists starting with 1 can interrupt paragraphs. and must be non
                //empty afterwards
                close_paragraph(ast, parsing_state, lrd_table);
                let list_node_id = ast.add_new_node(parsing_state.dmgci, List(true, lt));
                parsing_state.open_block_id = ast.add_new_node(list_node_id, b);
                parsing_state.dmgci = parsing_state.open_block_id;
                line_state = list_item_iter;
                line_state.consume_indent_until_sfls_ge(4);
                continue 'look_for_new_block_starts;
            }

            let mut block_quote_iter = line_state.clone();
            if block_quote_encountered(&mut block_quote_iter) {
                detighten_list(list_being_added_to_id_op, parsing_state, ast);
                close_paragraph(ast, parsing_state, lrd_table);
                parsing_state.open_block_id =
                    ast.add_new_node(parsing_state.dmgci, BlockQuote(true));
                parsing_state.dmgci = parsing_state.open_block_id;
                line_state = block_quote_iter;
                line_state.consume_indent_until_sfls_ge(4);
                // dbg!(line_state.space_from_last_structure);
                continue 'look_for_new_block_starts;
            }

            let mut fcb_iter = line_state.clone();
            if let Some(fcb) = fenced_code_block_encountered(&mut fcb_iter) {
                detighten_list(list_being_added_to_id_op, parsing_state, ast);
                close_paragraph(ast, parsing_state, lrd_table);
                ast.add_new_node(parsing_state.dmgci, fcb);
                return;
            }

            let mut atx_iter = line_state.clone();
            // dbg!("TRYING_TO_ATX_ITER");
            if let Some(atxh) = atx_heading_encountered(&mut atx_iter) {
                detighten_list(list_being_added_to_id_op, parsing_state, ast);
                close_paragraph(ast, parsing_state, lrd_table);
                ast.add_new_node(parsing_state.dmgci, atxh);
                return;
            }

            let mut html_iter = line_state.clone();
            if let Some(html) = html_start_encountered(
                &mut html_iter,
                parsing_state.open_par_above,
                line_state.space_from_last_structure,
            ) {
                detighten_list(list_being_added_to_id_op, parsing_state, ast);
                close_paragraph(ast, parsing_state, lrd_table);
                ast.add_new_node(parsing_state.dmgci, html);
                return;
            }
            break;
        }
    }

    // at this point we can let potentially
    // non-tight lists actually be non-tight
    detighten_list(list_being_added_to_id_op, parsing_state, ast);

    // its a blank line from here on out
    // if this is true, then we must have created some structure (list, bq)
    if line_state.is_blank_line() {
        assert_matches!(
            ast.get_block(parsing_state.open_block_id),
            List(..) | BlockQuote(..)
        );
        return;
    }

    //now check if theres a paragraph we can continue lazily
    if let Some(lazy_paragraph_id) = parsing_state.lcpi_op
        && let Paragraph(inline, true) = ast.get_block(lazy_paragraph_id)
    {
        line_state.consume_indent();
        inline.add_extra_space_and_line(0, line_state.get_remaining_str());
        return;
    }

    // if theres no paragraph to continue, check to create an indented code block
    if line_state.space_from_last_structure >= 4 {
        ast.add_new_node(
            parsing_state.dmgci,
            IndentedCodeBlock(
                C::from_extra_space_and_line(
                    line_state.space_from_last_structure - 4,
                    line_state.get_remaining_str(),
                ),
                C::default(),
            ),
        );
        return;
    }

    // finally, we can create a new paragraph with the line remaining
    // since we have been consuming spaces beforehand
    line_state.consume_indent();
    ast.add_new_node(
        parsing_state.dmgci,
        Paragraph(
            C::from_extra_space_and_line(0, line_state.get_remaining_str()),
            true,
        ),
    );
}
