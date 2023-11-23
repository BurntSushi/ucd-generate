use std::process;

mod common;
mod run;

pub(crate) use common::*;

fn main() {
    let matches = app::app().get_matches();
    if let Err(err) = run::run(matches) {
        if err.is_broken_pipe() {
            process::exit(0);
        }
        eprintln!("{}", err);
        process::exit(1);
    }
}
