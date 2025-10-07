/// Finds and parses class files.
pub struct Parser {}

impl Default for Parser {
    fn default() -> Self {
        Self {}
    }
}

impl Parser {
    pub fn parse(&self, identifier: &ClassIdentifier) {}
}

pub struct ClassIdentifier {
    package: String,
    name: String,
}

impl ClassIdentifier {
    pub fn new(package: String, name: String) -> Self {
        Self { package, name }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system() {
        let parser = Parser::default();
        let class_identifier = ClassIdentifier::new("java.lang".to_owned(), "System".to_owned());

        parser.parse(&class_identifier);
    }
}
