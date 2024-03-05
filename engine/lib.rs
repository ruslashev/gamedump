#![allow(
    clippy::missing_const_for_fn,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::missing_safety_doc,
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::option_if_let_else,
    clippy::similar_names,
    clippy::struct_excessive_bools,
    clippy::too_many_arguments,
    clippy::uninlined_format_args,
    clippy::wildcard_imports
)]
#![allow(dead_code)]
#![feature(iter_intersperse, panic_info_message, int_roundings)]

pub mod logger;
pub mod main_loop;
pub mod panic;
pub mod window;
pub mod world;

mod camera;
mod image;
mod input;
mod rand;
mod renderer;
mod utils;

#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;
