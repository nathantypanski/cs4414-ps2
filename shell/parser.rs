#[ path="helpers.rs"] mod helpers;

pub mod pathtype {
    pub struct PathType {
        path: Path,
        mode: FilePermission,
    }

    impl PathType{
        #[allow(dead_code)]
        pub fn new(path: ~str, mode: FilePermission) -> PathType {
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

    pub enum FilePermission {
        Read,
        Write,
    }
}

#[allow(dead_code)]
pub mod cmd {
    use helpers::helpers;
    use super::pathtype::{PathType, Read, Write};
    use std::run;

    // Represents a parsed element of a pipeline / io redirect.
    #[deriving(Clone)]
    pub struct Cmd {
        program : ~str,
        argv : ~[~str],
        pipe: Option<~Cmd>,
        file: Option<PathType>,
        last : bool,
    }
    impl Cmd {
        pub fn new(cmd_line: &str) -> ~Cmd {
            parse(lex(cmd_line))
        }

        #[allow(dead_code)]
        fn _new(cmd_name: ~str) -> ~Cmd {
            let mut argv: ~[~str] = helpers::split_words(cmd_name);
            let mut program : ~str;
            if argv.len() > 0 {
                program = argv.remove(0);
            }
            else {
                program = ~"";
            }
            ~Cmd {
                program: program,
                argv: argv,
                pipe: None,
                file: None,
                last: true,
            }
        }

        pub fn exists(&mut self) -> bool {
            let ret = run::process_output("which", [self.program.to_owned()]);
            ret.expect("exit code error.").status.success()
        }

        // Set the path at the bottom of the pipe chain. This means e.g. in order
        // to set an input path, it needs to happen *before* any pipes are tied
        // to this Cmd.
        #[allow(dead_code)]
        pub fn set_path(&self, path: PathType) -> ~Cmd {
            let this_pipe = self.pipe.clone();
            match this_pipe {
                Some(elem) => {
                    ~Cmd {
                        program: self.program.to_owned(), 
                        argv: self.argv.to_owned(), 
                        pipe: Some(elem.set_path(path)),
                        file: self.file.clone(),
                        last: self.last,
                    }
                }
                None => {
                    ~Cmd {
                        program: self.program.to_owned(), 
                        argv: self.argv.to_owned(), 
                        pipe: self.pipe.clone(),
                        file: Some(path),
                        last: self.last,
                    }
                }
            }
        }

        // Tie a new pipe to this Cmd. If a pipeline already exists, add
        // the pipe to the bottom of the pipeline.
        #[allow(dead_code)]
        pub fn set_pipe(&self, pipe: ~Cmd) -> ~Cmd {
            let this_pipe = self.pipe.clone();
            match this_pipe {
                Some(elem) => {
                    ~Cmd {
                        program: self.program.to_owned(), 
                        argv: self.argv.to_owned(), 
                        pipe: Some(elem.set_pipe(pipe)),
                        file: self.file.clone(),
                        last: false,
                    }
                }
                None => {
                    ~Cmd {
                        program: self.program.to_owned(), 
                        argv: self.argv.to_owned(), 
                        pipe: Some(pipe), 
                        file: self.file.clone(),
                        last: false,
                    }
                }
            }
        }
        
        #[allow(dead_code)]
        pub fn iter(&self) -> ~Cmd {
        ~self.clone()
        }
    }

    impl Iterator<~Cmd> for Cmd {
        #[allow(dead_code)]
        fn next(&mut self) -> Option<~Cmd> {
            let pipe = self.pipe.clone();
            match self.pipe.clone() {
                Some(pipe) => {
                    self.pipe = pipe.pipe;
                }
                None => { }
            }
            pipe
        }
    }

    // Split the input up into words.
    fn lex(cmd_line: &str) -> ~[~str] {
        let breakchars = ~['>', '<', '|'];
        let mut slices : ~[~str] = ~[];
        let mut last = 0;
        let mut current = 0;
        for i in range(0, cmd_line.len()) {
            // This is a special character.
            if breakchars.contains(&cmd_line.char_at(i)) && i != 0 {
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
    // list of Cmds.
    fn parse(words: ~[~str]) -> ~Cmd {
        let breakchars = ~['>', '<', '|'];
        let mut slices : ~[~Cmd] = ~[];
        let mut in_file = false;
        let mut out_file = false;
        let mut pipe = false;
        for i in range(0, words.len()) {
            if words[i].len() == 1 
                    && breakchars.contains(&words[i].char_at(0)) {
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
                    slices.push(cmd.set_pipe(Cmd::_new(words[i].to_owned())));
                    pipe = false;
                }
                else {
                    slices.push(Cmd::_new(words[i].to_owned()));
                }
            }
        }
        slices[0]
    }
}
