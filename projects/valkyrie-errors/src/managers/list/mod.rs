use super::*;

pub type FileID = usize;

#[derive(Copy, Clone, Debug, Default, Serialize, Deserialize)]
pub struct FileSpan {
    pub file: FileID,
    pub head: usize,
    pub tail: usize,
}
impl FileSpan {
    pub fn as_label(&self, message: String) -> Label<(FileID, Range<usize>)> {
        Label::new((self.file, self.head..self.tail)).with_message(message)
    }
}
