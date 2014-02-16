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

use std::{run, os};
use std::io::buffered::BufferedReader;
use std::io::{stdin, stdio, File, Truncate, Write};
use std::io::process::ProcessExit;
use std::io::signal::{Listener, Interrupt};
use std::path::posix::Path;
use std::option::Option;
use std::task::try;
use std::run::Process;
use std::run::ProcessOptions;
use std::comm::Port;

use extra::getopts;

use std::libc::types::os::arch::posix88::pid_t;
use std::libc::consts::os::posix88::{STDOUT_FILENO, STDIN_FILENO};
use std::libc;

extern {
  pub fn kill(pid: pid_t, sig: libc::c_int) -> libc::c_int;
}

// The basic unit for a command that could be run.
#[deriving(Clone)]
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

struct FgProcess {
    command     : ~str,
    args        : ~[~str],
    stdin       : Option<i32>,
    stdout      : Option<i32>,
}
impl FgProcess {
    fn new(cmd : Cmd,
           stdin : Option<i32>,
           stdout : Option<i32>) 
        -> Option<FgProcess> 
    {
        if (cmd_exists(&cmd)) {
            Some(FgProcess {
                command     : cmd.program.to_owned(),
                args        : cmd.argv.to_owned(),
                stdin       : stdin,
                stdout      : stdout,
            })
        }
        else { None }
    }
    
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

// Background processes are handled differently, but not *that* differently:
// we need to keep track of an ProcessExit port for determining whether a
// process has finished running, as well as a pid for the process (for killing
// it when the shell terminates).
struct BgProcess {
    command      : ~str,
    args         : ~[~str],
    exit_port    : Option<Port<ProcessExit>>,
    pid          : Option<i32>,
    stdin       : Option<i32>,
    stdout      : Option<i32>,
}
impl BgProcess {
    fn new(cmd : Cmd) -> Option<BgProcess> {
        if (cmd_exists(&cmd)) {
            Some(BgProcess {
                command: cmd.program.to_owned(),
                args: cmd.argv.to_owned(),
                exit_port: None,
                pid: None,
                stdin: None,
                stdout: None,
            })
        }
        else { None }
    }

    fn run(&mut self) -> Option<pid_t> {
        // Process exit ports; used for checking dead status.
        let (port, chan): (Port<ProcessExit>, Chan<ProcessExit>) = Chan::new();
        // Process ports; these don't leave this function and are used for
        // sending the PID out in the return value.
        let (pidport, pidchan): (Port<Option<pid_t>>, Chan<Option<pid_t>>) 
                            = Chan::new();
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
                    // Send the pid out for the return value
                    pidchan.try_send_deferred(Some(process.get_id()));
                    chan.try_send_deferred(process.finish());
                }
                None => {
                    pidchan.try_send_deferred(None);
                }
            }
        });
        self.exit_port = Some(port);
        self.pid = pidport.recv();
        self.pid
    }
}

struct Shell {
    cmd_prompt : ~str,
    history    : ~[~str],
    processes  : ~[~BgProcess],
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

    fn start(&mut self) {
        // Setup the interrupt handler. Has to happen here, or it won't 
        // retain control over interrupts.
        let mut listener = Listener::new();
        listener.register(Interrupt);
        let port = listener.port;
        spawn(proc() {
            try(proc() {
                loop {
                    match port.recv_opt() {
                        Some(Interrupt) => {
                        }
                        None => {
                            break;
                        }
                        _ => {
                        }
                    }
                }
                return 0
            });
            return ();
        });

        self.display_prompt();
    }

    fn display_prompt(&mut self) {
        // Standard input reader
        let mut stdin = BufferedReader::new(stdin());
        // Show the prompt
        print(self.cmd_prompt);
        stdio::flush();

        let line = stdin.read_line().unwrap();
        let cmd_line = line.trim().to_owned();
        let program = cmd_line.splitn(' ', 1).nth(0).expect("no program");
        self.disown_dead();

        // Push commands onto the history
        match program {
            "" => {}
            _  => { 
                self.push_hist(cmd_line) 
            }
        }

        match program {
            "" =>  {
                self.display_prompt(); 
            }
            "exit" =>  { 
                self.kill_all();
            }
            "history" => {
                self.show_hist();
                self.display_prompt();
            }
            "jobs" => {
                self.jobs();
                self.display_prompt();
            }
            "cd" =>  {
                self.chdir(cmd_line); 
                self.display_prompt();
            }
            _ => { 
                self.run_cmdline(cmd_line);
                self.display_prompt();
            }
        }
    }

    // Nice extra feature: list running jobs.
    fn jobs(&mut self) {
        for cmd in self.processes.iter() {
            print!("{:s}", cmd.command);
            match cmd.pid {
                Some(pid) => {
                    println!(" {:i}", pid);
                }
                None => {
                    println("");
                }
            }
        }
    }

    // Remove dead processes from the list of processes.
    fn disown_dead(&mut self) {
        let mut dead : ~[uint];
        dead = ~[];
        let mut i = 0;
        for cmd in self.processes.iter() {
            borrowed_maybe(&cmd.exit_port, |port| match port.try_recv() {
                Some(_) => {dead.push(i); Some(i)},
                _ => None,
            });
            i += 1;
        }
        for &p in dead.iter() {
            self.processes.remove(p);
        }
    }

    fn kill_all(&mut self) {
        for p in self.processes.iter() {
            match p.pid {
                Some(pid) => {
                    unsafe { 
                        // Kill perhaps isn't the best way to do this, but
                        // without it we have no way of killing our background
                        // processes with only the PID - we'd need the process
                        // object itself, and that can't be moved out of the
                        // spawn() that it's trapped inside.
                        kill(pid, 15); 
                    }
                }
                None => {
                }
            }
        }
    }

    // Push a new command onto history.
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
   
    // Determine the type of the current block, and send it to the right
    // parsing function.
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

    // Parse a new lone process. Background/foreground it appropriately.
    fn parse_process(&mut self, cmd_line : &str) -> Option<Process>{
        maybe(Cmd::new(cmd_line), |cmd| {
            if (cmd.argv.len() > 0 && cmd.argv.last() == &~"&") {
                let mut argv = cmd.argv.to_owned();
                argv.pop();
                self.make_bg_process(Cmd{
                    program:cmd.program,
                    argv:argv});
                None
            }
            else {
                make_process(cmd, Some(STDIN_FILENO), Some(STDOUT_FILENO))
            }
        })
    }

    // Parse a process pipeline, and redirect stdin/stdout appropriately.
    fn parse_pipeline(&mut self, cmd_line : &str) {
        let pipes : ~[Cmd] = cmd_line.split('|')
            .filter_map(Cmd::new)
            .to_owned_vec();
        let mut i = 0;
        let mut stdout = stdio::stdout();
        let x = pipes[0].clone();
        let y = pipes[1].clone();
        match self.pipe_input(x, y) {
            Some(mut p) => {
                let output = p.finish_with_output();
                stdout.write(output.output);
            }
            None => {

            }
        }
    }

    // Redirect the stdout of Process two into the stdin of `cmd`, and return
    // the process created from `cmd`.
    fn pipe_input(&mut self, left : Cmd, right : Cmd) -> Option<Process> {
        match simple_process(left) {
            Some(mut left) => {
                match simple_process(right) {
                    Some(mut right) => {
                        let output = left.finish_with_output();
                        if output.status.success() {
                            right.input().write(output.output);
                            right.close_input();
                        }
                        Some(right)
                    }
                    None => {
                        None
                    }
                }
            }
            None => {
                None
            }
        }
    }

    // Make a new background process, and push it onto our stack of tracked
    // background processes.
    fn make_bg_process(&mut self, cmd : Cmd) {
        let name = cmd.program.to_owned();
        match BgProcess::new(cmd) {
            Some(mut process) => {
                match process.run() {
                    Some(pid) => {
                        println!("{:s} {:i}", name, pid);
                        self.processes.push(~process);
                    }
                    None => {
                    }
                }
            }
            None => { 
            }
        }
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

// Determine whether a command exists using "which".
fn cmd_exists(command : &Cmd) -> bool {
    let ret = run::process_output("which", [command.program.to_owned()]);
    return ret.expect("exit code error.").status.success();
}

fn split_words(word : &str) -> ~[~str] {
    word.split(' ').filter_map(
        |x| if x != "" { Some(x.to_owned()) } else { None }
        ).to_owned_vec()
}

fn simple_process(cmd : Cmd) -> Option<run::Process> {
    maybe(FgProcess::new(cmd, None, None), 
          |mut cmdprocess| cmdprocess.run())
}

fn make_process(cmd : Cmd,
                stdin: Option<i32>,
                stdout: Option<i32>) -> Option<run::Process> {
    maybe(FgProcess::new(cmd, stdin, stdout), 
          |mut cmdprocess| cmdprocess.run())
}

fn parse_l_redirect(cmd_line : &str) {
    let pair : ~[&str] = cmd_line.rsplit('<').collect();
    let filename = pair[0].trim();
    match maybe(Cmd::new(pair[1].trim()), |c| 
                make_process(c, None, Some(STDOUT_FILENO))) {
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
                }
            }
        }
        None => { 
        }
    }
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
    let cmd = Cmd::new(pair[1].trim());
    match maybe(cmd, |c| make_process(c, None, None)) {
        Some(mut process) => {
            write_output_to_file(
                process.finish_with_output(),
                file);
        }
        None => { 
        }
    }
}

// Describes a computation that could fail.
// If v is Some(...), then f is called on v and the result is returned.
// Otherwise, None is returned.
fn maybe<A, B>(v : Option<A>, f : |A| -> Option<B>) -> Option<B> {
    match v {
        Some(v) => f(v),
        None    => None,
    }
}

// Same as maybe(v, f), but for &v.
fn borrowed_maybe<A, B>(v : &Option<A>, f : |&A| -> Option<B>) -> Option<B> {
    match *v {
        Some(ref v) => f(v),
        None    => None,
    }
}

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
