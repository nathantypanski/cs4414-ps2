//
// gash.rs
// by Nathan Typanski
//
// Starting code for PS2
// Running on Rust 0.9
//
// University of Virginia - cs4414 Spring 2014
// Weilin Xu, David Evans
// Version 0.4
//

extern mod extra;

use shell::{Shell};

mod shellprocess;
mod lineelem;
mod helpers;
mod cmd;
mod shell;

fn main() {
    let opt_cmd_line = helpers::get_cmdline_from_args();
    
    match opt_cmd_line {
        Some(cmd_line) => {
            let mut shell = Shell::new("");
            shell.run_cmdline(cmd_line);
        }
        None => {
            let mut shell = Shell::new("gash > ");
            shell.start();
        }
    }
}
