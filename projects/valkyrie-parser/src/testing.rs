use std::io::stderr;
use valkyrie_errors::{TextManager, ValkyrieResult};

use crate::ValkyrieParser;


pub fn debug_lexer(files: &[&str]) -> ValkyrieResult {
    let mut parser = ValkyrieParser::default();
    let mut problems = vec![];
    let mut text = TextManager::new("./");
    for file in files {
        // println!("Parsing file: {}", Url::from_file_path(text.resolve_file(file))?);
        let basic = text.add_file(file)?;
        if let Err(e) = parser.parse_file(basic, &text.get_text(basic)) {
            problems.push(e.as_report())
        }
    }
    for e in parser.take_errors() {
        problems.push(e.as_report())
    }
    for problem in problems {
        problem.write(&mut text, stderr().lock())?;
    }
    Ok(())
}
