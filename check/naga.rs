pub mod front { pub mod wgsl {
    pub struct ParseError;
    pub struct Module;
    pub fn parse_str(_: &str) -> Result<Module, ParseError> { Ok(Module) }
} }
