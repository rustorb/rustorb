"""The rustorbpy-dii implementation: the rustorb Python client and server
of this directory with every call made through the DII and every servant
of the server run through the DSI.

``install()`` (from common.py, when RUSTORB_DII is set) replaces
``CORBA.Object._invoke``, which the generated stubs call, with a
``CORBA.Request`` built from the operation's TypeCodes, and makes the POA
activate each static servant behind a ``PortableServer.DynamicImplementation``
that reads the arguments with ``CORBA.ServerRequest.arguments`` and calls
the servant's method. The generated descriptions do not tell ``inout``
from ``in`` plus ``out``, or a result from a first ``out`` argument, but
the messages are the same either way.
"""

import weakref

from rustorb import CORBA, PortableServer, _rt


def _dii_invoke(self, op, args):
    req = self._request(op.name)
    for tc, value in zip(op.in_types, args):
        req.add_in_arg(CORBA.Any(tc, value))
    if op.oneway:
        req.send_oneway()
        return None
    outs = list(op.out_types)
    if outs:
        req.set_return_type(outs[0])
    for tc in outs[1:]:
        req.add_out_arg(tc)
    for tc in op.exceptions:
        req.add_exception(tc)
    req.invoke()
    results = [req.return_value().value()] if outs else []
    results += [nv.argument.value() for nv in req.arguments() if nv.arg_modes == CORBA.ARG_OUT]
    if not results:
        return None
    if len(results) == 1:
        return results[0]
    return tuple(results)


class DsiServant(PortableServer.DynamicImplementation):
    """A static servant behind the DSI."""

    def __init__(self, inner):
        self._inner = inner
        # References the servant makes of itself are those of this one.
        inner._this = self._this

    def _primary_interface(self, oid, poa):
        return self._inner._NP_RepositoryId

    def _is_a(self, repoId):
        return self._inner._is_a(repoId)

    def _non_existent(self):
        return self._inner._non_existent()

    def _default_POA(self):
        return self._inner._default_POA()

    def invoke(self, request):
        op = _rt.operation(type(self._inner), request.operation())
        if op is None:
            raise CORBA.BAD_OPERATION(0, CORBA.COMPLETED_NO)
        params = CORBA.NVList()
        for tc in op.in_types:
            params.add_value("", CORBA.Any(tc, None), CORBA.ARG_IN)
        outs = list(op.out_types)
        for tc in outs[1:]:
            params.add_value("", CORBA.Any(tc, None), CORBA.ARG_OUT)
        request.arguments(params)
        method = getattr(self._inner, _rt._py_name(op.name), None)
        if method is None:
            raise CORBA.NO_IMPLEMENT(0, CORBA.COMPLETED_NO)
        args = [nv.argument.value() for nv in params if nv.arg_modes == CORBA.ARG_IN]
        try:
            result = method(*args)
        except CORBA.UserException as e:
            for tc in op.exceptions:
                if tc.id() == e._NP_RepositoryId:
                    request.set_exception(CORBA.Any(tc, e))
                    return
            raise CORBA.UNKNOWN(0, CORBA.COMPLETED_MAYBE)
        if op.oneway or not outs:
            return
        results = [result] if len(outs) == 1 else list(result)
        request.set_result(CORBA.Any(outs[0], results[0]))
        out_items = [nv for nv in params if nv.arg_modes == CORBA.ARG_OUT]
        for nv, tc, value in zip(out_items, outs[1:], results[1:]):
            nv.argument = CORBA.Any(tc, value)


_wrappers = weakref.WeakKeyDictionary()


def _dsi(servant):
    if isinstance(servant, PortableServer.DynamicImplementation) or not isinstance(
        servant, PortableServer.Servant
    ):
        return servant
    wrapper = _wrappers.get(servant)
    if wrapper is None:
        wrapper = _wrappers[servant] = DsiServant(servant)
    return wrapper


def _unwrap(servant):
    return servant._inner if isinstance(servant, DsiServant) else servant


def install():
    CORBA.Object._invoke = _dii_invoke
    for cls in {PortableServer.POA} | set(PortableServer.POA.__subclasses__()):
        for name in ("activate_object", "servant_to_id", "servant_to_reference", "set_servant"):
            orig = cls.__dict__.get(name)
            if orig is not None:
                setattr(cls, name, (lambda f: lambda self, s: f(self, _dsi(s)))(orig))
        orig = cls.__dict__.get("activate_object_with_id")
        if orig is not None:
            setattr(cls, "activate_object_with_id",
                    (lambda f: lambda self, oid, s: f(self, oid, _dsi(s)))(orig))
        for name in ("reference_to_servant", "id_to_servant", "get_servant"):
            orig = cls.__dict__.get(name)
            if orig is not None:
                setattr(cls, name, (lambda f: lambda self, *a: _unwrap(f(self, *a)))(orig))
