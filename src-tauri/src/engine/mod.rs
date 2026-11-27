//! The PDF engine: everything that reads, writes or shapes PDF content.
//! It runs only on the user's PC and contains no network code (SRS NFR-3.1).

pub mod edit;
pub mod extract;
pub mod fonts;
pub mod layout;
pub mod render;
pub mod script;
pub mod selftest;
pub mod viewer;
