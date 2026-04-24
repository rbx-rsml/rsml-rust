#![feature(generic_const_exprs)]
#![feature(iter_intersperse)]

pub(crate) mod string_clip;
pub mod types;
#[macro_use]
mod macros;

pub mod builtins;
pub mod datatype;
pub mod lexer;
pub mod list;
pub mod macro_registry;
pub mod parser;
pub mod range_from_span;

#[cfg(feature = "compiler")]
pub mod compiler;

#[cfg(feature = "typechecker")]
pub mod typechecker;

#[cfg(feature = "compiler")]
pub use compiler::RsmlCompiler;

pub use lexer::RsmlLexer;
pub use parser::RsmlParser;
