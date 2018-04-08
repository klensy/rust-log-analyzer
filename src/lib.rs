#![deny(unused_must_use)]
#![cfg_attr(feature = "cargo-clippy", allow(needless_range_loop, useless_let_if_seq))]

extern crate aho_corasick;
extern crate atomicwrites;
extern crate bincode;
#[macro_use]
extern crate failure;
extern crate fnv;
#[macro_use]
extern crate hyper;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
extern crate regex;
extern crate reqwest;
extern crate serde;
#[macro_use]
extern crate serde_derive;

pub mod extract;
pub mod index;
pub mod sanitize;
pub mod travis;

pub use self::index::Index;

pub static USER_AGENT: &str = concat!("rust-ops/rust-log-analyzer ", env!("CARGO_PKG_VERSION"));

pub type Result<T> = std::result::Result<T, failure::Error>;
