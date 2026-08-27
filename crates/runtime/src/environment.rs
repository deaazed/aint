use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::value::Value;

/// A lexical scope: a table of bindings plus an optional parent to fall
/// through to. Shared (`Rc<RefCell<_>>`) since a function call needs to
/// both own its local scope and read from an enclosing one.
///
/// This is the only type in the runtime holding an `Rc` to another
/// instance of itself, which makes it the one place reference counting
/// could leak via a cycle. It can't, today: `parent` only ever points
/// upward (nothing holds a reference back down from parent to child),
/// and no [`crate::Value`] variant carries an `Environment` reference
/// — `Function` holds an AST fragment (`body: Block`), not a captured
/// scope, because milestone 04 parents every call frame to globals
/// rather than modeling real closures. That keeps this graph a tree,
/// not just in shape but by there being nothing available to close the
/// loop with. If real closures are ever added — a function value
/// capturing *this* environment instead of just globals — that
/// argument stops holding and needs to be re-checked; see
/// `docs/milestones/21-memory-model/SPEC.md`.
#[derive(Debug, Default)]
pub struct Environment {
    values: HashMap<String, Value>,
    parent: Option<Rc<RefCell<Environment>>>,
}

impl Environment {
    pub fn new() -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self::default()))
    }

    pub fn child(parent: &Rc<RefCell<Environment>>) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            values: HashMap::new(),
            parent: Some(Rc::clone(parent)),
        }))
    }

    pub fn define(&mut self, name: impl Into<String>, value: Value) {
        self.values.insert(name.into(), value);
    }

    pub fn get(&self, name: &str) -> Option<Value> {
        if let Some(value) = self.values.get(name) {
            return Some(value.clone());
        }
        self.parent
            .as_ref()
            .and_then(|parent| parent.borrow().get(name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_falls_through_to_parent() {
        let parent = Environment::new();
        parent.borrow_mut().define("x", Value::Int(1));

        let child = Environment::child(&parent);
        assert_eq!(child.borrow().get("x"), Some(Value::Int(1)));
    }

    #[test]
    fn child_definitions_do_not_leak_into_parent() {
        let parent = Environment::new();
        let child = Environment::child(&parent);
        child.borrow_mut().define("y", Value::Int(2));

        assert_eq!(parent.borrow().get("y"), None);
        assert_eq!(child.borrow().get("y"), Some(Value::Int(2)));
    }

    #[test]
    fn child_can_shadow_parent() {
        let parent = Environment::new();
        parent.borrow_mut().define("x", Value::Int(1));

        let child = Environment::child(&parent);
        child.borrow_mut().define("x", Value::Int(2));

        assert_eq!(child.borrow().get("x"), Some(Value::Int(2)));
        assert_eq!(parent.borrow().get("x"), Some(Value::Int(1)));
    }

    #[test]
    fn undefined_name_is_none() {
        let env = Environment::new();
        assert_eq!(env.borrow().get("missing"), None);
    }
}
