use std::borrow::Cow;

pub struct Identifier<'a> {
    pub rendered: Cow<'a, str>,
    pub changed: bool,
}

pub fn identifier(value: &str) -> Identifier<'_> {
    if is_valid(value) {
        if is_keyword(value) {
            return Identifier {
                rendered: Cow::Owned(format!("@{value}")),
                changed: false,
            };
        }
        return Identifier {
            rendered: Cow::Borrowed(value),
            changed: false,
        };
    }

    let mut rendered = String::with_capacity(value.len().max(1));
    for (index, character) in value.chars().enumerate() {
        let valid = if index == 0 {
            character == '_' || character.is_alphabetic()
        } else {
            character == '_' || character.is_alphanumeric()
        };
        rendered.push(if valid { character } else { '_' });
    }
    if rendered.is_empty() {
        rendered.push('_');
    }
    Identifier {
        rendered: Cow::Owned(rendered),
        changed: true,
    }
}

pub fn without_generic_arity(value: &str) -> &str {
    let Some((name, arity)) = value.rsplit_once('`') else {
        return value;
    };
    if !arity.is_empty() && arity.bytes().all(|byte| byte.is_ascii_digit()) {
        name
    } else {
        value
    }
}

fn is_valid(value: &str) -> bool {
    let mut characters = value.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    (first == '_' || first.is_alphabetic())
        && characters.all(|character| character == '_' || character.is_alphanumeric())
}

fn is_keyword(value: &str) -> bool {
    matches!(
        value,
        "abstract"
            | "as"
            | "base"
            | "bool"
            | "break"
            | "byte"
            | "case"
            | "catch"
            | "char"
            | "checked"
            | "class"
            | "const"
            | "continue"
            | "decimal"
            | "default"
            | "delegate"
            | "do"
            | "double"
            | "else"
            | "enum"
            | "event"
            | "explicit"
            | "extern"
            | "false"
            | "finally"
            | "fixed"
            | "float"
            | "for"
            | "foreach"
            | "goto"
            | "if"
            | "implicit"
            | "in"
            | "int"
            | "interface"
            | "internal"
            | "is"
            | "lock"
            | "long"
            | "namespace"
            | "new"
            | "null"
            | "object"
            | "operator"
            | "out"
            | "override"
            | "params"
            | "private"
            | "protected"
            | "public"
            | "readonly"
            | "ref"
            | "return"
            | "sbyte"
            | "sealed"
            | "short"
            | "sizeof"
            | "stackalloc"
            | "static"
            | "string"
            | "struct"
            | "switch"
            | "this"
            | "throw"
            | "true"
            | "try"
            | "typeof"
            | "uint"
            | "ulong"
            | "unchecked"
            | "unsafe"
            | "ushort"
            | "using"
            | "virtual"
            | "void"
            | "volatile"
            | "while"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_keywords_and_compiler_generated_names() {
        assert_eq!(identifier("event").rendered, "@event");
        let generated = identifier("<Update>d__12");
        assert_eq!(generated.rendered, "_Update_d__12");
        assert!(generated.changed);
        assert_eq!(without_generic_arity("Dictionary`2"), "Dictionary");
    }
}
