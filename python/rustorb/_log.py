"""Trace output of the ORB core through :mod:`logging` (docs/tracing.md).

The core's events go to the logger ``rustorb`` and its children, named
after the core module that reports them (``rustorb.connection``,
``rustorb.server``, ``rustorb.object``, ``rustorb.poa``, ``rustorb.wire``).
Which events the core reports is set by the trace level
(``-ORBtraceLevel N``, or :func:`rustorb.set_trace_level`): 1 (the default)
for warnings, 5 for connections, 10 for each request, 25 for each GIOP
message. Their ``logging`` levels are WARNING, INFO, DEBUG and TRACE (5).

While no handler is configured for these loggers (no
``logging.basicConfig()``, say), the records are written to standard
error as ``rustorb: <level>: <message>`` lines instead, like the other
bindings do.
"""

import atexit
import logging
import sys

TRACE = 5
"""The ``logging`` level of the events of trace level 25 (GIOP messages)."""

_LEVELS = {
    "error": logging.ERROR,
    "warning": logging.WARNING,
    "info": logging.INFO,
    "debug": logging.DEBUG,
    "trace": TRACE,
}

if logging.getLevelName(TRACE) == "Level %d" % TRACE:
    logging.addLevelName(TRACE, "TRACE")

_installed = False
_level_set = False


def _logger_name(target):
    # rustorb_core::connection -> rustorb.connection
    head, _, rest = target.partition("::")
    if head == "rustorb_core":
        head = "rustorb"
    return ".".join([head] + [p for p in rest.split("::") if p])


def _emit(entries):
    for level, target, message, line in entries:
        logger = logging.getLogger(_logger_name(target))
        levelno = _LEVELS.get(level, logging.INFO)
        if logger.hasHandlers():
            if logger.isEnabledFor(levelno):
                logger.log(levelno, "%s", message)
        else:
            try:
                sys.stderr.write(line + "\n")
            except Exception:
                pass


def install(native):
    """Sends the core's events to :mod:`logging`; once."""
    global _installed
    if _installed:
        return
    _installed = True
    native._set_log_sink(_emit)
    atexit.register(native._flush_log)


def follow_trace_level(native):
    """Lets the ``rustorb`` logger pass what the trace level selects, unless
    the application set its level."""
    global _level_set
    logger = logging.getLogger("rustorb")
    if logger.level != logging.NOTSET and not _level_set:
        return
    level = native.trace_level()
    if level >= native.trace_level_of("trace"):
        logger.setLevel(TRACE)
    elif level >= native.trace_level_of("debug"):
        logger.setLevel(logging.DEBUG)
    elif level >= native.trace_level_of("info"):
        logger.setLevel(logging.INFO)
    else:
        logger.setLevel(logging.NOTSET)
    _level_set = logger.level != logging.NOTSET
