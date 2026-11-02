"""The IOP module of the IDL to Python mapping: service contexts, tagged
components and profiles, and ``Codec`` / ``CodecFactory`` for CDR
encapsulations (``ENCODING_CDR_ENCAPS`` 1.0 to 1.2).

Get the factory with ``orb.resolve_initial_references("CodecFactory")``
or, in an ``ORBInitializer``, ``info._get_codec_factory()``::

    codec = factory.create_codec(IOP.Encoding(IOP.ENCODING_CDR_ENCAPS, 1, 2))
    data = codec.encode_value(CORBA.Any(CORBA.TC_string, "hello"))

See docs/portable-interceptors.md.
"""

from . import CORBA, _types

# Profile tags.
TAG_INTERNET_IOP = 0
TAG_MULTIPLE_COMPONENTS = 1

# Component tags (those the core knows, see rustorb_giop::component_tag).
TAG_ORB_TYPE = 0
TAG_CODE_SETS = 1
TAG_POLICIES = 2
TAG_ALTERNATE_IIOP_ADDRESS = 3
TAG_COMPLETE_OBJECT_KEY = 5
TAG_ENDPOINT_ID_POSITION = 6
TAG_LOCATION_POLICY = 12
TAG_ASSOCIATION_OPTIONS = 13
TAG_SEC_NAME = 14
TAG_SSL_SEC_TRANS = 20
TAG_JAVA_CODEBASE = 25
TAG_CSI_SEC_MECH_LIST = 33
TAG_NULL_TAG = 34
TAG_TLS_SEC_TRANS = 36

# Service context ids (see rustorb_giop::service_id).
TransactionService = 0
CodeSets = 1
ChainBypassCheck = 2
ChainBypassInfo = 3
LogicalThreadId = 4
BI_DIR_IIOP = 5
SendingContextRunTime = 6
INVOCATION_POLICIES = 7
FORWARDED_IDENTITY = 8
UnknownExceptionInfo = 9
RTCorbaPriority = 10
RTCorbaPriorityRange = 11
FT_GROUP_VERSION = 12
FT_REQUEST = 13
ExceptionDetailMessage = 14

#: ``IOP::EncodingFormat`` of CDR encapsulations.
ENCODING_CDR_ENCAPS = 0


class TaggedProfile(_types.Struct):
    _NP_RepositoryId = "IDL:omg.org/IOP/TaggedProfile:1.0"
    _NP_members = ("tag", "profile_data")


class IOR(_types.Struct):
    _NP_RepositoryId = "IDL:omg.org/IOP/IOR:1.0"
    _NP_members = ("type_id", "profiles")


class TaggedComponent(_types.Struct):
    _NP_RepositoryId = "IDL:omg.org/IOP/TaggedComponent:1.0"
    _NP_members = ("tag", "component_data")


class ServiceContext(_types.Struct):
    _NP_RepositoryId = "IDL:omg.org/IOP/ServiceContext:1.0"
    _NP_members = ("context_id", "context_data")


class Encoding(_types.Struct):
    _NP_RepositoryId = "IDL:omg.org/IOP/Encoding:1.0"
    _NP_members = ("format", "major_version", "minor_version")


class Codec(CORBA.LocalObject):
    """``IOP::Codec``: CDR encapsulations of anys (``encode`` /
    ``decode``) and of values alone (``encode_value`` /
    ``decode_value``)."""

    _NP_RepositoryId = "IDL:omg.org/IOP/Codec:1.0"

    class InvalidTypeForEncoding(CORBA.UserException):
        _NP_RepositoryId = "IDL:omg.org/IOP/Codec/InvalidTypeForEncoding:1.0"

    class FormatMismatch(CORBA.UserException):
        _NP_RepositoryId = "IDL:omg.org/IOP/Codec/FormatMismatch:1.0"

    class TypeMismatch(CORBA.UserException):
        _NP_RepositoryId = "IDL:omg.org/IOP/Codec/TypeMismatch:1.0"

    def __init__(self, native):
        CORBA.LocalObject.__init__(self)
        self._native = native

    def _call(self, f, *args):
        try:
            return f(*args)
        except ValueError as e:
            raise getattr(Codec, str(e.args[0]))()

    def encode(self, data):
        return self._call(self._native.encode, data)

    def decode(self, data):
        return self._call(self._native.decode, bytes(data))

    def encode_value(self, data):
        return self._call(self._native.encode_value, data)

    def decode_value(self, data, tc):
        return self._call(self._native.decode_value, bytes(data), tc)


class CodecFactory(CORBA.LocalObject):
    """``IOP::CodecFactory`` (initial reference ``CodecFactory``)."""

    _NP_RepositoryId = "IDL:omg.org/IOP/CodecFactory:1.0"

    class UnknownEncoding(CORBA.UserException):
        _NP_RepositoryId = "IDL:omg.org/IOP/CodecFactory/UnknownEncoding:1.0"

    def __init__(self, native):
        CORBA.LocalObject.__init__(self)
        self._native = native

    def create_codec(self, enc):
        try:
            native = self._native.create_codec(
                int(enc.format), int(enc.major_version), int(enc.minor_version)
            )
        except (LookupError, OverflowError):
            raise CodecFactory.UnknownEncoding()
        return Codec(native)
