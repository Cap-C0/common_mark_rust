mod ast_types;

pub fn markdown_to_html(markdown: &str) -> String {
    String::from("")
}

/*
Each line that is processed has an effect on this tree.
The line is analyzed and, depending on its contents, the
document may be altered in one or more of the following ways:
    1. One or more open blocks may be closed.
    2. One or more new blocks may be created as children of the last open block.
    3. Text may be added to the last (deepest) open block remaining on the tree.
 */
