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
use std::option::Option;
use std::io::process;
use std::io::{IoError, io_error};
use std::run::Process;
use std::io::process::ProcessExit;
use std::run::ProcessOptions;
use extra::getopts;

fn cmd_exists(command : &str) -> bool {
    let ret = run::process_output("which", [command.to_owned()]);
    return ret.expect("exit code error.").status.success();
}

fn is_dead(exit_port : &Option<Port<ProcessExit>>) -> bool {
    let mut dead = false;
    match *exit_port {
        Some(ref p) => {
            match p.try_recv() {
                Some (exitstatus) => {
                    if (exitstatus.success()) {
                        dead = true;
                    }
                },
                _ => {}
            }
        },
        _ => {},
    };
    dead
}

trait Command {
    fn run(&mut self);
}

struct CmdProcess {
    command     : ~str,
    args        : ~[~str],
    exit_status : Option<process::ProcessExit>,
}
impl CmdProcess {
    fn new(command: &str, args: ~[~str]) -> Option<CmdProcess> {
        if (cmd_exists(command)) {
            Some(CmdProcess {
                command: command.to_owned(),
                args: args.to_owned(),
                exit_status: None,
            })
        }
        else { None }
    }
}
impl Command for CmdProcess {
    fn run(&mut self) {
        self.exit_status = run::process_status(self.command, self.args);
    }
}

struct BackgroundProcess {
    command      : ~str,
    args         : ~[~str],
    exit_status  : Option<ProcessExit>,
    exit_port    : Option<Port<ProcessExit>>,
    kill_chan    : Option<Chan<int>>,
}
impl BackgroundProcess {
    fn new(command: &str, args: ~[~str]) -> Option<BackgroundProcess> {
        if (cmd_exists(command)) {
            Some(BackgroundProcess {
                command: command.to_owned(),
                args: args.to_owned(),
                exit_status: None,
                exit_port: None,
                kill_chan: None,
            })
        }
        else { None }
    }
}
impl Command for BackgroundProcess {
    fn run(&mut self) {
        let (port, chan) : (Port<ProcessExit>, Chan<ProcessExit>) 
                = Chan::new();
        let (killport, killchan) : (Port<int>, Chan<int>) = Chan::new();
        let command = self.command.to_owned();
        let args = self.args.to_owned();
        spawn(proc() { 
            let options = ProcessOptions {
                env    : None,
                dir    : None,
                in_fd  : None,
                out_fd : None,
                err_fd : None,
            };
            let maybe_process = Process::new(command, args, options);
            match maybe_process {
                Some(mut process) => {
                    let signal = killport.recv();
                    let mut error = None;
                    match signal {
                        9 => { 
                            io_error::cond.trap(|e: IoError| {
                                error = Some(e);
                                }).inside(|| {
                                    process.force_destroy() 
                                })
                        }
                        15 => {
                            io_error::cond.trap(|e: IoError| {
                                error = Some(e);
                                }).inside(|| {
                                    process.force_destroy() 
                                })
                        }
                        _ => {}
                    }
                    chan.try_send_deferred(process.finish());
                }
                None => {
                }
            }
        });
        self.exit_port = Some(port);
        self.kill_chan = Some(killchan);
    }
}

struct Shell {
    cmd_prompt : ~str,
    history    : ~[~str],
    processes  : ~[~BackgroundProcess],
}
impl Shell {
    fn new(prompt_str: &str) -> Shell {
        Shell {
            cmd_prompt: prompt_str.to_owned(),
            history: ~[],
            processes: ~[],
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
            self.kill_dead();
            match program {
                "" => {}
                _  => { self.push_hist(cmd_line) } 
            }
            match program {
                ""        =>  { continue; }
                "exit"    =>  { 
                    self.kill_all();
                    return; 
                }
                "history" =>  { self.show_hist(); }
                "cd"      =>  { self.chdir(cmd_line); }
                _         =>  { self.run_cmdline(cmd_line); }
            }
        }
    }

    fn kill_dead(&mut self) {
        let mut dead : ~[uint];
        dead = ~[];
        let mut i = 0;
        for cmd in self.processes.iter() {
            if is_dead(&cmd.exit_port) {
                dead.push(i);
            }
            i += 1;
        }
        for &p in dead.iter() {
            self.processes.remove(p);
        }
    }


    fn kill_all(&mut self) {
        for p in self.processes.iter() {
            match p.kill_chan {
                Some(ref kill_channel) => {
                    kill_channel.try_send_deferred(15);
                }
                None => {}
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
        let argv: ~[~str] =
            cmd_line.split(' ').filter_map(|x| if x != "" 
                { 
                    Some(x.to_owned()) 
                }
                else { 
                    None 
                }).to_owned_vec();
        if argv.len() > 0 {
            self.parse_process(argv);
        }
    }

    fn parse_process(&mut self, mut argv : ~[~str]) {
        let program: ~str = argv.remove(0);
        if argv.len() > 0 {
            let last = argv.last().to_owned();
            if last == ~"&" {
                argv.pop();
                self.make_bg_process(program, argv);
            }
            else {
                self.make_fg_process(program, argv);
            }
        }
        else {
            self.make_fg_process(program, argv);
        }
    }
    
    fn make_fg_process(&mut self, program : ~str, argv : ~[~str]) {
        match CmdProcess::new(program, argv) {
            Some(mut process) => { process.run(); }    
            None              => { }
        }
    }

    fn make_bg_process(&mut self, program : ~str, argv : ~[~str]) {
        match BackgroundProcess::new(program, argv) {
            Some(process) => {self.add_process(~process);}    
            None          => { }
        }
    }

    fn add_process(&mut self, mut process : ~BackgroundProcess) {
        &process.run();
        self.processes.push(process);
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
                None          => {~""}
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
