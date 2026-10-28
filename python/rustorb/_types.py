"""Base classes of the Python values of IDL constructed types.

Generated code (``rustorb-idl -bpython``) derives its struct, union and
exception classes from these, and builds enums with :class:`Enum`. They
follow the OMG IDL to Python mapping; the module is internal and its names
may change.
"""

from typing import Optional


class EnumItem(object):
    """One enumerator of an IDL enum (``InteropTest.RED``).

    Enumerators compare by identity, are ordered by their values and can be
    used as dict keys. ``_n`` is the name and ``_v`` the value (position).
    """

    __slots__ = ("_n", "_v", "_parent_id")

    def __init__(self, name, value, parent_id):
        self._n = name
        self._v = value
        self._parent_id = parent_id

    def __repr__(self):
        return self._n

    __str__ = __repr__

    def __int__(self):
        return self._v

    __index__ = __int__

    def __hash__(self):
        return hash((self._parent_id, self._v))

    def __eq__(self, other):
        return self is other or (
            isinstance(other, EnumItem)
            and other._parent_id == self._parent_id
            and other._v == self._v
        )

    def __ne__(self, other):
        return not self.__eq__(other)

    def __lt__(self, other):
        self._check(other)
        return self._v < other._v

    def __le__(self, other):
        self._check(other)
        return self._v <= other._v

    def __gt__(self, other):
        self._check(other)
        return self._v > other._v

    def __ge__(self, other):
        self._check(other)
        return self._v >= other._v

    def _check(self, other):
        if not isinstance(other, EnumItem) or other._parent_id != self._parent_id:
            raise TypeError("cannot compare enumerators of different enums")

    def __reduce__(self):
        return (_enum_item, (self._parent_id, self._v))


class Enum(object):
    """An IDL enum type (``InteropTest.Color``).

    ``_items`` holds the enumerators in order and ``_item(n)`` returns the
    one whose value is ``n``.
    """

    def __init__(self, repoId, names):
        self._NP_RepositoryId = repoId
        self._items = tuple(EnumItem(n, i, repoId) for i, n in enumerate(names))
        _enums[repoId] = self

    def _item(self, n):
        return self._items[n]

    def __repr__(self):
        return "<enum %s>" % self._NP_RepositoryId


_enums: dict = {}


def _enum_item(repoId, value):
    return _enums[repoId]._items[value]


class Struct(object):
    """Base of IDL structs. Subclasses set ``_NP_RepositoryId`` and
    ``_NP_members`` (the member names in order) and take the members as
    constructor arguments, positional or by name."""

    _NP_RepositoryId: Optional[str] = None
    _NP_members: tuple = ()

    def __init__(self, *args, **kw):
        _init_members(self, type(self).__name__, self._NP_members, args, kw)

    def __repr__(self):
        return _repr_members(self, self._NP_members)

    def __eq__(self, other):
        return type(other) is type(self) and all(
            getattr(self, m) == getattr(other, m) for m in self._NP_members
        )

    def __ne__(self, other):
        return not self.__eq__(other)

    __hash__ = None  # type: ignore[assignment]


def _init_members(obj, cls_name, members, args, kw):
    if len(args) > len(members):
        raise TypeError("%s() takes %d arguments (%d given)" % (cls_name, len(members), len(args)))
    values = dict(zip(members, args))
    for k, v in kw.items():
        if k not in members:
            raise TypeError("%s() got an unexpected keyword argument '%s'" % (cls_name, k))
        if k in values:
            raise TypeError("%s() got multiple values for argument '%s'" % (cls_name, k))
        values[k] = v
    missing = [m for m in members if m not in values]
    if missing:
        raise TypeError("%s() missing arguments: %s" % (cls_name, ", ".join(missing)))
    for m in members:
        setattr(obj, m, values[m])


def _repr_members(obj, members):
    return "%s(%s)" % (
        type(obj).__name__,
        ", ".join("%s=%r" % (m, getattr(obj, m)) for m in members),
    )


_NO_DEFAULT = object()


class Union(object):
    """Base of IDL unions.

    A union has the discriminator ``_d`` and the value ``_v``. It is made
    with ``U(d, v)`` or with one member by name, ``U(l=1)``; a member can
    then be read and set by name. Subclasses set:

    - ``_NP_RepositoryId``
    - ``_d_to_m``: discriminator value -> member name, for each case label
    - ``_m_to_d``: member name -> its first label
    - ``_def_m``: the member of the ``default`` case, or None
    - ``_def_d``: a discriminator value that no case label uses, or
      ``_NO_DEFAULT`` when the labels cover every value
    """

    _NP_RepositoryId: Optional[str] = None
    _d_to_m: dict = {}
    _m_to_d: dict = {}
    _def_m = None
    _def_d = _NO_DEFAULT

    def __init__(self, *args, **kw):
        if len(args) == 2 and not kw:
            self._d, self._v = args
            return
        if not args and len(kw) == 1:
            ((name, value),) = kw.items()
            self.__setattr__(name, value)
            return
        raise TypeError(
            "%s() takes a discriminator and a value, or one member by name" % type(self).__name__
        )

    def _member(self):
        """The name of the member that ``_d`` selects, or None."""
        m = self._d_to_m.get(self._d)
        if m is None:
            m = self._def_m
        return m

    def __getattr__(self, name):
        cls = type(self)
        if name in cls._m_to_d or (name == cls._def_m and name is not None):
            if self.__dict__.get("_d", _NO_DEFAULT) is not _NO_DEFAULT and self._member() == name:
                return self.__dict__["_v"]
            from . import CORBA

            raise CORBA.BAD_PARAM(0, CORBA.COMPLETED_NO)
        raise AttributeError(name)

    def __setattr__(self, name, value):
        cls = type(self)
        if name in ("_d", "_v"):
            object.__setattr__(self, name, value)
        elif name in cls._m_to_d:
            object.__setattr__(self, "_d", cls._m_to_d[name])
            object.__setattr__(self, "_v", value)
        elif name == cls._def_m and name is not None:
            object.__setattr__(self, "_d", cls._def_d)
            object.__setattr__(self, "_v", value)
        else:
            raise AttributeError("%s has no member '%s'" % (cls.__name__, name))

    def __repr__(self):
        m = self._member()
        if m is None:
            return "%s(%r, %r)" % (type(self).__name__, self._d, self._v)
        return "%s(%s=%r)" % (type(self).__name__, m, self._v)

    def __eq__(self, other):
        return type(other) is type(self) and self._d == other._d and self._v == other._v

    def __ne__(self, other):
        return not self.__eq__(other)

    __hash__ = None  # type: ignore[assignment]
