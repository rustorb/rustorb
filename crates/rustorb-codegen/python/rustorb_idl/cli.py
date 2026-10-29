"""Command line: ``rustorb-idl -bBACKEND [options] FILE.idl...``.

The options follow omniidl where they mean the same thing:

  -bpython          Python stubs and skeletons (standard Python mapping)
  -bc++11           C++ stubs and skeletons (IDL to C++11 mapping); also
                    -bcxx11
  -bjava            Java stubs and skeletons (IDL to Java mapping)
  -brust            Rust stubs and skeletons (docs/rust-mapping.md)
  -I DIR            look for #include files in DIR
  -D NAME[=VALUE]   define a preprocessor macro
  -C DIR            write the output under DIR (default: the current directory)
  -Wbglobal=NAME    the module for definitions outside modules (_GlobalIDL)
  -Wbpackage=PKG    put the Python modules in the package PKG (python), or
                    every Java package under PKG (java)
  -Wbno_stubs       do not write the .pyi type stubs (python)
  -Wbh=SUFFIX       the header suffix (c++11, default .hh)
  -Wbs=SUFFIX       the source suffix (c++11, default .cc)
  -Wbextern=MODULE=PATH
                    refer to the definitions of the IDL module MODULE under
                    the Rust path PATH instead of writing them (rust)
  -Wbcore=PATH      the path of rustorb-core (rust, default ::rustorb_core)
  -Wbno_rustfmt     do not pass the output through rustfmt (rust)
"""

import argparse
import sys

from . import __version__
from . import cxx_backend
from . import frontend
from . import java_backend
from . import python_backend
from . import rust_backend


def parse_args(argv):
    ap = argparse.ArgumentParser(
        prog="rustorb-idl",
        description="Generate code for rustorb from IDL files.",
        epilog=__doc__.split("\n\n", 1)[1],
        formatter_class=argparse.RawDescriptionHelpFormatter,
        add_help=True,
    )
    ap.add_argument(
        "-b", dest="backend", required=True, metavar="BACKEND", help="the backend: python, c++11, java or rust"
    )
    ap.add_argument("-I", dest="include", action="append", default=[], metavar="DIR")
    ap.add_argument("-D", dest="define", action="append", default=[], metavar="NAME[=VALUE]")
    ap.add_argument("-C", dest="outdir", default=".", metavar="DIR")
    ap.add_argument("-W", dest="backend_options", action="append", default=[], metavar="bOPTION")
    ap.add_argument("--encoding", default=None, help="character encoding of the IDL files")
    ap.add_argument("--version", action="version", version="rustorb-idl " + __version__)
    ap.add_argument("files", nargs="+", metavar="FILE.idl")
    return ap.parse_args(argv)


def main(argv=None):
    args = parse_args(sys.argv[1:] if argv is None else argv)
    if args.backend == "python":
        backend, options = python_backend, python_backend.Options()
    elif args.backend in ("c++11", "cxx11"):
        backend, options = cxx_backend, cxx_backend.Options()
    elif args.backend == "java":
        backend, options = java_backend, java_backend.Options()
    elif args.backend == "rust":
        backend, options = rust_backend, rust_backend.Options()
    else:
        sys.stderr.write(
            "rustorb-idl: unknown backend %r (python, c++11, java and rust are available)\n" % args.backend
        )
        return 2
    for opt in args.backend_options:
        if backend is python_backend and opt.startswith("bglobal="):
            options.global_module = opt[len("bglobal=") :]
        elif backend is python_backend and opt.startswith("bpackage="):
            options.package = opt[len("bpackage=") :].strip(".")
        elif backend is java_backend and opt.startswith("bpackage="):
            options.package = opt[len("bpackage=") :].strip(".")
        elif backend is python_backend and opt == "bno_stubs":
            options.stubs = False
        elif backend is cxx_backend and opt.startswith("bh="):
            options.header_suffix = opt[len("bh=") :]
        elif backend is cxx_backend and opt.startswith("bs="):
            options.source_suffix = opt[len("bs=") :]
        elif backend is rust_backend and opt.startswith("bextern="):
            module, sep, path = opt[len("bextern=") :].partition("=")
            if not sep or not module or not path:
                sys.stderr.write("rustorb-idl: -Wbextern needs MODULE=PATH\n")
                return 2
            if not path.startswith(("::", "crate::", "super::", "self::")):
                path = "::" + path
            options.externs[module] = path
        elif backend is rust_backend and opt.startswith("bcore="):
            options.core = opt[len("bcore=") :]
        elif backend is rust_backend and opt == "bno_rustfmt":
            options.rustfmt = False
        else:
            sys.stderr.write("rustorb-idl: unknown option -W%s\n" % opt)
            return 2
    defines = {}
    for d in args.define:
        name, _, value = d.partition("=")
        defines[name] = value or "1"
    status = 0
    for path in args.files:
        try:
            spec = frontend.load(path, args.include, defines, args.encoding)
            if backend is python_backend:
                generator = python_backend.PythonBackend(spec, options)
            elif backend is java_backend:
                generator = java_backend.JavaBackend(spec, options)
            elif backend is rust_backend:
                generator = rust_backend.RustBackend(spec, options)
            else:
                generator = cxx_backend.CxxBackend(spec, options)
            backend.write_outputs(generator, args.outdir)
        except frontend.GeneratorError as e:
            sys.stderr.write("rustorb-idl: %s\n" % e)
            status = 1
    return status


def main_exit():
    sys.exit(main())
