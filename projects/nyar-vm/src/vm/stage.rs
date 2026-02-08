#[derive(Clone)]
pub struct CodeFragment(pub Vec<u8>);

pub fn quote(bytes: Vec<u8>) -> CodeFragment {
    CodeFragment(bytes)
}
pub fn eval(_: CodeFragment) {}
