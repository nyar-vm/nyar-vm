use nyar_ast::AST;

trait Codegen<T> {
    fn get_codes(self) -> Vec<String>;
}

impl Codegen<AST> for AST {
    fn get_codes(self) -> Vec<String> {
        unimplemented!()
    }
}
