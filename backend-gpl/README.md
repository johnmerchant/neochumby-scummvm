# backend-gpl

ScummVM's licence, GPL-3.0-or-later (`COPYING`), and not the MIT licence of the rest
of this repository.

`chumby.cpp` is the OSystem backend. It is written to ScummVM's interfaces, after
`backends/platform/null/null.cpp` in ScummVM's own source, and is compiled against
its headers, so it's kept here under ScummVM's terms. `chumby.mk` extends ScummVM's
generated Makefile to build it.
