// The basic unit for a command that could be run.

use std::run;
mod helpers;

#[deriving(Clone)]
pub struct Cmd {
    program : ~str,
    argv : ~[~str],
}

impl Cmd {
    // Make a new command. Handles splitting the input into words.
    pub fn new(cmd_name: &str) -> Option<Cmd> {
        let mut argv: ~[~str] = helpers::split_words(cmd_name);
        if argv.len() > 0 {
            let program: ~str = argv.remove(0);
            cmd_exists(Cmd {
                program : program,
                argv : argv,
            })
        }
        else {
            None
        }
    }
}
// Determine whether a command exists using "which".
pub fn cmd_exists(cmd : Cmd) -> Option<Cmd> {
    let ret = run::process_output("which", [cmd.program.to_owned()]);
    if (ret.expect("exit code error.").status.success()) {
        Some(cmd)
    }
    else {
        None
    }
}

