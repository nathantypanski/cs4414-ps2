pub struct PathType {
    path: Path,
    mode: FilePermission,
}

impl PathType{
    #[allow(dead_code)]
    pub fn new(path: ~str, mode: FilePermission) -> PathType{
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

// Represents a parsed element of a pipeline / io redirect.
#[deriving(Clone)]
pub struct LineElem {
    cmd: ~str,
    pipe: Option<~LineElem>,
    file: Option<PathType>,
    last : bool,
}
impl LineElem {
    #[allow(dead_code)]
    pub fn new(cmd: ~str) -> ~LineElem {
        ~LineElem {
            cmd: cmd.to_owned(),
            pipe: None,
            file: None,
            last: true,
        }
    }

    // Set the path at the bottom of the pipe chain. This means e.g. in order
    // to set an input path, it needs to happen *before* any pipes are tied
    // to this LineElem.
    #[allow(dead_code)]
    pub fn set_path(&self, path: PathType) -> ~LineElem {
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

    // Tie a new pipe to this LineElem. If a pipeline already exists, add
    // the pipe to the bottom of the pipeline.
    #[allow(dead_code)]
    pub fn set_pipe(&self, pipe: ~LineElem) -> ~LineElem {
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
    
    #[allow(dead_code)]
    pub fn iter(&self) -> ~LineElem {
       ~self.clone()
    }
}

impl Iterator<~LineElem> for LineElem {
    #[allow(dead_code)]
    fn next(&mut self) -> Option<~LineElem> {
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
