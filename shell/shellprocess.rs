#[allow(dead_code)]
pub mod fg{
    use std::run::Process;
    use std::run::ProcessOptions;
    // A foreground process is a command, arguments, and file descriptors for its
    // input and output.
    
    // Note: std::run is replaced with std::io::process in the latest Rust
    // release.
    pub struct FgProcess {
        command     : ~str,
        args        : ~[~str],
        stdin       : Option<i32>,
        stdout      : Option<i32>,
    }
    impl FgProcess {
        pub fn new(program : ~str, argv: ~[~str],
            stdin : Option<i32>, stdout : Option<i32>) 
            -> FgProcess
        {
            FgProcess {
                command     : program.to_owned(),
                args        : argv.to_owned(),
                stdin       : stdin,
                stdout      : stdout,
            }
        }
        
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

#[allow(dead_code)]
pub mod bg {
    // Background processes are handled differently, but not *that* differently:
    // we need to keep track of an ProcessExit port for determining whether a
    // process has finished running, as well as a pid for the process (for killing
    // it when the shell terminates).

    // Note: std::run is replaced with std::io::process in the latest Rust
    // release.
    use std::run::Process;
    use std::run::ProcessOptions;
    use std::io::process::ProcessExit;
    use std::libc::types::os::arch::posix88::pid_t;
    pub struct BgProcess {
        command      : ~str,
        args         : ~[~str],
        exit_port    : Option<Port<ProcessExit>>,
        pid          : Option<i32>,
        stdin       : Option<i32>,
        stdout      : Option<i32>,
    }
    impl BgProcess {
        pub fn new(program : ~str, argv: ~[~str]) -> BgProcess {
            BgProcess {
                command: program.to_owned(),
                args: argv.to_owned(),
                exit_port: None,
                pid: None,
                stdin: None,
                stdout: None,
            }
        }

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
