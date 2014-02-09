//
// gash.rs
//
// Starting code for PS2
// Running on Rust 0.9
//
// University of Virginia - cs4414 Spring 2014
// Weilin Xu, David Evans
// Version 0.4
//

extern mod extra;

use std::{io, run, os};
use std::io::buffered::BufferedReader;
use std::path::posix::Path;
use std::io::stdin;
use extra::getopts;

struct Shell {
    cmd_prompt: ~str,
    history: ~[~str],
}

struct Command {
    command: ~str,
    args: ~[~str],
    background: bool,
}

impl Command {
    // Really ugly argument parser.
    fn new(cmd_line: &str) -> Command {
        let mut argv: ~[~str] =
            cmd_line.split(' ').filter_map(|x| if x != "" 
                { 
                    Some(x.to_owned()) 
                }
                else { 
                    None 
                }).to_owned_vec();
        let mut cmd = Command {
            command: "".to_owned(),
            args: argv.to_owned(),
            background: false
        };
        if argv.len() > 0 {
            let program: ~str = argv.remove(0);
            cmd = Command {
                command: program.to_owned(),
                args: argv.to_owned(),
                background: false
            };
            if argv.len() > 0 {
                let last = argv.last();
                if *last == ~"&" {
                    cmd.background = true;
                }
            }
        }
        cmd
    }

    fn run(&mut self) {
        if self.cmd_exists() {
            run::process_status(self.command, self.args);
        } else {
            println!("{:s}: command not found", self.command);
        }
    }
    
    fn cmd_exists(&mut self) -> bool {
        let ret = run::process_output("which", [self.command.to_owned()]);
        return ret.expect("exit code error.").status.success();
    }
}

impl Shell {
    fn new(prompt_str: &str) -> Shell {
        Shell {
            cmd_prompt: prompt_str.to_owned(),
            history: ~[]
        }
    }
    
    fn run(&mut self) {
        let mut stdin = BufferedReader::new(stdin());
        
        loop {
            print(self.cmd_prompt);
            io::stdio::flush();
            
            let line = stdin.read_line().unwrap();
            let cmd_line = line.trim().to_owned();
            let program = cmd_line.splitn(' ', 1).nth(0).expect("no program");
            
            // Push run commands onto the history stack
            match program {
                "" => {}
                _  => { self.push_hist(cmd_line) } 
            }
            match program {
                ""        =>  { continue; }
                "exit"    =>  { return; }
                "history" =>  { self.show_hist(); }
                "cd"      =>  { self.chdir(cmd_line); }
                _         =>  { self.run_cmdline(cmd_line); }
            }
        }
    }

    fn push_hist(&mut self, cmd_line: &str) {
        &self.history.push(cmd_line.to_owned());
    }

    fn show_hist(&mut self) {
        println!("{:s}", self.get_hist());
    }
    
    fn get_hist(&mut self) -> ~str {
        let mut hist = ~"";
        for i in self.history.iter() {
            let s = i.to_owned();
            match hist {
                ~"" => { hist = s; }
                _   => { hist = hist + "\n" + s; }
            }
        }
        return hist
    }

    fn chdir(&mut self, cmd_line: &str) {
        let mut argv: ~[~str] =
            cmd_line.split(' ').filter_map(|x| if x != "" 
                { 
                    Some(x.to_owned()) 
                }
                else { 
                    None 
                }).to_owned_vec();
        if argv.len() > 0 {
            argv.remove(0);
            let path = Path::new(argv[0]);
            os::change_dir(&path);
        }
    }
    
    fn run_cmdline(&mut self, cmd_line: &str) {
        let mut cmd = Command::new(cmd_line);
        cmd.run();
    }
    
}

fn get_cmdline_from_args() -> Option<~str> {
    /* Begin processing program arguments and initiate the parameters. */
    let args = os::args();
    
    let opts = ~[
        getopts::optopt("c")
    ];
    
    let matches = match getopts::getopts(args.tail(), opts) {
        Ok(m) => { m }
        Err(f) => { fail!(f.to_err_msg()) }
    };
    
    if matches.opt_present("c") {
        let cmd_str = match matches.opt_str("c") {
                                                Some(cmd_str) => {cmd_str.to_owned()}, 
                                                None => {~""}
                                              };
        return Some(cmd_str);
    } else {
        return None;
    }
}

fn main() {
    let opt_cmd_line = get_cmdline_from_args();
    
    match opt_cmd_line {
        Some(cmd_line) => Shell::new("").run_cmdline(cmd_line),
        None           => Shell::new("gash > ").run()
    }
}
