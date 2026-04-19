#![feature(generic_const_exprs)]
#![feature(iter_intersperse)]

pub(crate) mod string_clip;
pub mod types;
#[macro_use]
mod macros;

pub mod builtins;
pub mod compiler;
pub mod datatype;
pub mod lexer;
pub mod list;
pub mod parser;
pub mod range_from_span;
pub mod typechecker;
