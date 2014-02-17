use std::run::Process;
use std::os;
use std::io::{stdin, stdout, stdio};
use std::io::buffered::BufferedReader;
use std::io::signal::{Listener, Interrupt};
use std::task::try;

use std::libc::consts::os::posix88::{STDOUT_FILENO, STDIN_FILENO};
use std::libc::types::os::arch::posix88::pid_t;
use std::libc;

use helpers::{split_words, input_redirect, output_redirect, pipe_redirect,
              maybe, borrowed_maybe};

use shellprocess::FgProcess;
use shellprocess::BgProcess;
use lineelem::{LineElem, PathType, Read, Write};
use cmd::Cmd;

mod helpers;
mod lineelem;
mod shellprocess;
mod cmd;


extern {
  pub fn kill(pid: pid_t, sig: libc::c_int) -> libc::c_int;
}


pub struct Shell {
    cmd_prompt : ~str,
    history    : ~[~str],
    processes  : ~[~BgProcess],
    breakchars : ~[char],
    broken : bool,
}

impl Shell {
    pub fn new(prompt_str: &str) -> Shell {
        Shell {
            cmd_prompt: prompt_str.to_owned(),
            history: ~[],
            processes: ~[],
            breakchars: ~['>', '<', '|'],
            broken: false,
        }
    }

    // Start the shell with an interrupt handler. Only needed when
    // an interactive shell is used.
    pub fn start(&mut self) {
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

    // Show the prompt. If called by itself, without start(), no interrupt
    // handling will occur.
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
                self.push_hist(cmd_line);
                self.jobs();
                self.display_prompt();
            }
            "cd" =>  {
                self.push_hist(cmd_line);
                self.chdir(cmd_line); 
                self.display_prompt();
            }
            _ => { 
                self.push_hist(cmd_line);
                self.run_cmdline(cmd_line);
                self.display_prompt();
            }
        }
    }

    // Split the input up into words.
    fn lex(&mut self, cmd_line: &str) -> ~[~str] {
        let mut slices : ~[~str] = ~[];
        let mut last = 0;
        let mut current = 0;
        for i in range(0, cmd_line.len()) {
            // This is a special character.
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
        slices
    }

    // Parse the lexxed input into a (recursive linked by .pipe field) 
    // list of LineElems.
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
        slices[0]
    }

    fn _run(&mut self, elem : ~LineElem) -> Option<~Process> {
        if elem.pipe.is_some() {
            let left = self.parse_process(elem.cmd, None, None).expect("Couldn't spawn!");
            Some(elem.iter().fold(left, |left, right| {
                let right = self.pipe_file(right).expect("Couldn't spawn!");
                pipe_redirect(left,right)
            }))
        }
        else {
            self.pipe_file(elem)
        }
    }

    // "Pipe" output to or from a file. If no file is available, just return
    // the created process.
    fn pipe_file(&mut self, elem : ~LineElem) -> Option<~Process> {
        match elem.clone().file {
            Some(file) => {
                match file.mode {
                    Read => {
                        let process = self.to_process(elem).expect("Couldn't spawn!");
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
                self.to_process(elem)
            }
        }
    }

    // Make a process from a LineElem. Sets the output to stdout if the "last"
    // field is true.
    fn to_process(&mut self, elem : ~LineElem) -> Option<~Process> {
            self.parse_process(elem.cmd,
                               None, 
                               if elem.last { Some(STDOUT_FILENO) }
                               else { None })
    }

    // Determine the type of the current block, and send it to the right
    // parsing function.
    pub fn run_cmdline(&mut self, cmd_line: &str) {
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

    // Kill all the background jobs. Used when "exit" is called at the CLI.
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

    // Show the history.
    fn show_hist(&mut self) {
        let mut hist = ~"";
        for i in self.history.iter() {
            let s = i.to_owned();
            match hist {
                ~"" => { hist = s; }
                _   => { hist = hist + "\n" + s; }
            }
        }
        println!("{:s}", hist);
    }
  
    // Change directories.
    fn chdir(&mut self, cmd_line: &str) {
        let mut argv: ~[~str] = split_words(cmd_line);
        if argv.len() > 0 {
            argv.remove(0);
            let path = Path::new(argv[0]);
            os::change_dir(&path);
        }
    }
}
