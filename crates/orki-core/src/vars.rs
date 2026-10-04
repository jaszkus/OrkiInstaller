use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Bool(bool),
    Int(i64),
    Str(Arc<str>),
    List(Arc<[Value]>),
}

impl Value {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }
}

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Self::Str(Arc::from(v))
    }
}

impl From<String> for Value {
    fn from(v: String) -> Self {
        Self::Str(Arc::from(v.as_str()))
    }
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Self::Bool(v)
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Self::Int(v)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Vars {
    map: BTreeMap<String, Value>,
}

impl Vars {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, key: impl Into<String>, value: impl Into<Value>) {
        self.map.insert(key.into(), value.into());
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.map.get(key)
    }

    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.map.get(key).and_then(Value::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::{Value, Vars};

    #[test]
    fn set_and_get() {
        let mut vars = Vars::new();
        vars.set("app.name", "My App");
        vars.set("flag", true);
        assert_eq!(vars.get_str("app.name"), Some("My App"));
        assert_eq!(vars.get("flag").and_then(Value::as_bool), Some(true));
        assert_eq!(vars.get_str("missing"), None);
    }
}
