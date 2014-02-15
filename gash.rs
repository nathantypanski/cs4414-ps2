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

use std::{run, os};
use std::io::buffered::BufferedReader;
use std::path::posix::Path;
use std::option::Option;
use std::io::{stdin, stdio, IoError, io_error, File, Truncate, Write};
use std::run::Process;
use std::io::process::ProcessExit;
use std::run::ProcessOptions;
use std::comm::Port;
use extra::getopts;
use std::io::signal::{Listener, Interrupt, Signum};

struct Cmd {
    program : ~str,
    argv : ~[~str],
}

impl Cmd {
    fn new(cmd_name: &str) -> Option<Cmd> {
        let mut argv: ~[~str] = split_words(cmd_name);
        if argv.len() > 0 {
            let program: ~str = argv.remove(0);
            let argv : ~[~str] = argv;
            Some(Cmd {
                program : program,
                argv : argv,
            })
        }
        else {
            None
        }
    }
}

trait Command {
    fn run(&mut self) -> Option<Process>;
}

struct CmdProcess {
    command     : ~str,
    args        : ~[~str],
    stdin       : Option<i32>,
    stdout      : Option<i32>,
}
impl CmdProcess {
    fn new(cmd : Cmd,
           stdin : Option<i32>,
           stdout : Option<i32>) 
        -> Option<CmdProcess> 
    {
        if (cmd_exists(&cmd)) {
            Some(CmdProcess {
                command     : cmd.program.to_owned(),
                args        : cmd.argv.to_owned(),
                stdin       : stdin,
                stdout      : stdout,
            })
        }
        else { None }
    }

    /*
    fn set_stdin(&mut self, stdin : Option<i32>) {
        self.stdin = stdin;
    }

    fn set_stdout(&mut self, stdout : Option<i32>) {
        self.stdout = stdout;
    }
    */
}
impl Command for CmdProcess {
    fn run(&mut self) -> Option<Process> {
        let command = self.command.to_owned();
        let args = self.args.to_owned();
        let options = ProcessOptions {
            env    : None,
            dir    : None,
            in_fd  : self.stdin,
            out_fd : self.stdout,
            err_fd : None,
        };
        Process::new(command, args, options)
    }
}

struct BackgroundProcess {
    command      : ~str,
    args         : ~[~str],
    exit_port    : Option<Port<ProcessExit>>,
    kill_chan    : Option<Chan<int>>,
}
impl BackgroundProcess {
    fn new(cmd : Cmd) -> Option<BackgroundProcess> {
        if (cmd_exists(&cmd)) {
            Some(BackgroundProcess {
                command: cmd.program.to_owned(),
                args: cmd.argv.to_owned(),
                exit_port: None,
                kill_chan: None,
            })
        }
        else { None }
    }
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
                println("Background signal handler started");
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
                _ => {
                }}
                chan.try_send_deferred(process.finish());
            }
            None => {
            }}
        });
        self.exit_port = Some(port);
        self.kill_chan = Some(killchan);
    }
}

struct Shell {
    cmd_prompt : ~str,
    history    : ~[~str],
    processes  : ~[~BackgroundProcess],
    broken : bool,
}

impl Shell {
    fn new(prompt_str: &str) -> Shell {
        Shell {
            cmd_prompt: prompt_str.to_owned(),
            history: ~[],
            processes: ~[],
            broken: false,
        }
    }

    fn run(&mut self) {
        let mut stdin = BufferedReader::new(stdin());
        let mut interrupt = Listener::new();
        interrupt.register(Interrupt);
        let (port, chan) : (Port<bool>, Chan<bool>) 
                = Chan::new();
        self.rupt(interrupt.port, chan);
        loop {
            let port = &port;
            print(self.cmd_prompt);
            stdio::flush();

            match port.try_recv() {
            Some(val) => {
                if val {
                    println("Got dead signal");
                    break;
                }
            }
            None => {
            }}
            
            let line = stdin.read_line().unwrap();
            let cmd_line = line.trim().to_owned();
            let program = cmd_line.splitn(' ', 1).nth(0).expect("no program");
            self.kill_dead();
            match program {
            "" => {}
            _  => { 
                self.push_hist(cmd_line) 
            }}

            match program {
            "" =>  {
                continue; 
            }
            "exit" =>  { 
                self.kill_all();
                return; 
            }
            "history" => {
                self.show_hist();
            }
            "cd" =>  {
                self.chdir(cmd_line); 
            }
            _ => { 
                self.run_cmdline(cmd_line);
            }}
        }
    }

    fn rupt(&mut self, port: Port<Signum>, chan : Chan<bool>) {
        do spawn {
            loop {
                match port.recv_opt() {
                    Some(recv) => {
                        match recv {
                        Interrupt => { 
                            println("Got Interrupt'ed");
                            match chan.try_send_deferred(true) {
                            true => {
                                break;
                            }
                            false => {
                                continue;
                            }}
                        }
                        _ => {
                        }}
                    }
                    None => {
                    }}
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
            None => {
            }}
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
        let mut argv: ~[~str] = split_words(cmd_line);
        if argv.len() > 0 {
            argv.remove(0);
            let path = Path::new(argv[0]);
            os::change_dir(&path);
        }
    }
    
    fn run_cmdline(&mut self, cmd_line: &str) {
        if cmd_line.contains_char('>') {
            parse_r_redirect(cmd_line);
        }
        else if cmd_line.contains_char('<') {
            parse_l_redirect(cmd_line);
        }
        else if cmd_line.contains_char('|') { 
            self.parse_pipeline(cmd_line);
        }
        else {
            self.parse_process(cmd_line);
        }
    }

    fn parse_process(&mut self, cmd_line : &str) -> Option<Process>{
        match Cmd::new(cmd_line) {
        Some(cmd) => {
            if (cmd.argv.len() > 0 && cmd.argv.last() == &~"&") {
                let mut argv = cmd.argv.to_owned();
                argv.pop();
                self.make_bg_process(Cmd{
                    program:cmd.program,
                    argv:argv});
                None
            }
            else {
                make_process(cmd, Some(0), Some(1))
            }
        }
        None => {
            None
        }}
    }

    fn parse_pipeline(&mut self, cmd_line : &str) -> Option<~run::Process> {
        let pair : ~[&str] = cmd_line.rsplitn('|', 1).collect();
        match Cmd::new(pair[0].trim()) {
        Some(cmd) => {
            if pair.len() > 1 {
                let two = self.parse_pipeline(pair[1].trim());
                self.pipe_input(cmd, two)
            }
            else {
                match make_process(cmd, None, None) {
                Some(process) => Some(~process),
                None => None 
                }
            }
        }
        None => {
            None
        }}
    }

    fn pipe_input(&mut self,
                  cmd : Cmd,
                  two : Option<~run::Process>) -> Option<~run::Process> {
        match make_process(cmd, None, Some(1)) {
        Some(mut process) => {
            match two {
            Some(mut p2) => {
                let output = p2.finish_with_output();
                if output.status.success() {
                    process.input().write(output.output);
                    process.close_input();
                    process.finish();
                }
                Some(~process)
            }
            None => {
                println("No two remain.");
                Some(~process)
            }}
        }
        None => { 
            None
        }}
    }

    fn make_bg_process(&mut self, cmd : Cmd) {
        match BackgroundProcess::new(cmd) {
        Some(mut process) => {
            process.run();
            self.processes.push(~process);
        }    
        None          => { 
        }}
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

fn cmd_exists(command : &Cmd) -> bool {
    let ret = run::process_output("which", [command.program.to_owned()]);
    return ret.expect("exit code error.").status.success();
}

fn is_dead(exit_port : &Option<Port<ProcessExit>>) -> bool {
    let mut dead = false;
    match *exit_port {
    Some(ref p) => {
        println("DEBUG: doing shutdown");
        match p.try_recv() {
        Some (exitstatus) => {
            dead = exitstatus.success();
        },
        None => {
        }}
    }
    _ => {
    }}
    dead
}

fn split_words(word : &str) -> ~[~str] {
    word.split(' ').filter_map(
        |x| if x != "" { Some(x.to_owned()) } else { None }
        ).to_owned_vec()
}

fn make_process(cmd : Cmd,
                stdin: Option<i32>,
                stdout: Option<i32>) -> Option<run::Process> {
    match CmdProcess::new(cmd, stdin, stdout) {
    Some(mut cmdprocess) => { 
        cmdprocess.run()
    }    
    None => { 
        None 
    }}
}

fn parse_l_redirect(cmd_line : &str) {
    let pair : ~[&str] = cmd_line.rsplit('<').collect();
    let filename = pair[0].trim();
    match Cmd::new(pair[1].trim()) {
    Some(cmd) => {
        match make_process(cmd, None, Some(1)) {
        Some(mut process) => {
            match File::open_mode(&Path::new(filename),
                                    std::io::Append,
                                    std::io::ReadWrite) {
            Some(file) => {
                let proc_input = process.input();
                let mut file_buffer = BufferedReader::new(file);
                proc_input.write(file_buffer.read_to_end());
            }
            None => {
            }}
        }
        None => { 
        }}
    }
    None => {
    }}
}

fn write_output_to_file(output : std::run::ProcessOutput,
                        filename : &str) {
    if output.status.success() {
        match File::open_mode(&Path::new(filename),
                                std::io::Truncate, 
                                std::io::Write) 
        {
            Some(mut file) => {
                file.write(output.output);
            }
            None =>{
                println!("Opening {:s} failed!", filename);
            }
        }
    }
    else {
        println!("{:?}", output.error);
    }
}

fn parse_r_redirect(cmd_line : &str) {
    let pair : ~[&str] = cmd_line.rsplit('>').collect();
    let file = pair[0].trim();
    //let command = pair[1].trim();
    match Cmd::new(pair[1].trim()) {
    Some(cmd) => {
    //let mut argv: ~[~str] = split_words(command);
        match make_process(cmd, None, None) {
        Some(mut process) => {
            write_output_to_file(
                process.finish_with_output(),
                file);
        }
        None => { 
        }}
    }
    None => {
    }}
}


fn main() {
    let opt_cmd_line = get_cmdline_from_args();
    
    match opt_cmd_line {
        Some(cmd_line) => {
            let mut shell = Shell::new("");
            shell.run_cmdline(cmd_line);
        }
        None           => {
            Shell::new("gash > ").run();
        }
    }

}
