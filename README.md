ps2 - gash
==========

Gash is a [shell](https://en.wikipedia.org/wiki/Shell_%28computing%29)
written in
[Rust](https://en.wikipedia.org/wiki/Rust_%28programming_language%29)
(v0.9 as of the time of writing).

It has features including:

- *input/output redirection* with `>` and `<` operators.
- *building pipelines from commands* using the pipe (`|`) operator.
- *backgrounding processes* with the `&` operator.

and has the benefit of being written in a "pointer-safe", thread safe,
and statically type-checked language.

This code was part of [Problem Set 2](http://rust-class.org/pages/ps2.html)
of [CS4414](http://rust-class.org/index.html) at UVa.
