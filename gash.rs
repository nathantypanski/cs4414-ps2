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
use std::io::{stdin, stdout, stdio, File, Truncate};
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

struct PathType {
    path: Path,
    mode: FilePermission,
}

impl PathType{
    fn new(path: ~str, mode: FilePermission) -> PathType{
        let path = Path::new(path);
        PathType{
            path: path,
            mode: mode,
        }
    }
}

impl Clone for PathType {
    fn clone(&self) -> PathType {
        PathType{
            path: self.path.clone(),
            mode: self.mode,
        }
    }
}

enum FilePermission {
    Read,
    Write,
}

#[deriving(Clone)]
struct LineElem {
    cmd: ~str,
    pipe: Option<~LineElem>,
    file: Option<PathType>,
    last : bool,
}
impl LineElem {
    fn new(cmd: ~str) -> ~LineElem {
        ~LineElem {
            cmd: cmd.to_owned(),
            pipe: None,
            file: None,
            last: true,
        }
    }

    fn set_path(&self, path: PathType) -> ~LineElem {
        let this_pipe = self.pipe.clone();
        match this_pipe {
            Some(elem) => {
                ~LineElem {
                    cmd: self.cmd.to_owned(), 
                    pipe: Some(elem.set_path(path)),
                    file: self.file.clone(),
                    last: self.last,
                }
            }
            None => {
                ~LineElem {
                    cmd: self.cmd.to_owned(), 
                    pipe: self.pipe.clone(),
                    file: Some(path),
                    last: self.last,
                }
            }
        }
    }

    fn set_pipe(&self, pipe: ~LineElem) -> ~LineElem {
        let this_pipe = self.pipe.clone();
        match this_pipe {
            Some(elem) => {
                ~LineElem {
                    cmd: self.cmd.to_owned(), 
                    pipe: Some(elem.set_pipe(pipe)),
                    file: self.file.clone(),
                    last: false,
                }
            }
            None => {
                ~LineElem {
                    cmd: self.cmd.to_owned(), 
                    pipe: Some(pipe), 
                    file: self.file.clone(),
                    last: false,
                }
            }
        }
    }
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
            println!("DEBUG:\n\tprogram: {:s}\nargv: {:?}", program, argv);
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
        -> FgProcess
    {
        FgProcess {
            command     : cmd.program.to_owned(),
            args        : cmd.argv.to_owned(),
            stdin       : stdin,
            stdout      : stdout,
        }
    }
    
    fn run(&mut self) -> Process {
        let command = self.command.to_owned();
        let args = self.args.to_owned();
        let options = ProcessOptions {
            env    : None,
            dir    : None,
            in_fd  : self.stdin,
            out_fd : self.stdout,
            err_fd : None,
        };
        Process::new(command, args, options).expect("Couldn't run!")
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
    fn new(cmd : Cmd) -> BgProcess {
        BgProcess {
            command: cmd.program.to_owned(),
            args: cmd.argv.to_owned(),
            exit_port: None,
            pid: None,
            stdin: None,
            stdout: None,
        }
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
    breakchars : ~[char],
    broken : bool,
}

impl Shell {
    fn new(prompt_str: &str) -> Shell {
        Shell {
            cmd_prompt: prompt_str.to_owned(),
            history: ~[],
            processes: ~[],
            breakchars: ~['>', '<', '|'],
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

    fn lex(&mut self, cmd_line: &str) -> ~[~str] {
        let mut slices : ~[~str] = ~[];
        let mut last = 0;
        let mut current = 0;
        for i in range(0, cmd_line.len()) {
            if self.breakchars.contains(&cmd_line.char_at(i)) && i != 0 {
                if last != 0 {
                    slices.push(cmd_line.slice(last+1, current).trim().to_owned());
                }
                else {
                    slices.push(cmd_line.slice_to(current).trim().to_owned());
                }
                slices.push(cmd_line.slice(i, i+1).to_owned());
                last = i;
            }
            current += 1;
        }
        if last == 0 {
            slices.push(cmd_line.trim().to_owned());
        }
        else {
            slices.push(cmd_line.slice_from(last+1).trim().to_owned());
        }
        println!("DEBUG: {:?}", slices);
        slices
    }

    fn parse(&mut self, words: ~[~str]) -> ~LineElem {
        let mut slices : ~[~LineElem] = ~[];
        let mut in_file = false;
        let mut out_file = false;
        let mut pipe = false;
        for i in range(0, words.len()) {
            if words[i].len() == 1 
                    && self.breakchars.contains(&words[i].char_at(0)) {
                if words[i] == ~">" {
                    out_file = true;
                }
                if words[i] == ~"<" {
                    in_file = true;
                }
                if words[i] == ~"|" {
                    pipe = true;
                }
            }
            else {
                if out_file {
                    let cmd = slices.pop();
                    slices.push(cmd.set_path(PathType::new(words[i].to_owned(), Write)));
                    out_file = false;
                }
                else if in_file {
                    let cmd = slices.pop();
                    slices.push(cmd.set_path(PathType::new(words[i].to_owned(), Read)));
                    in_file = false;
                }
                else if pipe {
                    let cmd = slices.pop();
                    slices.push(cmd.set_pipe(LineElem::new(words[i].to_owned())));
                    pipe = false;
                }
                else {
                    slices.push(LineElem::new(words[i].to_owned()));
                }
            }
        }
        println!("DEBUG: {:?}", slices[0]);
        slices[0]
    }

    fn _run(&mut self, elem : ~LineElem) -> Option<~Process> {
        if elem.last && elem.file.is_none() {
            self.parse_process(elem.cmd, None, Some(STDOUT_FILENO))
        }
        else {
            match elem.clone().pipe {
                Some(pipe_elem) => {
                    let left = self.parse_process(elem.cmd, None, None).expect("Couldn't spawn!");

                    let right = self._run(pipe_elem).expect("Broken pipe"); 
                    Some(pipe_redirect(left, right))
                }
                None => {
                    match elem.clone().file {
                        Some(file) => {
                            match file.mode {
                                Read => {
                                    let mut process : ~Process;
                                    if elem.last {
                                        process = self.parse_process(elem.cmd, None, Some(STDOUT_FILENO)).expect("Couldn't spawn!");
                                    } 
                                    else {
                                        process = self.parse_process(elem.cmd, None, None).expect("Couldn't spawn!");
                                    }
                                    Some(input_redirect(process, &file.path))
                                }
                                Write => {
                                    let process = self.parse_process(elem.cmd, None, None).expect("Couldn't spawn!");
                                    output_redirect(process, &file.path);
                                    None
                                }
                            }
                        }
                        None => {
                            if elem.last {
                                self.parse_process(elem.cmd, None, Some(STDOUT_FILENO))
                            } 
                            else {
                                self.parse_process(elem.cmd, None, None)
                            }
                        }
                    }
                }
            }
        }
    }

    // Determine the type of the current block, and send it to the right
    // parsing function.
    fn run_cmdline(&mut self, cmd_line: &str) {
        let lex = self.lex(cmd_line);
        let parse = self.parse(lex);
        if parse.pipe.is_none() && parse.file.is_none() {
            self.parse_process(parse.cmd, Some(STDIN_FILENO), Some(STDOUT_FILENO));
        }
        else {
            self._run(parse);
        }
    }

    // Parse a new lone process. Background/foreground it appropriately.
    fn parse_process(&mut self,
                     cmd_line : &str,
                     stdin: Option<i32>,
                     stdout:Option<i32>) 
                    -> Option<~Process> {
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
                    Some(~(FgProcess::new(cmd, stdin, stdout).run()))
                }
        })
    }

    // background processes.
    fn make_bg_process(&mut self, cmd : Cmd) {
        let name = cmd.program.to_owned();
        let mut process = BgProcess::new(cmd);
        match process.run() {
            Some(pid) => {
                println!("{:s} {:i}", name, pid);
                self.processes.push(~process);
            }
            None => {
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
fn cmd_exists(cmd : Cmd) -> Option<Cmd> {
    let ret = run::process_output("which", [cmd.program.to_owned()]);
    if (ret.expect("exit code error.").status.success()) {
        Some(cmd)
    }
    else {
        None
    }
}

fn split_words(words : &str) -> ~[~str] {
    let mut splits = ~[];
    let mut lastword = 0;
    let mut quoted = false;
    for i in range(0, words.len()) {
        if !quoted {
            if i == 0 {
                if words.char_at(i) == '"' {
                    quoted = true;
                    lastword = i+1;
                }
            }
            else {
                if words.char_at(i) == '"' && words.char_at(i-1) != '\\' {
                    quoted = true;
                    lastword = i+1;
                }
                else if words.char_at(i) == ' ' && words.char_at(i-1) != '\\' {
                    let word = words.slice(lastword, i).to_owned();
                    if word != ~"" {
                        splits.push(word);
                    }
                    lastword = i+1;
                }
            }
        }
        else {
            if words.char_at(i) == '"' {
                splits.push(words.slice(lastword, i).to_owned());
                lastword = i+1;
                quoted = false;
            }
        }
    }
    if lastword != words.len() {
        splits.push(words.slice_from(lastword).to_owned());
    }
    println!("DEBUG: split_words: {:?}", splits);
    splits
}

fn input_redirect(mut process: ~Process, path: &Path) -> ~Process {
    let file = File::open_mode(path,
                            std::io::Open,
                            std::io::Read)
        .expect(format!("Couldn't open file!"));
    let file_buffer = &mut BufferedReader::new(file);
    write_buffer(file_buffer, process.input());
    process
}

fn write_buffer(input : &mut Reader, output: &mut Writer) {
    output.write(input.read_to_end());
}

fn write_output_to_file(output : ~[u8],
                        path : &Path) {
    let mut file = File::open_mode(path,
                                    std::io::Truncate,  
                                    std::io::Write)
    .expect(format!("Failed to open a file for output!"));
    file.write(output);
}

fn output_redirect(mut process : ~Process, path : &Path) -> ~Process {
    let output = process.finish_with_output();
    if output.status.success() {
        write_output_to_file(output.output, path);
    }
    process
}

fn pipe_redirect(mut left: ~Process, mut right: ~Process) -> ~Process {
    let output = left.finish_with_output();
    if output.status.success() {
        right.input().write(output.output);
    }
    right
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
