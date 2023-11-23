use std::{env, process};

mod common;
mod run;

pub(crate) use common::*;

fn main() {
    if let Err(err) = run::run(&mut env::args_os()) {
        if err.is_broken_pipe() {
            process::exit(0);
        }
        eprintln!("{}", err);
        process::exit(1);
    }
}
