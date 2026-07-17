pub struct Document {}

trait Block {
    fn to_html(&self) -> String;
}
