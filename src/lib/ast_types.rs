trait Block {
    fn to_html(&self) -> String {
        String::from("")
    }

    fn continuation_condition(&self, line: &mut String) -> bool {
        false
    }
}

pub struct Document {
    child: Vec<Box<dyn Block>>,
}

pub struct BlockQuote {}

pub struct ListItem {}

pub struct List {}

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
