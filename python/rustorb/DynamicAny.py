"""``DynamicAny``: building and taking apart values of types known only by
their TypeCode, as in the IDL to Python mapping::

    factory = orb.resolve_initial_references("DynAnyFactory")
    d = factory.create_dyn_any_from_type_code(tc)   # a DynStruct...
    d.insert_long(1)
    d.next()
    d.insert_string("x")
    any = d.to_any()

The factory gives the DynAny of the class its type calls for: DynFixed,
DynEnum, DynStruct (structs and exceptions), DynUnion, DynSequence,
DynArray, DynValue, DynValueBox or plain DynAny. Components share their
state with the DynAny they belong to. See docs/dynany.md.
"""

from . import CORBA

_native = None


def _n():
    global _native
    if _native is None:
        from . import _native as native

        _native = native
    return _native


def _kind(k):
    items = CORBA.TCKind._items
    # The Python TCKind ends at tk_local_interface.
    return items[k] if k < len(items) else k


class DynAny(CORBA.LocalObject):
    """``DynamicAny::DynAny``."""

    _NP_RepositoryId = "IDL:omg.org/DynamicAny/DynAny:1.0"

    class InvalidValue(CORBA.UserException):
        _NP_RepositoryId = "IDL:omg.org/DynamicAny/DynAny/InvalidValue:1.0"

    class TypeMismatch(CORBA.UserException):
        _NP_RepositoryId = "IDL:omg.org/DynamicAny/DynAny/TypeMismatch:1.0"

    def __init__(self, native):
        CORBA.LocalObject.__init__(self)
        self._n = native

    def _is_equivalent(self, other):
        return isinstance(other, DynAny) and self._n.same(other._n)

    def type(self):
        return self._n.type()

    def assign(self, dyn_any):
        self._n.assign(dyn_any._n)

    def from_any(self, value):
        self._n.from_any(value)

    def to_any(self):
        return self._n.to_any()

    def equal(self, dyn_any):
        return self._n.equal(dyn_any._n)

    def destroy(self):
        self._n.destroy()

    def copy(self):
        return _wrap(self._n.copy())

    def insert_reference(self, value):
        self._n.insert(CORBA.tk_objref._v, CORBA.TC_Object, value)

    def get_reference(self):
        return self._n.get(CORBA.tk_objref._v, CORBA.TC_Object)

    def insert_any(self, value):
        self._n.insert(CORBA.tk_any._v, CORBA.TC_any, value)

    def get_any(self):
        return self._n.get(CORBA.tk_any._v, CORBA.TC_any)

    def insert_dyn_any(self, value):
        self._n.insert_dyn_any(value._n)

    def get_dyn_any(self):
        return _wrap(self._n.get_dyn_any())

    def insert_val(self, value):
        self._n.insert_val(value)

    def get_val(self):
        return self._n.get_val()

    def insert_abstract(self, value):
        self._n.no_implement()

    def get_abstract(self):
        self._n.no_implement()

    def seek(self, index):
        return self._n.seek(index)

    def rewind(self):
        self._n.rewind()

    def next(self):
        return self._n.next()

    def component_count(self):
        return self._n.component_count()

    def current_component(self):
        c = self._n.current_component()
        return None if c is None else _wrap(c)


def _basic(name, tc):
    def insert(self, value):
        self._n.insert(tc._k._v, tc, value)

    def get(self):
        return self._n.get(tc._k._v, tc)

    insert.__name__ = "insert_" + name
    get.__name__ = "get_" + name
    setattr(DynAny, "insert_" + name, insert)
    setattr(DynAny, "get_" + name, get)


def _seq(name, tc):
    seq_tc = CORBA.TypeCode(CORBA.tk_sequence, content=tc, length=0)

    def insert(self, value):
        self._n.insert_seq(tc._k._v, seq_tc, value)

    def get(self):
        return self._n.get_seq(tc._k._v, seq_tc)

    insert.__name__ = "insert_%s_seq" % name
    get.__name__ = "get_%s_seq" % name
    setattr(DynAny, insert.__name__, insert)
    setattr(DynAny, get.__name__, get)


for _name, _tc in [
    ("boolean", CORBA.TC_boolean),
    ("octet", CORBA.TC_octet),
    ("char", CORBA.TC_char),
    ("short", CORBA.TC_short),
    ("ushort", CORBA.TC_ushort),
    ("long", CORBA.TC_long),
    ("ulong", CORBA.TC_ulong),
    ("float", CORBA.TC_float),
    ("double", CORBA.TC_double),
    ("longdouble", CORBA.TC_longdouble),
    ("longlong", CORBA.TC_longlong),
    ("ulonglong", CORBA.TC_ulonglong),
    ("wchar", CORBA.TC_wchar),
]:
    _basic(_name, _tc)
    _seq(_name, _tc)
_basic("string", CORBA.TC_string)
_basic("wstring", CORBA.TC_wstring)
_basic("typecode", CORBA.TC_TypeCode)


class DynFixed(DynAny):
    """``DynamicAny::DynFixed``: the value as a string, such as "-12.5"."""

    _NP_RepositoryId = "IDL:omg.org/DynamicAny/DynFixed:1.0"

    def get_value(self):
        return self._n.get_fixed_value()

    def set_value(self, val):
        return self._n.set_fixed_value(val)


class DynEnum(DynAny):
    """``DynamicAny::DynEnum``."""

    _NP_RepositoryId = "IDL:omg.org/DynamicAny/DynEnum:1.0"

    def get_as_string(self):
        return self._n.get_as_string()

    def set_as_string(self, value):
        self._n.set_as_string(value)

    def get_as_ulong(self):
        return self._n.get_as_ulong()

    def set_as_ulong(self, value):
        self._n.set_as_ulong(value)


class NameValuePair(CORBA._types.Struct):
    """A member and its value: ``id`` (the name) and ``value`` (an Any)."""

    _NP_RepositoryId = "IDL:omg.org/DynamicAny/NameValuePair:1.0"
    _NP_members = ("id", "value")

    def __init__(self, id, value):  # noqa: A002
        self.id = id
        self.value = value


class NameDynAnyPair(CORBA._types.Struct):
    """A member and its DynAny: ``id`` and ``value``."""

    _NP_RepositoryId = "IDL:omg.org/DynamicAny/NameDynAnyPair:1.0"
    _NP_members = ("id", "value")

    def __init__(self, id, value):  # noqa: A002
        self.id = id
        self.value = value


class _Members(object):
    """The member operations of DynStruct and DynValue."""

    def current_member_name(self):
        return self._n.current_member_name()

    def current_member_kind(self):
        return _kind(self._n.current_member_kind())

    def get_members(self):
        return [NameValuePair(n, a) for n, a in self._n.get_members()]

    def set_members(self, value):
        self._n.set_members([(p.id, p.value) for p in value])

    def get_members_as_dyn_any(self):
        return [NameDynAnyPair(n, _wrap(d)) for n, d in self._n.get_members_as_dyn_any()]

    def set_members_as_dyn_any(self, value):
        self._n.set_members_as_dyn_any([(p.id, p.value._n) for p in value])


class DynStruct(_Members, DynAny):
    """``DynamicAny::DynStruct``, for structs and exceptions."""

    _NP_RepositoryId = "IDL:omg.org/DynamicAny/DynStruct:1.0"


class DynUnion(DynAny):
    """``DynamicAny::DynUnion``."""

    _NP_RepositoryId = "IDL:omg.org/DynamicAny/DynUnion:1.0"

    def get_discriminator(self):
        return _wrap(self._n.get_discriminator())

    def set_discriminator(self, d):
        self._n.set_discriminator(d._n)

    def set_to_default_member(self):
        self._n.set_to_default_member()

    def set_to_no_active_member(self):
        self._n.set_to_no_active_member()

    def has_no_active_member(self):
        return self._n.has_no_active_member()

    def discriminator_kind(self):
        return _kind(self._n.discriminator_kind())

    def member(self):
        return _wrap(self._n.member())

    def member_name(self):
        return self._n.member_name()

    def member_kind(self):
        return _kind(self._n.member_kind())


class _Elements(object):
    """The element operations of DynSequence and DynArray."""

    def get_elements(self):
        return self._n.get_elements()

    def set_elements(self, value):
        self._n.set_elements(list(value))

    def get_elements_as_dyn_any(self):
        return [_wrap(d) for d in self._n.get_elements_as_dyn_any()]

    def set_elements_as_dyn_any(self, value):
        self._n.set_elements_as_dyn_any([d._n for d in value])


class DynSequence(_Elements, DynAny):
    """``DynamicAny::DynSequence``."""

    _NP_RepositoryId = "IDL:omg.org/DynamicAny/DynSequence:1.0"

    def get_length(self):
        return self._n.get_length()

    def set_length(self, len):  # noqa: A002
        self._n.set_length(len)


class DynArray(_Elements, DynAny):
    """``DynamicAny::DynArray``."""

    _NP_RepositoryId = "IDL:omg.org/DynamicAny/DynArray:1.0"


class DynValueCommon(DynAny):
    """``DynamicAny::DynValueCommon``."""

    _NP_RepositoryId = "IDL:omg.org/DynamicAny/DynValueCommon:1.0"

    def is_null(self):
        return self._n.is_null()

    def set_to_null(self):
        self._n.set_to_null()

    def set_to_value(self):
        self._n.set_to_value()


class DynValue(_Members, DynValueCommon):
    """``DynamicAny::DynValue``, for valuetypes and eventtypes."""

    _NP_RepositoryId = "IDL:omg.org/DynamicAny/DynValue:1.0"


class DynValueBox(DynValueCommon):
    """``DynamicAny::DynValueBox``."""

    _NP_RepositoryId = "IDL:omg.org/DynamicAny/DynValueBox:1.0"

    def get_boxed_value(self):
        return self._n.get_boxed_value()

    def set_boxed_value(self, boxed):
        self._n.set_boxed_value(boxed)

    def get_boxed_value_as_dyn_any(self):
        return _wrap(self._n.get_boxed_value_as_dyn_any())

    def set_boxed_value_as_dyn_any(self, boxed):
        self._n.set_boxed_value_as_dyn_any(boxed._n)


class MustTruncate(CORBA.UserException):
    _NP_RepositoryId = "IDL:omg.org/DynamicAny/MustTruncate:1.0"


_CLASSES = {
    CORBA.tk_fixed._v: DynFixed,
    CORBA.tk_enum._v: DynEnum,
    CORBA.tk_struct._v: DynStruct,
    CORBA.tk_except._v: DynStruct,
    CORBA.tk_union._v: DynUnion,
    CORBA.tk_sequence._v: DynSequence,
    CORBA.tk_array._v: DynArray,
    CORBA.tk_value._v: DynValue,
    36: DynValue,  # tk_event
    CORBA.tk_value_box._v: DynValueBox,
}


def _wrap(native):
    return _CLASSES.get(native.kind(), DynAny)(native)


class DynAnyFactory(CORBA.LocalObject):
    """``DynamicAny::DynAnyFactory``, the initial reference
    ``DynAnyFactory``."""

    _NP_RepositoryId = "IDL:omg.org/DynamicAny/DynAnyFactory:1.0"

    class InconsistentTypeCode(CORBA.UserException):
        _NP_RepositoryId = "IDL:omg.org/DynamicAny/DynAnyFactory/InconsistentTypeCode:1.0"

    def create_dyn_any(self, value):
        return _wrap(_n().DynAny.create(value, True))

    def create_dyn_any_from_type_code(self, type):  # noqa: A002
        return _wrap(_n().DynAny.from_type_code(type))

    def create_dyn_any_without_truncation(self, value):
        return _wrap(_n().DynAny.create(value, False))

    def create_multiple_dyn_anys(self, values, allow_truncate):
        return [_wrap(_n().DynAny.create(v, allow_truncate)) for v in values]

    def create_multiple_anys(self, values):
        return [d.to_any() for d in values]
