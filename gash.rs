// gash - a shell written in Rust
// by Nathan Typanski
//
// Running on Rust 0.9
//
// Course information:
//     University of Virginia - cs4414 Spring 2014
//     Weilin Xu, David Evans
//     Version 0.4
#[ crate_id = "gash" ];
#[ desc = "A shell written in Rust." ];
#[ license = "MIT" ];
#[ warn(non_camel_case_types) ];
extern mod extra;

use shell::shell::Shell;
use helpers::helpers::get_cmdline_from_args;

mod shellprocess;
mod lineelem;
mod functional;
mod parser;
mod helpers;
mod cmd;
mod shell;

fn main() {
    let opt_cmd_line = get_cmdline_from_args();
    
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
