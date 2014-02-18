#[ path="helpers.rs"] mod helpers;

// The basic unit for a command that could be run.
pub mod cmd {
    use helpers::helpers;
    use std::run;
    #[deriving(Clone)]
    pub struct Cmd {
        program : ~str,
        argv : ~[~str],
    }

    impl Cmd {
        // Make a new command. Handles splitting the input into words.
        #[allow(dead_code)]
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
    #[allow(dead_code)]
    pub fn cmd_exists(cmd : Cmd) -> Option<Cmd> {
        let ret = run::process_output("which", [cmd.program.to_owned()]);
        if (ret.expect("exit code error.").status.success()) {
            Some(cmd)
        }
        else {
            None
        }
    }
}

pub mod fg{
    use super::cmd::Cmd;
    use std::run::Process;
    use std::run::ProcessOptions;
    // A foreground process is a command, arguments, and file descriptors for its
    // input and output.
    #[allow(dead_code)]
    pub struct FgProcess {
        command     : ~str,
        args        : ~[~str],
        stdin       : Option<i32>,
        stdout      : Option<i32>,
    }
    impl FgProcess {
        #[allow(dead_code)]
        pub fn new(cmd : Cmd,
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
        
        #[allow(dead_code)]
        pub fn run(&mut self) -> Process {
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
}

// Background processes are handled differently, but not *that* differently:
// we need to keep track of an ProcessExit port for determining whether a
// process has finished running, as well as a pid for the process (for killing
// it when the shell terminates).
pub mod bg {
    use super::cmd::Cmd;
    use std::run::Process;
    use std::run::ProcessOptions;
    use std::io::process::ProcessExit;
    use std::libc::types::os::arch::posix88::pid_t;
    #[allow(dead_code)]
    pub struct BgProcess {
        command      : ~str,
        args         : ~[~str],
        exit_port    : Option<Port<ProcessExit>>,
        pid          : Option<i32>,
        stdin       : Option<i32>,
        stdout      : Option<i32>,
    }
    impl BgProcess {
        #[allow(dead_code)]
        pub fn new(cmd : Cmd) -> BgProcess {
            BgProcess {
                command: cmd.program.to_owned(),
                args: cmd.argv.to_owned(),
                exit_port: None,
                pid: None,
                stdin: None,
                stdout: None,
            }
        }

        #[allow(dead_code)]
        pub fn run(&mut self) -> Option<pid_t> {
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
}
