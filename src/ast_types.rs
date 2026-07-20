pub trait Block {
    fn to_html(&self) -> String {
        String::from("")
    }

    //use vec of chars instead of string for better unicode performance
    fn continuation_condition(&self, line: &mut Vec<char>, offset: &mut usize) -> bool {
        false
    }
}

pub trait Container {
    fn get_last_child(&mut self) -> Option<&mut (dyn Block + '_)>;
}

pub struct Document {
    pub children: Vec<Box<dyn Block>>,
}

impl Block for Document {
    fn continuation_condition(&self, line: &mut Vec<char>, offset: &mut usize) -> bool {
        true // Document doesnt stop being valid til the end
    }

    fn to_html(&self) -> String {
        self.children.iter().map(|child| child.to_html()).collect()
    }
}

impl Container for Document {
    fn get_last_child(mut self) -> Option<&mut (dyn Block + '_)> {
        self.children.last_mut().map(|b| b.as_mut())
    }
}

pub struct BlockQuote {
    pub children: Vec<Box<dyn Block>>,
}

impl Block for BlockQuote {
    fn continuation_condition(&self, line: &mut Vec<char>, offset: &mut usize) -> bool {
        for i in 0..3 {
            if line.len() >= *offset + i {
                return false;
            }
            if line[*offset + i] == '>' {
                //bounds check
                if line.len() >= *offset + i + 1 {
                    *offset += i + 1;
                    return true;
                }
                if line[*offset + i + 1] == ' ' {
                    *offset += i + 2;
                } else {
                    *offset += i + 1;
                }
                return true;
            }
            if line[*offset + i] != ' ' {
                return false;
            }
        }
        false
    }

    fn to_html(&self) -> String {
        let mut out = "<blockquote>\n".to_string();
        let child: String = self.children.iter().map(|child| child.to_html()).collect();
        out.push_str(&child);
        out.push_str("\n<blockquote>");
        out
    }
}

impl Container for BlockQuote {
    fn get_last_child<'a>(&'a mut self) -> Option<&mut dyn Block> {
        self.children.last_mut().map(|b| b.as_mut())
    }
}

pub struct ListItem {
    pub children: Vec<Box<dyn Block>>,
    pub indent_amount: usize,
}

impl Block for ListItem {
    fn continuation_condition(&self, line: &mut Vec<char>, offset: &mut usize) -> bool {
        if line.len() - *offset > self.indent_amount {
            for i in line.len()..(line.len() + self.indent_amount) {
                if line[i] != ' ' {
                    return false;
                }
            }
            *offset += self.indent_amount;
            return true;
        }
        false
    }
}

impl Container for ListItem {
    fn get_last_child<'a>(&'a mut self) -> Option<&'a mut dyn Block> {
        self.children.last_mut().map(move |b| b.as_mut())
    }
}

pub enum ListType {
    OrderedList(usize),
    UnorderedList,
}

pub struct List {
    pub children: Vec<Box<ListItem>>,
    pub list_marker: char, // in tandem with list type.
    pub list_type: ListType,
    pub is_loose: bool,
}

impl Block for List {}

impl Container for List {
    fn get_last_child(&mut self) -> Option<&mut dyn Block> {
        self.children
            .last_mut()
            .map(|a| a.as_mut() as &mut dyn Block)
    }
}

pub struct ThematicBreak {}

impl Block for ThematicBreak {
    fn to_html(&self) -> String {
        String::from("<hr \\>")
    }
}

pub struct ATXHeading {
    text: String,
    level: usize,
}

impl Block for ATXHeading {
    fn to_html(&self) -> String {
        format!("<h{}>{}<h{}\\>", self.level, self.text, self.level)
    }
}

pub struct SetextHeading {
    text: String,
    level: usize,
}

impl Block for SetextHeading {
    fn to_html(&self) -> String {
        format!("<h{}>{}<h{}\\>", self.level, self.text, self.level)
    }
}

pub struct IndentedCodeBlock {}
pub struct FencedCodeBlock {}
pub struct HTMLBlock {}
pub struct LinkReferenceDefinition {}
pub struct Paragraphs {}
pub struct BlankLine {}
