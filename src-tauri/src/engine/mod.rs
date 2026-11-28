//! The PDF engine: everything that reads, writes or shapes PDF content.
//! It runs only on the user's PC and contains no network code (SRS NFR-3.1).

pub mod bijoy;
pub mod convert;
pub mod edit;
pub mod extract;
pub mod fonts;
pub mod forms;
pub mod images;
pub mod imagetools;
pub mod layout;
pub mod ocr;
pub mod office;
pub mod pages;
pub mod render;
pub mod script;
pub mod selftest;
pub mod signatures;
pub mod textedit;
pub mod viewer;
