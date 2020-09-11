use std::{collections::HashMap, rc::Rc};

use indexmap::IndexMap;

use crate::{
    value::{error::Level3, Value},
    NyarError, Result,
};

pub use self::{attributes::NyarFunctionAttributes, prototype::NyarFunction};

mod attributes;
mod prototype;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct EffectHandler {
    effects: HashMap<String, String>,
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Argument;
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Statement;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FunctionInstance {
    prototype: Rc<NyarFunction>,
    args: Vec<Value>,
    kvs: IndexMap<String, Value>,
}

impl FunctionInstance {
    pub fn new(f: Rc<NyarFunction>) -> Self {
        Self { prototype: f, args: vec![], kvs: IndexMap::new() }
    }
    pub fn fill_arguments(&mut self, args: Vec<Value>) -> Result<()> {
        self.args.extend(args);
        self.check_valid()?;
        // The non-curried function must fill all parameters at once!
        if !self.is_currying() {
            self.check_ready()?
        }
        Ok(())
    }
    pub fn fill_named_arguments(&mut self, args: HashMap<String, Value>) -> Result<()> {
        match self.allow_override_keywords() {
            Level3::Allow => self.kvs.extend(args),
            Level3::Warning => {
                for (k, v) in args.into_iter() {
                    if self.kvs.contains_key(k.as_str()) {
                        println!("noooop!")
                    }
                    self.kvs.insert(k, v);
                }
            }
            Level3::Deny => {
                for (k, v) in args.into_iter() {
                    if self.kvs.contains_key(k.as_str()) {
                        return Err(NyarError::msg("GG"));
                    }
                    self.kvs.insert(k, v);
                }
            }
        }
        self.check_valid()?;
        // The non-curried function must fill all parameters at once!
        if !self.is_currying() {
            self.check_ready()?
        }
        Ok(())
    }

    pub fn check_valid(&self) -> Result<()> {
        match self.allow_extra_arguments() {
            Level3::Allow => {}
            Level3::Warning => {}
            Level3::Deny => {}
        }
        match self.allow_extra_keywords() {
            Level3::Allow => {}
            Level3::Warning => {}
            Level3::Deny => {}
        }
        Ok(())
    }
    pub fn check_ready(&self) -> Result<()> {
        Ok(())
    }
    pub fn evaluate(&self) -> Result<Value> {
        self.check_ready()?;
        unimplemented!()
    }
}
