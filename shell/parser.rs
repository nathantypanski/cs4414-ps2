mod lineelem;

#[allow(dead_code)]
pub mod parser {
    pub use lineelem::{LineElem, PathType, Read, Write};
    // Split the input up into words.
    pub fn lex(cmd_line: &str) -> ~[~str] {
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
    // list of LineElems.
    pub fn parse(words: ~[~str]) -> ~LineElem {
        let breakchars = ~['>', '<', '|'];
        let mut slices : ~[~LineElem] = ~[];
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
}
