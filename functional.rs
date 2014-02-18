pub mod functional { 
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
}
