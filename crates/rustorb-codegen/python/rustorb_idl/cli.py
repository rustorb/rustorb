"""Command line: ``rustorb-idl -bBACKEND [options] FILE.idl...``.

The options follow omniidl where they mean the same thing:

  -bpython          Python stubs and skeletons (standard Python mapping)
  -bc++11           C++ stubs and skeletons (IDL to C++11 mapping); also
                    -bcxx11
  -I DIR            look for #include files in DIR
  -D NAME[=VALUE]   define a preprocessor macro
  -C DIR            write the output under DIR (default: the current directory)
  -Wbglobal=NAME    the module for definitions outside modules (_GlobalIDL)
  -Wbpackage=PKG    put the Python modules in the package PKG (python)
  -Wbno_stubs       do not write the .pyi type stubs (python)
  -Wbh=SUFFIX       the header suffix (c++11, default .hh)
  -Wbs=SUFFIX       the source suffix (c++11, default .cc)
"""

import argparse
import sys

from . import __version__
from . import cxx_backend
from . import frontend
from . import python_backend


def parse_args(argv):
    ap = argparse.ArgumentParser(
        prog="rustorb-idl",
        description="Generate code for rustorb from IDL files.",
        epilog=__doc__.split("\n\n", 1)[1],
        formatter_class=argparse.RawDescriptionHelpFormatter,
        add_help=True,
    )
    ap.add_argument(
        "-b", dest="backend", required=True, metavar="BACKEND", help="the backend: python or c++11"
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
    else:
        sys.stderr.write(
            "rustorb-idl: unknown backend %r (python and c++11 are available)\n" % args.backend
        )
        return 2
    for opt in args.backend_options:
        if backend is python_backend and opt.startswith("bglobal="):
            options.global_module = opt[len("bglobal=") :]
        elif backend is python_backend and opt.startswith("bpackage="):
            options.package = opt[len("bpackage=") :].strip(".")
        elif backend is python_backend and opt == "bno_stubs":
            options.stubs = False
        elif backend is cxx_backend and opt.startswith("bh="):
            options.header_suffix = opt[len("bh=") :]
        elif backend is cxx_backend and opt.startswith("bs="):
            options.source_suffix = opt[len("bs=") :]
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
            else:
                generator = cxx_backend.CxxBackend(spec, options)
            backend.write_outputs(generator, args.outdir)
        except frontend.GeneratorError as e:
            sys.stderr.write("rustorb-idl: %s\n" % e)
            status = 1
    return status


def main_exit():
    sys.exit(main())
