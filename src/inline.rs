use crate::arena_dll::*;
use crate::block_structure::LRDTable;
use crate::chars::*;
use crate::inline::InlineContent::InlineLink;
use crate::inline::InlineContent::*;
use crate::inline::InlineStructureDelimiters::*;
use crate::parsers::ReferenceLinkType::Collapsed;
use crate::parsers::ReferenceLinkType::Full;
use crate::parsers::*;
use crate::peekable_char_indices::*;
use crate::string_normalize::normalize_label;
use std::fmt::Debug;
use std::ops::{Add, Range};
use std::vec;

include!(concat!(env!("OUT_DIR"), "/unicode_categories.rs"));

pub trait CharsInRange: Iterator<Item = char> + Clone {}
impl<T: Iterator<Item = char> + Clone> CharsInRange for T {}

pub trait LeafContainerInline {
    type Offset: Add<usize, Output = Self::Offset> + Ord + Clone + Debug + Copy;
    type Chars<'a>: PeekableCharIndices<Offset = Self::Offset> + Debug
    where
        Self: 'a;

    fn to_html(&self, string_builder: &mut String, lrd_table: &LRDTable);
    fn is_empty(&self) -> bool;
    fn char_iter(&self) -> impl Iterator<Item = char>;
    fn as_pci(&self) -> impl PeekableCharIndices<Offset = Self::Offset>;
    fn chars_in_range<'a>(&'a self, range: Range<Self::Offset>) -> Self::Chars<'a>;
}

impl LeafContainerInline for String {
    type Offset = usize;
    type Chars<'a>
        = BorrowedStringPCI<'a>
    where
        Self: 'a;
    fn to_html(&self, string_builder: &mut String, lrd_table: &LRDTable) {
        for ic in parse_inline(self, lrd_table) {
            ic.to_html(self, string_builder, lrd_table);
        }
    }
    fn is_empty(&self) -> bool {
        self.is_empty()
    }
    fn char_iter(&self) -> impl Iterator<Item = char> {
        self.chars()
    }

    fn as_pci(&self) -> impl PeekableCharIndices<Offset = Self::Offset> {
        BorrowedStringPCI::new(self)
    }

    fn chars_in_range<'a>(&'a self, range: Range<Self::Offset>) -> Self::Chars<'a> {
        BorrowedStringPCI::new(&self[range])
    }
}

// pub fn inline_string_to_html(str_in: &str, string_builder: &mut String, lrd_table: &LRDTable) {
//     for ic in parse_inline(str_in, lrd_table) {
//         ic.to_html(str_in, string_builder, lrd_table);
//     }
// }

// we do not need to enforce multiple new line requirements in these parsers as that will be enforced
// by paragraphs ending at new lines
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum InlineContent<T: PeekableCharIndices> {
    Softbreak,
    Hardbreak,
    Text(T),
    Emph(Vec<InlineContent<T>>),
    Strong(Vec<InlineContent<T>>),
    /// is_image, href, title, link_text
    //TODO: make this work with reference links
    InlineLink(bool, Option<T>, Option<T>, Vec<InlineContent<T>>),
    ReferenceLink(bool, String, Vec<InlineContent<T>>),
    /// src, title, link_text
    // Image((usize, usize), (usize, usize), Vec<InlineContent>),
    /// href and text, is_email
    AutoLink(T, bool),
    /// start, end char index (exclusive)
    HTMLTag(T),
    Code(T),
    // for use when swapping memory
    Dummy,
}

impl<T> InlineContent<T>
where
    T: PeekableCharIndices,
{
    //TODO: Make these all use fmt instead of string?
    pub fn to_html(&self, string_array: &str, string_builder: &mut String, lrd_table: &LRDTable) {
        match self {
            Softbreak => string_builder.push('\n'),
            Hardbreak => string_builder.push_str("<br />\n"),
            Text(chars) => {
                push_chars_with_entities_and_bs(chars, string_builder, false);
            }
            Emph(inline_contents) => {
                string_builder.push_str("<em>");
                for ic in inline_contents {
                    ic.to_html(string_array, string_builder, lrd_table);
                }
                string_builder.push_str("</em>");
            }
            Strong(inline_contents) => {
                string_builder.push_str("<strong>");
                for ic in inline_contents {
                    ic.to_html(string_array, string_builder, lrd_table);
                }
                string_builder.push_str("</strong>");
            }
            InlineLink(is_image, dest_op, tit_op, inline_contents) => {
                if *is_image {
                    string_builder.push_str("<img src=\"");
                    if let Some(dest_chars) = dest_op {
                        push_chars_with_entities_and_bs(dest_chars, string_builder, true);
                    }
                    string_builder.push_str("\" alt=\"");
                    for ic in inline_contents {
                        ic.to_alt_text(string_array, string_builder);
                    }
                    string_builder.push_str("\" ");
                    if let Some(tit_chars) = tit_op {
                        string_builder.push_str("title=\"");
                        push_chars_with_entities_and_bs(tit_chars, string_builder, false);
                        string_builder.push_str("\" ");
                    }
                    string_builder.push_str("/>");
                } else {
                    string_builder.push_str("<a href=\"");
                    if let Some(dest_chars) = dest_op {
                        push_chars_with_entities_and_bs(dest_chars, string_builder, true);
                    }
                    string_builder.push('\"');
                    if let Some(tit_chars) = tit_op {
                        string_builder.push_str(" title=\"");
                        push_chars_with_entities_and_bs(tit_chars, string_builder, false);
                        string_builder.push('\"');
                    }
                    string_builder.push('>');
                    for ic in inline_contents {
                        ic.to_html(string_array, string_builder, lrd_table);
                    }
                    string_builder.push_str("</a>");
                }
            }
            ReferenceLink(is_image, normalized_label, inline_contents) => {
                let (dest, tit_op) = lrd_table.get(normalized_label).unwrap();
                if *is_image {
                    string_builder.push_str("<img src=\"");
                    push_chars_with_entities_and_bs(
                        &BorrowedStringPCI::new(dest),
                        string_builder,
                        true,
                    );
                    string_builder.push_str("\" alt=\"");
                    for ic in inline_contents {
                        ic.to_alt_text(string_array, string_builder);
                    }
                    string_builder.push_str("\" ");
                    if let Some(tit_string) = tit_op {
                        string_builder.push_str("title=\"");
                        push_chars_with_entities_and_bs(
                            &BorrowedStringPCI::new(tit_string),
                            string_builder,
                            false,
                        );
                        string_builder.push_str("\" ");
                    }
                    string_builder.push_str("/>");
                } else {
                    string_builder.push_str("<a href=\"");
                    push_chars_with_entities_and_bs(
                        &BorrowedStringPCI::new(dest),
                        string_builder,
                        true,
                    );
                    string_builder.push('\"');
                    if let Some(tit) = tit_op {
                        string_builder.push_str(" title=\"");
                        push_chars_with_entities_and_bs(
                            &BorrowedStringPCI::new(tit),
                            string_builder,
                            false,
                        );
                        string_builder.push('\"');
                    }
                    string_builder.push('>');
                    for ic in inline_contents {
                        ic.to_html(string_array, string_builder, lrd_table);
                    }
                    string_builder.push_str("</a>");
                }
            }
            AutoLink(link_chars, is_email) => {
                string_builder.push_str("<a href=\"");
                if *is_email {
                    string_builder.push_str("mailto:");
                }
                for c in link_chars.clone() {
                    push_character_in_uri(c, string_builder);
                }
                string_builder.push_str("\">");
                for c in link_chars.clone() {
                    push_html_reserved_char(c, string_builder);
                }
                string_builder.push_str("</a>");
            }
            HTMLTag(html_chars) => {
                for c in html_chars.clone() {
                    string_builder.push(c);
                }
            }
            Code(code_chars) => {
                string_builder.push_str("<code>");
                for c in code_chars.clone() {
                    if c == '\n' {
                        push_html_reserved_char(' ', string_builder);
                    } else {
                        push_html_reserved_char(c, string_builder);
                    }
                }
                string_builder.push_str("</code>");
            }
            Dummy => panic!("should not encounter dummy at this point"),
        }
    }

    pub fn to_alt_text(&self, _string_array: &str, _string_builder: &mut String) {
        match self {
            Softbreak => _string_builder.push('\n'),
            Hardbreak => _string_builder.push('\n'),
            Text(text_chars) => {
                push_chars_with_entities_and_bs(text_chars, _string_builder, false);
            }
            Emph(inline_contents) => {
                for ic in inline_contents {
                    ic.to_alt_text(_string_array, _string_builder);
                }
            }
            Strong(inline_contents) => {
                for ic in inline_contents {
                    ic.to_alt_text(_string_array, _string_builder);
                }
            }
            InlineLink(.., inline_contents) => {
                for ic in inline_contents {
                    ic.to_alt_text(_string_array, _string_builder);
                }
            }
            ReferenceLink(.., inline_contents) => {
                for ic in inline_contents {
                    ic.to_alt_text(_string_array, _string_builder);
                }
            }
            AutoLink(link_chars, ..) => {
                for c in link_chars.clone() {
                    push_html_reserved_char(c, _string_builder);
                }
            }
            HTMLTag(html_chars) => {
                for c in html_chars.clone() {
                    _string_builder.push(c);
                }
            }
            Code(code_chars) => {
                for c in code_chars.clone() {
                    if c == '\n' {
                        push_html_reserved_char(' ', _string_builder);
                    } else {
                        push_html_reserved_char(c, _string_builder);
                    }
                }
            }
            Dummy => panic!("should not encounter dummy at this point"),
        }
    }
}

#[derive(Debug, Clone)]
enum InlineStructureDelimiters<T: PeekableCharIndices> {
    /// Total_count,Count_consumed, potential_opener, potential_closer
    Asts(usize, usize, bool, bool),
    /// Total_count,Count_consumed, potential_opener, potential_closer
    Unds(usize, usize, bool, bool),
    ImgOpen(bool),
    /// active
    LinkOpen(bool),
    BrackClose,
    /// Total_count
    /// Offset to last char (exclusive)
    TextualContent(Range<T::Offset>),
    CompletedContent(InlineContent<T>),
}

impl<T> InlineStructureDelimiters<T>
where
    T: PeekableCharIndices,
{
    fn convert_to_completed_content(&mut self, start_index: T::Offset) {
        if matches!(self, TextualContent(..) | CompletedContent(..)) {
            return;
        }
        *self = match self {
            Unds(total, consumed, ..) | Asts(total, consumed, ..) => {
                TextualContent(start_index..(start_index + (*total - *consumed)))
            }
            ImgOpen(..) => TextualContent(start_index..(start_index + 2)),
            _ => TextualContent(start_index..(start_index + 1)),
        }
    }

    fn convert_to_inline_content<'a>(
        self,
        char_offset: T::Offset,
        // base_string: &'a str,
        base_container: &'a impl LeafContainerInline<Offset = T::Offset, Chars<'a> = T>,
    ) -> InlineContent<T> {
        match self {
            Unds(total, consumed, ..) | Asts(total, consumed, ..) => {
                Text(base_container.chars_in_range(char_offset..(char_offset + (total - consumed))))
            }
            TextualContent(range) => Text(base_container.chars_in_range(range)),
            ImgOpen(..) => Text(base_container.chars_in_range(char_offset..(char_offset + 2))),
            CompletedContent(c) => c,
            _ => Text(base_container.chars_in_range(char_offset..(char_offset + 1))),
        }
    }
}

type DLStack<'a, T> = ArenaDLL<
    InlineStructureDelimiters<<T as LeafContainerInline>::Chars<'a>>,
    <T as LeafContainerInline>::Offset,
>;

//BackTick code spans, auto links, raw_html >
//brackets in link text >
//emph markers.
pub fn parse_inline<'a, C>(
    inline_container: &'a C,
    lrd_table: &LRDTable,
) -> Vec<InlineContent<C::Chars<'a>>>
where
    C: LeafContainerInline,
{
    let mut delimit_stack: DLStack<C> = ArenaDLL::default();
    let mut char_iter = inline_container.as_pci();

    let add_text_to_stack = |sicl: &mut DLStack<C>, text_begin: C::Offset, text_end: C::Offset| {
        if text_end > text_begin {
            sicl.push_back(
                text_begin,
                InlineStructureDelimiters::TextualContent(text_begin..text_end),
            );
        }
    };
    let mut text_begin = char_iter.offset();
    let mut first_space_offset: Option<C::Offset> = None;
    let mut space_count = 0;
    let mut preceding_char = ' '; // for purposes of emphasis delimeter types (beginning counts as
    // whitespace)
    //do the function!
    while let Some((char_index, c)) = char_iter.next_and_index() {
        match c {
            ' ' => {
                space_count += 1;
                if first_space_offset.is_none() {
                    first_space_offset = Some(char_index);
                }
            }
            '\n' => {
                add_text_to_stack(
                    &mut delimit_stack,
                    text_begin,
                    first_space_offset.unwrap_or(char_index),
                );
                if space_count >= 2 {
                    delimit_stack.push_back(
                        char_index,
                        InlineStructureDelimiters::CompletedContent(Hardbreak),
                    );
                } else {
                    delimit_stack.push_back(
                        char_index,
                        InlineStructureDelimiters::CompletedContent(Softbreak),
                    );
                }
                space_count = 0;
                first_space_offset = None;
                text_begin = char_iter.offset();
            }
            _ => {
                space_count = 0;
                first_space_offset = None;
            }
        }
        match c {
            '\\' => {
                if let Some((o, c)) = char_iter.next_and_index() {
                    if c.is_ascii_punctuation() {
                        add_text_to_stack(&mut delimit_stack, text_begin, char_index);
                        //exclude the backslash, include just the punctuation
                        delimit_stack
                            .push_back(o, InlineStructureDelimiters::TextualContent(o..o + 1));
                        // both characters are 1 byte.
                        text_begin = char_iter.offset();
                    } else if c == '\n' {
                        add_text_to_stack(&mut delimit_stack, text_begin, char_index);
                        delimit_stack.push_back(char_index, CompletedContent(Hardbreak));
                        text_begin = char_iter.offset();
                    }
                    preceding_char = c;
                }
            }
            '`' => {
                // since the backticks are ascii, they are one byte and safe to do string offset math on throughout
                // TODO: space stripping.
                add_text_to_stack(&mut delimit_stack, text_begin, char_index);

                let initial_tick_count = 1 + char_iter.consume_while_char_eq('`');
                let start_offset = char_iter.offset();

                let mut matching_tick_count_search_iter = char_iter.clone();

                let mut code_completed = false;
                let starts_with_space = matching_tick_count_search_iter
                    .next_if(|c| " \n".contains(c))
                    .map(|_| matching_tick_count_search_iter.offset());
                let mut entirely_space = starts_with_space.is_some();
                let mut last_is_space = starts_with_space;
                'search_for_matching_tc: while let Some((current_offset, c)) =
                    matching_tick_count_search_iter.next_and_index()
                {
                    if c == '`' {
                        let closing_tick_count =
                            1 + matching_tick_count_search_iter.consume_while_char_eq('`');
                        if closing_tick_count == initial_tick_count {
                            char_iter = matching_tick_count_search_iter;
                            delimit_stack.push_back(
                                char_index,
                                CompletedContent(Code(inline_container.chars_in_range(
                                    if let Some(after_first_space) = starts_with_space
                                        && let Some(before_last_space) = last_is_space
                                        && !entirely_space
                                    {
                                        after_first_space..before_last_space
                                    } else {
                                        start_offset..current_offset
                                    },
                                ))),
                            );
                            code_completed = true;
                            text_begin = current_offset + closing_tick_count;
                            break 'search_for_matching_tc;
                        }
                    }
                    if " \n".contains(c) {
                        last_is_space = Some(current_offset);
                    } else {
                        last_is_space = None;
                        entirely_space = false;
                    }
                }
                if !code_completed {
                    add_text_to_stack(
                        &mut delimit_stack,
                        char_index,
                        char_index + initial_tick_count,
                    );
                    text_begin = char_index + initial_tick_count;
                }
                preceding_char = '`';
            }
            '_' | '*' => {
                // these delimeters are size 1 byte, so it is possible to count chars by subtracting
                // indices.
                add_text_to_stack(&mut delimit_stack, text_begin, char_index);

                let dc = 1 + char_iter.consume_while_char_eq(c);

                //for these purposes, end and beginning of line count as whitespace.
                let following_char = char_iter.peek().unwrap_or(' ');

                let punc_preceded =
                    preceding_char.is_unicode_punctuation() || preceding_char.is_unicode_symbol();
                let punc_followed =
                    following_char.is_unicode_punctuation() || following_char.is_unicode_symbol();
                let is_left_flanking: bool = !following_char.is_whitespace()
                    && (!punc_followed
                        || (punc_followed && (preceding_char.is_whitespace() || punc_preceded)));
                let is_right_flanking: bool = !preceding_char.is_whitespace()
                    && (!punc_preceded
                        || (punc_preceded && (following_char.is_whitespace() || punc_followed)));
                delimit_stack.push_back(
                    char_index,
                    match c {
                        '*' => Asts(dc, 0, is_left_flanking, is_right_flanking),
                        '_' => Unds(
                            dc,
                            0,
                            is_left_flanking
                                && (!is_right_flanking || (is_right_flanking && punc_preceded)),
                            is_right_flanking
                                && (!is_left_flanking || (is_left_flanking && punc_followed)),
                        ),
                        _ => panic!(),
                    },
                );
                text_begin = char_index + dc;
                preceding_char = c;
            }
            '!' => {
                if char_iter.next_if_char_eq('[').is_some() {
                    add_text_to_stack(&mut delimit_stack, text_begin, char_index);
                    delimit_stack.push_back(char_index, ImgOpen(true));
                    text_begin = char_iter.offset();
                    preceding_char = '[';
                } else {
                    preceding_char = c;
                }
            }
            '[' => {
                add_text_to_stack(&mut delimit_stack, text_begin, char_index);
                delimit_stack.push_back(char_index, LinkOpen(true));
                text_begin = char_index + 1;
                preceding_char = '[';
            }
            ']' => {
                // do the back search thing.
                add_text_to_stack(&mut delimit_stack, text_begin, char_index);
                let mut matched_node_id_op = delimit_stack.get_end_id();

                while let Some(ref matched_node_id) = matched_node_id_op {
                    if matches!(
                        delimit_stack.get_content(*matched_node_id),
                        LinkOpen(..) | ImgOpen(..)
                    ) {
                        break;
                    }
                    matched_node_id_op = delimit_stack.get_prev_id(*matched_node_id);
                }

                let Some(matched_node_id) = matched_node_id_op else {
                    delimit_stack.push_back(char_index, BrackClose);
                    text_begin = char_iter.offset();
                    continue;
                };

                let matched_itc = delimit_stack.get_content(matched_node_id);
                if matches!(
                    delimit_stack.get_content(matched_node_id),
                    LinkOpen(false, ..) | ImgOpen(false, ..)
                ) {
                    let start_index = *delimit_stack.get_position_indicator(matched_node_id);
                    delimit_stack
                        .get_content_mut(matched_node_id)
                        .convert_to_completed_content(start_index);
                    add_text_to_stack(&mut delimit_stack, text_begin, char_index);
                    delimit_stack.push_back(char_index, BrackClose);
                    text_begin = char_iter.offset();
                    continue;
                }
                let is_image = matches!(matched_itc, ImgOpen(..));
                // we have found one and it is active
                // let link_content_iter = match matched_node_itc {
                //     LinkOpen(true, iter) | ImgOpen(true, iter) => iter.clone(),
                //     _ => unreachable!(),
                // };

                let mut link_made = false;
                '_try_to_make_link: {
                    let mut try_to_inline_link_iter = char_iter.clone();
                    let mut try_to_reference_link_iter = char_iter.clone();
                    if let Some((dest_op, tit_op)) =
                        parse_inline_suffix(&mut try_to_inline_link_iter)
                    {
                        let bci = *delimit_stack.get_position_indicator(matched_node_id);

                        let link_displayed = process_emphasis(
                            Some(matched_node_id),
                            &mut delimit_stack,
                            inline_container,
                        );
                        delimit_stack.push_back(
                            bci,
                            CompletedContent(InlineLink(
                                is_image,
                                dest_op.map(|range| inline_container.chars_in_range(range)),
                                tit_op.map(|range| inline_container.chars_in_range(range)),
                                link_displayed,
                            )),
                        );
                        char_iter = try_to_inline_link_iter;
                        text_begin = char_iter.offset();
                        link_made = true;
                    } else if let Some(rlt) = parse_reference_link(&mut try_to_reference_link_iter)
                    {
                        match rlt {
                            Full(range) => {
                                // (&(start, end));
                                let my_str: String =
                                    inline_container.chars_in_range(range).collect();
                                let norm_lab = normalize_label(&my_str);
                                // (lrd_table);
                                // (&norm_lab);
                                if lrd_table.contains_key(&norm_lab) {
                                    let bci =
                                        *delimit_stack.get_position_indicator(matched_node_id);
                                    let link_displayed = process_emphasis(
                                        Some(matched_node_id),
                                        &mut delimit_stack,
                                        inline_container,
                                    );
                                    delimit_stack.push_back(
                                        bci,
                                        CompletedContent(ReferenceLink(
                                            is_image,
                                            norm_lab,
                                            link_displayed,
                                        )),
                                    );
                                    char_iter = try_to_reference_link_iter;
                                    text_begin = char_iter.offset();
                                    link_made = true;
                                }
                            }
                            Collapsed => {
                                let open_brack_offset = (*delimit_stack
                                    .get_position_indicator(matched_node_id))
                                    + if is_image { 1 } else { 0 };
                                let my_str: String = inline_container
                                    .chars_in_range(open_brack_offset..char_iter.offset())
                                    .collect();
                                let norm_lab = normalize_label(&my_str);
                                if lrd_table.contains_key(&norm_lab) {
                                    let bci =
                                        *delimit_stack.get_position_indicator(matched_node_id);
                                    let link_displayed = process_emphasis(
                                        Some(matched_node_id),
                                        &mut delimit_stack,
                                        inline_container,
                                    );
                                    delimit_stack.push_back(
                                        bci,
                                        CompletedContent(ReferenceLink(
                                            is_image,
                                            norm_lab,
                                            link_displayed,
                                        )),
                                    );
                                    char_iter = try_to_reference_link_iter;
                                    text_begin = char_iter.offset();
                                    link_made = true;
                                }
                            }
                        }
                    } else {
                        let open_brack_offset = (*delimit_stack
                            .get_position_indicator(matched_node_id))
                            + if is_image { 1 } else { 0 };
                        let my_str: String = inline_container
                            .chars_in_range(open_brack_offset..char_iter.offset())
                            .collect();
                        let norm_lab = normalize_label(&my_str);
                        if lrd_table.contains_key(&norm_lab) {
                            let bci = *delimit_stack.get_position_indicator(matched_node_id);
                            let link_displayed = process_emphasis(
                                Some(matched_node_id),
                                &mut delimit_stack,
                                inline_container,
                            );
                            delimit_stack.push_back(
                                bci,
                                CompletedContent(ReferenceLink(is_image, norm_lab, link_displayed)),
                            );
                            char_iter = try_to_reference_link_iter;
                            text_begin = char_iter.offset();
                            link_made = true;
                        }
                    }
                }
                if link_made {
                    delimit_stack.take_content_at_id(matched_node_id);
                    if !is_image {
                        let mut before_opener_id_op = delimit_stack.get_prev_id(matched_node_id);
                        while let Some(before_opener_id) = before_opener_id_op {
                            if let LinkOpen(b @ true) =
                                delimit_stack.get_content_mut(before_opener_id)
                            {
                                *b = false;
                            }
                            before_opener_id_op = delimit_stack.get_prev_id(before_opener_id)
                        }
                    }
                } else {
                    let start_index = *delimit_stack.get_position_indicator(matched_node_id);
                    delimit_stack
                        .get_content_mut(matched_node_id)
                        .convert_to_completed_content(start_index);
                    delimit_stack
                        .push_back(char_index, TextualContent(char_index..char_iter.offset()));
                    text_begin = char_iter.offset();
                }
                //could technically be ')', but doesnt matter for purposes of is_punc/is_not_punc
                preceding_char = ']';
            }
            '<' => {
                let real_range_bottom = char_index;
                //eagerly try to make autolink or html,
                add_text_to_stack(&mut delimit_stack, text_begin, char_index);
                let mut autolink_iter = char_iter.clone();
                let mut html_iter = char_iter.clone();
                if let Some((range, is_email)) = parse_autolink(&mut autolink_iter) {
                    delimit_stack.push_back(
                        char_index,
                        CompletedContent(AutoLink(
                            inline_container.chars_in_range(range),
                            is_email,
                        )),
                    );
                    char_iter = autolink_iter;
                    text_begin = char_iter.offset();
                    preceding_char = '>'
                } else if let Some(tag_range) =
                    parse_html_tag(&mut html_iter).map(|r| real_range_bottom..r.end)
                {
                    delimit_stack.push_back(
                        char_index,
                        CompletedContent(HTMLTag(inline_container.chars_in_range(tag_range))),
                    );
                    char_iter = html_iter;
                    text_begin = char_iter.offset();
                    preceding_char = '>'
                } else {
                    add_text_to_stack(&mut delimit_stack, char_index, char_index + 1);
                    text_begin = char_index + 1;
                    preceding_char = '<'
                }
            }
            _ => {
                preceding_char = c;
            }
        }
    }
    add_text_to_stack(
        &mut delimit_stack,
        text_begin,
        first_space_offset.unwrap_or(char_iter.offset()),
    );

    process_emphasis(None, &mut delimit_stack, inline_container)
}

//TODO!
fn process_emphasis<'a, C>(
    stack_bottom_id_op: Option<DLLNodeID>,
    dl_stack: &mut DLStack<'a, C>,
    inline_container: &'a C,
) -> Vec<InlineContent<C::Chars<'a>>>
where
    C: LeafContainerInline,
{
    // todo!();
    let stack_bottom_char_offset: Option<C::Offset> =
        stack_bottom_id_op.map(|id| *dl_stack.get_position_indicator(id));

    let mut current_id_op = if let Some(stack_bottom_id) = stack_bottom_id_op {
        dl_stack.get_next_id(stack_bottom_id)
    } else {
        dl_stack.get_start_id()
    };

    //     stack_bottom.map_or(stack.initial_index, |i| stack.get_content(i).index_of_next);
    // let stack_bottom_char_index =
    //     stack_bottom.map(|dll_pointer| stack.get_content(dll_pointer).beginning_char_index);
    // // only set op_bot when we find a non_matched closer_delimiter
    // // set it as the *CHARACTER_OFFSET* in the corresponding vec of chars.
    // TODO: I think only and_opening needs this adjustment?
    // maybe write some generated tests to see what values ever actually change?
    let mut openers_bottom_asts_and_opening: [Option<C::Offset>; 3] = [stack_bottom_char_offset; 3];
    let mut openers_bottom_asts_not_opening: [Option<C::Offset>; 3] = [stack_bottom_char_offset; 3];

    let mut openers_bottom_unds_and_opening: [Option<C::Offset>; 3] = [stack_bottom_char_offset; 3];
    let mut openers_bottom_unds_not_opening: [Option<C::Offset>; 3] = [stack_bottom_char_offset; 3];
    // index_in_fakedldll, char_offset it points to
    let mut asts_op_stack: Vec<DLLNodeID> = vec![];
    let mut unds_op_stack: Vec<DLLNodeID> = vec![];
    // todo!();
    //
    // //TODO: DRY THIS SOMEHOW, UNDS AND ASTS are the same except variable names
    while let Some(current_id) = current_id_op {
        // (dl_stack.get_content(current_id));
        match dl_stack.get_content(current_id) {
            Asts(total_count, consumed, pot_op, pot_close)
            | Unds(total_count, consumed, pot_op, pot_close) => {
                let (this_opener_bottom, this_op_stack, other_op_stack) =
                    match dl_stack.get_content(current_id) {
                        Asts(.., po, _) => (
                            if *po {
                                &mut openers_bottom_asts_and_opening[*total_count % 3]
                            } else {
                                &mut openers_bottom_asts_not_opening[*total_count % 3]
                            },
                            &mut asts_op_stack,
                            &mut unds_op_stack,
                        ),
                        Unds(.., po, _) => (
                            if *po {
                                &mut openers_bottom_unds_and_opening[*total_count % 3]
                            } else {
                                &mut openers_bottom_unds_not_opening[*total_count % 3]
                            },
                            &mut unds_op_stack,
                            &mut asts_op_stack,
                        ),
                        _ => unreachable!(),
                    };
                if *pot_close && !this_op_stack.is_empty() {
                    let mut this_op_stack_offset = this_op_stack.len() - 1;
                    let matching_dl_id_op: Option<DLLNodeID> = loop {
                        if this_opener_bottom.is_none_or(|x| {
                            *dl_stack.get_position_indicator(this_op_stack[this_op_stack_offset])
                                > x
                        }) {
                            match dl_stack.get_content(this_op_stack[this_op_stack_offset]) {
                                Asts(opener_tc, _, true, opener_pc)
                                | Unds(opener_tc, _, true, opener_pc) => {
                                    if (*opener_pc || *pot_op)
                                        && (total_count + *opener_tc) % 3 == 0
                                        && !(total_count % 3 == 0 && opener_tc % 3 == 0)
                                    {
                                        if this_op_stack_offset == 0 {
                                            break None;
                                        }
                                        this_op_stack_offset -= 1;
                                    } else {
                                        break Some(this_op_stack[this_op_stack_offset]);
                                    }
                                }
                                _ => unreachable!(),
                            }
                        } else {
                            break None;
                        }
                    };

                    let Some(matching_dl_id) = matching_dl_id_op else {
                        *this_opener_bottom = dl_stack
                            .get_prev_id(current_id)
                            .map(|prev_id| *dl_stack.get_position_indicator(prev_id));
                        if *pot_op {
                            asts_op_stack.push(current_id);
                        }
                        // we don't need a notion of "deleting" non potential_openers Since
                        // we track backwards in a different stack than this.
                        current_id_op = dl_stack.get_next_id(current_id);
                        continue;
                    };
                    let (op_tc, op_used) = match dl_stack.get_content(matching_dl_id) {
                        Asts(ot, ou, ..) | Unds(ot, ou, ..) => (ot, ou),
                        _ => unreachable!(),
                    };
                    let matching_dl_unconsumed = op_tc - op_used;
                    let this_dl_unconsumed = total_count - consumed;
                    let is_strong = matching_dl_unconsumed >= 2 && this_dl_unconsumed >= 2;
                    // turn stack items inside the stack delimiters into actual inline content.
                    let mut emph_children: Vec<InlineContent<C::Chars<'a>>> = vec![];
                    let mut start_pos = None;
                    for (isd, pos) in dl_stack.take_content_between(matching_dl_id, current_id) {
                        if start_pos.is_none() {
                            start_pos = Some(pos);
                        }
                        emph_children.push(isd.convert_to_inline_content(pos, inline_container));
                    }
                    // also clear out any delimiters on our local stack that we took inside the emphasis
                    while this_op_stack
                        .pop_if(|node_id| dl_stack.is_taken(*node_id))
                        .is_some()
                    {}
                    while other_op_stack
                        .pop_if(|node_id| dl_stack.is_taken(*node_id))
                        .is_some()
                    {}
                    dl_stack.replace_inside_stack_range(
                        matching_dl_id,
                        current_id,
                        start_pos.unwrap(),
                        CompletedContent(if is_strong {
                            Strong(emph_children)
                        } else {
                            Emph(emph_children)
                        }),
                    );
                    match dl_stack.get_content_mut(current_id) {
                        Asts(total, used, ..) | Unds(total, used, ..) => {
                            *used += if is_strong { 2 } else { 1 };
                            if *used == *total {
                                current_id_op = dl_stack.get_next_id(current_id);
                                dl_stack.delete(current_id);
                            }
                        }
                        _ => unreachable!(),
                    };
                    match dl_stack.get_content_mut(matching_dl_id) {
                        Asts(total, used, ..) | Unds(total, used, ..) => {
                            *used += if is_strong { 2 } else { 1 };
                            if *used == *total {
                                dl_stack.delete(matching_dl_id);
                                this_op_stack.remove(this_op_stack_offset);
                            }
                        }
                        _ => unreachable!(),
                    };
                } else {
                    if *pot_op {
                        this_op_stack.push(current_id);
                    }
                    current_id_op = dl_stack.get_next_id(current_id);
                }
            }
            _ => current_id_op = dl_stack.get_next_id(current_id),
        }
    }
    dl_stack
        .take_content_above(stack_bottom_id_op)
        .map(|(isd, pos)| isd.convert_to_inline_content(pos, inline_container))
        .collect()
}
