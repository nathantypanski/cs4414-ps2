extern mod extra;
use extra::getopts;

use std::io::buffered::BufferedReader;
use std::io::{Open, Truncate, Read, Write};
use std::io::fs::File;
use std::os;
use std::run::Process;

#[allow(dead_code)]
// Ugly, ugly, word-splitting parser for individual commands. Gets the job
// done, though.
pub fn split_words(words : &str) -> ~[~str] {
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
    splits.iter().map(|x| x.replace("\\n", "\n")).collect()
}

#[allow(dead_code)]
pub fn input_redirect(mut process: ~Process, path: &Path) -> ~Process {
    let file = File::open_mode(path, Open, Read)
        .expect(format!("ERR: Failed opening input file"));
    let file_buffer = &mut BufferedReader::new(file);
    process.input().write(file_buffer.read_to_end());
    process
}

#[allow(dead_code)]
pub fn write_output_to_file(output : ~[u8],
                        path : &Path) {
    let mut file = File::open_mode(path,
                                    Truncate,  
                                    Write)
        .expect(format!("ERR: Failed opening output"));
    file.write(output);
}

#[allow(dead_code)]
pub fn output_redirect(mut process : ~Process, path : &Path) -> ~Process {
    let output = process.finish_with_output();
    if output.status.success() {
        write_output_to_file(output.output, path);
    }
    process
}

#[allow(dead_code)]
pub fn pipe_redirect(mut left: ~Process, mut right: ~Process) -> ~Process {
    right.input().write(left.finish_with_output().output);
    left.close_outputs();
    right
}

// Begin processing program arguments and initiate the parameters.
#[allow(dead_code)]
pub fn get_cmdline_from_args() -> Option<~str> {
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

// Describes a computation that could fail.
// If v is Some(...), then f is called on v and the result is returned.
// Otherwise, None is returned.
#[allow(dead_code)]
pub fn maybe<A, B>(v : Option<A>, f : |A| -> Option<B>) -> Option<B> {
    match v {
        Some(v) => f(v),
        None    => None,
    }
}

// Same as maybe(v, f), but for &v.
#[allow(dead_code)]
pub fn borrowed_maybe<A, B>(v : &Option<A>, f : |&A| -> Option<B>) -> Option<B> {
    match *v {
        Some(ref v) => f(v),
        None    => None,
    }
}
