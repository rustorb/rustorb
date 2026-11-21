// The system exceptions of CORBA (CORBA 3.4 part 1, 10.5), one class per
// exception in the Corba namespace. Generated once by a script from the list
// of the standard system exceptions.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

namespace Corba
{

/// <summary>The system exception CORBA::UNKNOWN.</summary>
public class UNKNOWN : SystemException
{
    public UNKNOWN() : this("", 0, CompletionStatus.COMPLETED_NO) { }
    public UNKNOWN(string reason) : this(reason, 0, CompletionStatus.COMPLETED_NO) { }
    public UNKNOWN(uint minor, CompletionStatus completed) : this("", minor, completed) { }
    public UNKNOWN(string reason, uint minor, CompletionStatus completed) : base(reason, minor, completed) { }
    public override string _id() => "IDL:omg.org/CORBA/UNKNOWN:1.0";
}

/// <summary>The system exception CORBA::BAD_PARAM.</summary>
public class BAD_PARAM : SystemException
{
    public BAD_PARAM() : this("", 0, CompletionStatus.COMPLETED_NO) { }
    public BAD_PARAM(string reason) : this(reason, 0, CompletionStatus.COMPLETED_NO) { }
    public BAD_PARAM(uint minor, CompletionStatus completed) : this("", minor, completed) { }
    public BAD_PARAM(string reason, uint minor, CompletionStatus completed) : base(reason, minor, completed) { }
    public override string _id() => "IDL:omg.org/CORBA/BAD_PARAM:1.0";
}

/// <summary>The system exception CORBA::NO_MEMORY.</summary>
public class NO_MEMORY : SystemException
{
    public NO_MEMORY() : this("", 0, CompletionStatus.COMPLETED_NO) { }
    public NO_MEMORY(string reason) : this(reason, 0, CompletionStatus.COMPLETED_NO) { }
    public NO_MEMORY(uint minor, CompletionStatus completed) : this("", minor, completed) { }
    public NO_MEMORY(string reason, uint minor, CompletionStatus completed) : base(reason, minor, completed) { }
    public override string _id() => "IDL:omg.org/CORBA/NO_MEMORY:1.0";
}

/// <summary>The system exception CORBA::IMP_LIMIT.</summary>
public class IMP_LIMIT : SystemException
{
    public IMP_LIMIT() : this("", 0, CompletionStatus.COMPLETED_NO) { }
    public IMP_LIMIT(string reason) : this(reason, 0, CompletionStatus.COMPLETED_NO) { }
    public IMP_LIMIT(uint minor, CompletionStatus completed) : this("", minor, completed) { }
    public IMP_LIMIT(string reason, uint minor, CompletionStatus completed) : base(reason, minor, completed) { }
    public override string _id() => "IDL:omg.org/CORBA/IMP_LIMIT:1.0";
}

/// <summary>The system exception CORBA::COMM_FAILURE.</summary>
public class COMM_FAILURE : SystemException
{
    public COMM_FAILURE() : this("", 0, CompletionStatus.COMPLETED_NO) { }
    public COMM_FAILURE(string reason) : this(reason, 0, CompletionStatus.COMPLETED_NO) { }
    public COMM_FAILURE(uint minor, CompletionStatus completed) : this("", minor, completed) { }
    public COMM_FAILURE(string reason, uint minor, CompletionStatus completed) : base(reason, minor, completed) { }
    public override string _id() => "IDL:omg.org/CORBA/COMM_FAILURE:1.0";
}

/// <summary>The system exception CORBA::INV_OBJREF.</summary>
public class INV_OBJREF : SystemException
{
    public INV_OBJREF() : this("", 0, CompletionStatus.COMPLETED_NO) { }
    public INV_OBJREF(string reason) : this(reason, 0, CompletionStatus.COMPLETED_NO) { }
    public INV_OBJREF(uint minor, CompletionStatus completed) : this("", minor, completed) { }
    public INV_OBJREF(string reason, uint minor, CompletionStatus completed) : base(reason, minor, completed) { }
    public override string _id() => "IDL:omg.org/CORBA/INV_OBJREF:1.0";
}

/// <summary>The system exception CORBA::NO_PERMISSION.</summary>
public class NO_PERMISSION : SystemException
{
    public NO_PERMISSION() : this("", 0, CompletionStatus.COMPLETED_NO) { }
    public NO_PERMISSION(string reason) : this(reason, 0, CompletionStatus.COMPLETED_NO) { }
    public NO_PERMISSION(uint minor, CompletionStatus completed) : this("", minor, completed) { }
    public NO_PERMISSION(string reason, uint minor, CompletionStatus completed) : base(reason, minor, completed) { }
    public override string _id() => "IDL:omg.org/CORBA/NO_PERMISSION:1.0";
}

/// <summary>The system exception CORBA::INTERNAL.</summary>
public class INTERNAL : SystemException
{
    public INTERNAL() : this("", 0, CompletionStatus.COMPLETED_NO) { }
    public INTERNAL(string reason) : this(reason, 0, CompletionStatus.COMPLETED_NO) { }
    public INTERNAL(uint minor, CompletionStatus completed) : this("", minor, completed) { }
    public INTERNAL(string reason, uint minor, CompletionStatus completed) : base(reason, minor, completed) { }
    public override string _id() => "IDL:omg.org/CORBA/INTERNAL:1.0";
}

/// <summary>The system exception CORBA::MARSHAL.</summary>
public class MARSHAL : SystemException
{
    public MARSHAL() : this("", 0, CompletionStatus.COMPLETED_NO) { }
    public MARSHAL(string reason) : this(reason, 0, CompletionStatus.COMPLETED_NO) { }
    public MARSHAL(uint minor, CompletionStatus completed) : this("", minor, completed) { }
    public MARSHAL(string reason, uint minor, CompletionStatus completed) : base(reason, minor, completed) { }
    public override string _id() => "IDL:omg.org/CORBA/MARSHAL:1.0";
}

/// <summary>The system exception CORBA::INITIALIZE.</summary>
public class INITIALIZE : SystemException
{
    public INITIALIZE() : this("", 0, CompletionStatus.COMPLETED_NO) { }
    public INITIALIZE(string reason) : this(reason, 0, CompletionStatus.COMPLETED_NO) { }
    public INITIALIZE(uint minor, CompletionStatus completed) : this("", minor, completed) { }
    public INITIALIZE(string reason, uint minor, CompletionStatus completed) : base(reason, minor, completed) { }
    public override string _id() => "IDL:omg.org/CORBA/INITIALIZE:1.0";
}

/// <summary>The system exception CORBA::NO_IMPLEMENT.</summary>
public class NO_IMPLEMENT : SystemException
{
    public NO_IMPLEMENT() : this("", 0, CompletionStatus.COMPLETED_NO) { }
    public NO_IMPLEMENT(string reason) : this(reason, 0, CompletionStatus.COMPLETED_NO) { }
    public NO_IMPLEMENT(uint minor, CompletionStatus completed) : this("", minor, completed) { }
    public NO_IMPLEMENT(string reason, uint minor, CompletionStatus completed) : base(reason, minor, completed) { }
    public override string _id() => "IDL:omg.org/CORBA/NO_IMPLEMENT:1.0";
}

/// <summary>The system exception CORBA::BAD_TYPECODE.</summary>
public class BAD_TYPECODE : SystemException
{
    public BAD_TYPECODE() : this("", 0, CompletionStatus.COMPLETED_NO) { }
    public BAD_TYPECODE(string reason) : this(reason, 0, CompletionStatus.COMPLETED_NO) { }
    public BAD_TYPECODE(uint minor, CompletionStatus completed) : this("", minor, completed) { }
    public BAD_TYPECODE(string reason, uint minor, CompletionStatus completed) : base(reason, minor, completed) { }
    public override string _id() => "IDL:omg.org/CORBA/BAD_TYPECODE:1.0";
}

/// <summary>The system exception CORBA::BAD_OPERATION.</summary>
public class BAD_OPERATION : SystemException
{
    public BAD_OPERATION() : this("", 0, CompletionStatus.COMPLETED_NO) { }
    public BAD_OPERATION(string reason) : this(reason, 0, CompletionStatus.COMPLETED_NO) { }
    public BAD_OPERATION(uint minor, CompletionStatus completed) : this("", minor, completed) { }
    public BAD_OPERATION(string reason, uint minor, CompletionStatus completed) : base(reason, minor, completed) { }
    public override string _id() => "IDL:omg.org/CORBA/BAD_OPERATION:1.0";
}

/// <summary>The system exception CORBA::NO_RESOURCES.</summary>
public class NO_RESOURCES : SystemException
{
    public NO_RESOURCES() : this("", 0, CompletionStatus.COMPLETED_NO) { }
    public NO_RESOURCES(string reason) : this(reason, 0, CompletionStatus.COMPLETED_NO) { }
    public NO_RESOURCES(uint minor, CompletionStatus completed) : this("", minor, completed) { }
    public NO_RESOURCES(string reason, uint minor, CompletionStatus completed) : base(reason, minor, completed) { }
    public override string _id() => "IDL:omg.org/CORBA/NO_RESOURCES:1.0";
}

/// <summary>The system exception CORBA::NO_RESPONSE.</summary>
public class NO_RESPONSE : SystemException
{
    public NO_RESPONSE() : this("", 0, CompletionStatus.COMPLETED_NO) { }
    public NO_RESPONSE(string reason) : this(reason, 0, CompletionStatus.COMPLETED_NO) { }
    public NO_RESPONSE(uint minor, CompletionStatus completed) : this("", minor, completed) { }
    public NO_RESPONSE(string reason, uint minor, CompletionStatus completed) : base(reason, minor, completed) { }
    public override string _id() => "IDL:omg.org/CORBA/NO_RESPONSE:1.0";
}

/// <summary>The system exception CORBA::PERSIST_STORE.</summary>
public class PERSIST_STORE : SystemException
{
    public PERSIST_STORE() : this("", 0, CompletionStatus.COMPLETED_NO) { }
    public PERSIST_STORE(string reason) : this(reason, 0, CompletionStatus.COMPLETED_NO) { }
    public PERSIST_STORE(uint minor, CompletionStatus completed) : this("", minor, completed) { }
    public PERSIST_STORE(string reason, uint minor, CompletionStatus completed) : base(reason, minor, completed) { }
    public override string _id() => "IDL:omg.org/CORBA/PERSIST_STORE:1.0";
}

/// <summary>The system exception CORBA::BAD_INV_ORDER.</summary>
public class BAD_INV_ORDER : SystemException
{
    public BAD_INV_ORDER() : this("", 0, CompletionStatus.COMPLETED_NO) { }
    public BAD_INV_ORDER(string reason) : this(reason, 0, CompletionStatus.COMPLETED_NO) { }
    public BAD_INV_ORDER(uint minor, CompletionStatus completed) : this("", minor, completed) { }
    public BAD_INV_ORDER(string reason, uint minor, CompletionStatus completed) : base(reason, minor, completed) { }
    public override string _id() => "IDL:omg.org/CORBA/BAD_INV_ORDER:1.0";
}

/// <summary>The system exception CORBA::TRANSIENT.</summary>
public class TRANSIENT : SystemException
{
    public TRANSIENT() : this("", 0, CompletionStatus.COMPLETED_NO) { }
    public TRANSIENT(string reason) : this(reason, 0, CompletionStatus.COMPLETED_NO) { }
    public TRANSIENT(uint minor, CompletionStatus completed) : this("", minor, completed) { }
    public TRANSIENT(string reason, uint minor, CompletionStatus completed) : base(reason, minor, completed) { }
    public override string _id() => "IDL:omg.org/CORBA/TRANSIENT:1.0";
}

/// <summary>The system exception CORBA::FREE_MEM.</summary>
public class FREE_MEM : SystemException
{
    public FREE_MEM() : this("", 0, CompletionStatus.COMPLETED_NO) { }
    public FREE_MEM(string reason) : this(reason, 0, CompletionStatus.COMPLETED_NO) { }
    public FREE_MEM(uint minor, CompletionStatus completed) : this("", minor, completed) { }
    public FREE_MEM(string reason, uint minor, CompletionStatus completed) : base(reason, minor, completed) { }
    public override string _id() => "IDL:omg.org/CORBA/FREE_MEM:1.0";
}

/// <summary>The system exception CORBA::INV_IDENT.</summary>
public class INV_IDENT : SystemException
{
    public INV_IDENT() : this("", 0, CompletionStatus.COMPLETED_NO) { }
    public INV_IDENT(string reason) : this(reason, 0, CompletionStatus.COMPLETED_NO) { }
    public INV_IDENT(uint minor, CompletionStatus completed) : this("", minor, completed) { }
    public INV_IDENT(string reason, uint minor, CompletionStatus completed) : base(reason, minor, completed) { }
    public override string _id() => "IDL:omg.org/CORBA/INV_IDENT:1.0";
}

/// <summary>The system exception CORBA::INV_FLAG.</summary>
public class INV_FLAG : SystemException
{
    public INV_FLAG() : this("", 0, CompletionStatus.COMPLETED_NO) { }
    public INV_FLAG(string reason) : this(reason, 0, CompletionStatus.COMPLETED_NO) { }
    public INV_FLAG(uint minor, CompletionStatus completed) : this("", minor, completed) { }
    public INV_FLAG(string reason, uint minor, CompletionStatus completed) : base(reason, minor, completed) { }
    public override string _id() => "IDL:omg.org/CORBA/INV_FLAG:1.0";
}

/// <summary>The system exception CORBA::INTF_REPOS.</summary>
public class INTF_REPOS : SystemException
{
    public INTF_REPOS() : this("", 0, CompletionStatus.COMPLETED_NO) { }
    public INTF_REPOS(string reason) : this(reason, 0, CompletionStatus.COMPLETED_NO) { }
    public INTF_REPOS(uint minor, CompletionStatus completed) : this("", minor, completed) { }
    public INTF_REPOS(string reason, uint minor, CompletionStatus completed) : base(reason, minor, completed) { }
    public override string _id() => "IDL:omg.org/CORBA/INTF_REPOS:1.0";
}

/// <summary>The system exception CORBA::BAD_CONTEXT.</summary>
public class BAD_CONTEXT : SystemException
{
    public BAD_CONTEXT() : this("", 0, CompletionStatus.COMPLETED_NO) { }
    public BAD_CONTEXT(string reason) : this(reason, 0, CompletionStatus.COMPLETED_NO) { }
    public BAD_CONTEXT(uint minor, CompletionStatus completed) : this("", minor, completed) { }
    public BAD_CONTEXT(string reason, uint minor, CompletionStatus completed) : base(reason, minor, completed) { }
    public override string _id() => "IDL:omg.org/CORBA/BAD_CONTEXT:1.0";
}

/// <summary>The system exception CORBA::OBJ_ADAPTER.</summary>
public class OBJ_ADAPTER : SystemException
{
    public OBJ_ADAPTER() : this("", 0, CompletionStatus.COMPLETED_NO) { }
    public OBJ_ADAPTER(string reason) : this(reason, 0, CompletionStatus.COMPLETED_NO) { }
    public OBJ_ADAPTER(uint minor, CompletionStatus completed) : this("", minor, completed) { }
    public OBJ_ADAPTER(string reason, uint minor, CompletionStatus completed) : base(reason, minor, completed) { }
    public override string _id() => "IDL:omg.org/CORBA/OBJ_ADAPTER:1.0";
}

/// <summary>The system exception CORBA::DATA_CONVERSION.</summary>
public class DATA_CONVERSION : SystemException
{
    public DATA_CONVERSION() : this("", 0, CompletionStatus.COMPLETED_NO) { }
    public DATA_CONVERSION(string reason) : this(reason, 0, CompletionStatus.COMPLETED_NO) { }
    public DATA_CONVERSION(uint minor, CompletionStatus completed) : this("", minor, completed) { }
    public DATA_CONVERSION(string reason, uint minor, CompletionStatus completed) : base(reason, minor, completed) { }
    public override string _id() => "IDL:omg.org/CORBA/DATA_CONVERSION:1.0";
}

/// <summary>The system exception CORBA::OBJECT_NOT_EXIST.</summary>
public class OBJECT_NOT_EXIST : SystemException
{
    public OBJECT_NOT_EXIST() : this("", 0, CompletionStatus.COMPLETED_NO) { }
    public OBJECT_NOT_EXIST(string reason) : this(reason, 0, CompletionStatus.COMPLETED_NO) { }
    public OBJECT_NOT_EXIST(uint minor, CompletionStatus completed) : this("", minor, completed) { }
    public OBJECT_NOT_EXIST(string reason, uint minor, CompletionStatus completed) : base(reason, minor, completed) { }
    public override string _id() => "IDL:omg.org/CORBA/OBJECT_NOT_EXIST:1.0";
}

/// <summary>The system exception CORBA::TRANSACTION_REQUIRED.</summary>
public class TRANSACTION_REQUIRED : SystemException
{
    public TRANSACTION_REQUIRED() : this("", 0, CompletionStatus.COMPLETED_NO) { }
    public TRANSACTION_REQUIRED(string reason) : this(reason, 0, CompletionStatus.COMPLETED_NO) { }
    public TRANSACTION_REQUIRED(uint minor, CompletionStatus completed) : this("", minor, completed) { }
    public TRANSACTION_REQUIRED(string reason, uint minor, CompletionStatus completed) : base(reason, minor, completed) { }
    public override string _id() => "IDL:omg.org/CORBA/TRANSACTION_REQUIRED:1.0";
}

/// <summary>The system exception CORBA::TRANSACTION_ROLLEDBACK.</summary>
public class TRANSACTION_ROLLEDBACK : SystemException
{
    public TRANSACTION_ROLLEDBACK() : this("", 0, CompletionStatus.COMPLETED_NO) { }
    public TRANSACTION_ROLLEDBACK(string reason) : this(reason, 0, CompletionStatus.COMPLETED_NO) { }
    public TRANSACTION_ROLLEDBACK(uint minor, CompletionStatus completed) : this("", minor, completed) { }
    public TRANSACTION_ROLLEDBACK(string reason, uint minor, CompletionStatus completed) : base(reason, minor, completed) { }
    public override string _id() => "IDL:omg.org/CORBA/TRANSACTION_ROLLEDBACK:1.0";
}

/// <summary>The system exception CORBA::INVALID_TRANSACTION.</summary>
public class INVALID_TRANSACTION : SystemException
{
    public INVALID_TRANSACTION() : this("", 0, CompletionStatus.COMPLETED_NO) { }
    public INVALID_TRANSACTION(string reason) : this(reason, 0, CompletionStatus.COMPLETED_NO) { }
    public INVALID_TRANSACTION(uint minor, CompletionStatus completed) : this("", minor, completed) { }
    public INVALID_TRANSACTION(string reason, uint minor, CompletionStatus completed) : base(reason, minor, completed) { }
    public override string _id() => "IDL:omg.org/CORBA/INVALID_TRANSACTION:1.0";
}

/// <summary>The system exception CORBA::INV_POLICY.</summary>
public class INV_POLICY : SystemException
{
    public INV_POLICY() : this("", 0, CompletionStatus.COMPLETED_NO) { }
    public INV_POLICY(string reason) : this(reason, 0, CompletionStatus.COMPLETED_NO) { }
    public INV_POLICY(uint minor, CompletionStatus completed) : this("", minor, completed) { }
    public INV_POLICY(string reason, uint minor, CompletionStatus completed) : base(reason, minor, completed) { }
    public override string _id() => "IDL:omg.org/CORBA/INV_POLICY:1.0";
}

/// <summary>The system exception CORBA::CODESET_INCOMPATIBLE.</summary>
public class CODESET_INCOMPATIBLE : SystemException
{
    public CODESET_INCOMPATIBLE() : this("", 0, CompletionStatus.COMPLETED_NO) { }
    public CODESET_INCOMPATIBLE(string reason) : this(reason, 0, CompletionStatus.COMPLETED_NO) { }
    public CODESET_INCOMPATIBLE(uint minor, CompletionStatus completed) : this("", minor, completed) { }
    public CODESET_INCOMPATIBLE(string reason, uint minor, CompletionStatus completed) : base(reason, minor, completed) { }
    public override string _id() => "IDL:omg.org/CORBA/CODESET_INCOMPATIBLE:1.0";
}

/// <summary>The system exception CORBA::REBIND.</summary>
public class REBIND : SystemException
{
    public REBIND() : this("", 0, CompletionStatus.COMPLETED_NO) { }
    public REBIND(string reason) : this(reason, 0, CompletionStatus.COMPLETED_NO) { }
    public REBIND(uint minor, CompletionStatus completed) : this("", minor, completed) { }
    public REBIND(string reason, uint minor, CompletionStatus completed) : base(reason, minor, completed) { }
    public override string _id() => "IDL:omg.org/CORBA/REBIND:1.0";
}

/// <summary>The system exception CORBA::TIMEOUT.</summary>
public class TIMEOUT : SystemException
{
    public TIMEOUT() : this("", 0, CompletionStatus.COMPLETED_NO) { }
    public TIMEOUT(string reason) : this(reason, 0, CompletionStatus.COMPLETED_NO) { }
    public TIMEOUT(uint minor, CompletionStatus completed) : this("", minor, completed) { }
    public TIMEOUT(string reason, uint minor, CompletionStatus completed) : base(reason, minor, completed) { }
    public override string _id() => "IDL:omg.org/CORBA/TIMEOUT:1.0";
}

/// <summary>The system exception CORBA::TRANSACTION_UNAVAILABLE.</summary>
public class TRANSACTION_UNAVAILABLE : SystemException
{
    public TRANSACTION_UNAVAILABLE() : this("", 0, CompletionStatus.COMPLETED_NO) { }
    public TRANSACTION_UNAVAILABLE(string reason) : this(reason, 0, CompletionStatus.COMPLETED_NO) { }
    public TRANSACTION_UNAVAILABLE(uint minor, CompletionStatus completed) : this("", minor, completed) { }
    public TRANSACTION_UNAVAILABLE(string reason, uint minor, CompletionStatus completed) : base(reason, minor, completed) { }
    public override string _id() => "IDL:omg.org/CORBA/TRANSACTION_UNAVAILABLE:1.0";
}

/// <summary>The system exception CORBA::TRANSACTION_MODE.</summary>
public class TRANSACTION_MODE : SystemException
{
    public TRANSACTION_MODE() : this("", 0, CompletionStatus.COMPLETED_NO) { }
    public TRANSACTION_MODE(string reason) : this(reason, 0, CompletionStatus.COMPLETED_NO) { }
    public TRANSACTION_MODE(uint minor, CompletionStatus completed) : this("", minor, completed) { }
    public TRANSACTION_MODE(string reason, uint minor, CompletionStatus completed) : base(reason, minor, completed) { }
    public override string _id() => "IDL:omg.org/CORBA/TRANSACTION_MODE:1.0";
}

/// <summary>The system exception CORBA::BAD_QOS.</summary>
public class BAD_QOS : SystemException
{
    public BAD_QOS() : this("", 0, CompletionStatus.COMPLETED_NO) { }
    public BAD_QOS(string reason) : this(reason, 0, CompletionStatus.COMPLETED_NO) { }
    public BAD_QOS(uint minor, CompletionStatus completed) : this("", minor, completed) { }
    public BAD_QOS(string reason, uint minor, CompletionStatus completed) : base(reason, minor, completed) { }
    public override string _id() => "IDL:omg.org/CORBA/BAD_QOS:1.0";
}

/// <summary>The system exception CORBA::INVALID_ACTIVITY.</summary>
public class INVALID_ACTIVITY : SystemException
{
    public INVALID_ACTIVITY() : this("", 0, CompletionStatus.COMPLETED_NO) { }
    public INVALID_ACTIVITY(string reason) : this(reason, 0, CompletionStatus.COMPLETED_NO) { }
    public INVALID_ACTIVITY(uint minor, CompletionStatus completed) : this("", minor, completed) { }
    public INVALID_ACTIVITY(string reason, uint minor, CompletionStatus completed) : base(reason, minor, completed) { }
    public override string _id() => "IDL:omg.org/CORBA/INVALID_ACTIVITY:1.0";
}

/// <summary>The system exception CORBA::ACTIVITY_COMPLETED.</summary>
public class ACTIVITY_COMPLETED : SystemException
{
    public ACTIVITY_COMPLETED() : this("", 0, CompletionStatus.COMPLETED_NO) { }
    public ACTIVITY_COMPLETED(string reason) : this(reason, 0, CompletionStatus.COMPLETED_NO) { }
    public ACTIVITY_COMPLETED(uint minor, CompletionStatus completed) : this("", minor, completed) { }
    public ACTIVITY_COMPLETED(string reason, uint minor, CompletionStatus completed) : base(reason, minor, completed) { }
    public override string _id() => "IDL:omg.org/CORBA/ACTIVITY_COMPLETED:1.0";
}

/// <summary>The system exception CORBA::ACTIVITY_REQUIRED.</summary>
public class ACTIVITY_REQUIRED : SystemException
{
    public ACTIVITY_REQUIRED() : this("", 0, CompletionStatus.COMPLETED_NO) { }
    public ACTIVITY_REQUIRED(string reason) : this(reason, 0, CompletionStatus.COMPLETED_NO) { }
    public ACTIVITY_REQUIRED(uint minor, CompletionStatus completed) : this("", minor, completed) { }
    public ACTIVITY_REQUIRED(string reason, uint minor, CompletionStatus completed) : base(reason, minor, completed) { }
    public override string _id() => "IDL:omg.org/CORBA/ACTIVITY_REQUIRED:1.0";
}

/// <summary>The system exception CORBA::THREAD_CANCELLED.</summary>
public class THREAD_CANCELLED : SystemException
{
    public THREAD_CANCELLED() : this("", 0, CompletionStatus.COMPLETED_NO) { }
    public THREAD_CANCELLED(string reason) : this(reason, 0, CompletionStatus.COMPLETED_NO) { }
    public THREAD_CANCELLED(uint minor, CompletionStatus completed) : this("", minor, completed) { }
    public THREAD_CANCELLED(string reason, uint minor, CompletionStatus completed) : base(reason, minor, completed) { }
    public override string _id() => "IDL:omg.org/CORBA/THREAD_CANCELLED:1.0";
}
}

namespace Rustorb
{
    internal static class SystemExceptions
    {
        private const string Prefix = "IDL:omg.org/CORBA/";

        /// <summary>The exception of a repository id; UNKNOWN for one that is not a
        /// standard system exception.</summary>
        internal static Corba.SystemException Make(string id, uint minor, Corba.CompletionStatus completed)
        {
            string name = id;
            if (name.StartsWith(Prefix, System.StringComparison.Ordinal) && name.EndsWith(":1.0", System.StringComparison.Ordinal))
            {
                name = name.Substring(Prefix.Length, name.Length - Prefix.Length - 4);
            }
            switch (name)
            {
                case "UNKNOWN": return new Corba.UNKNOWN("", minor, completed);
                case "BAD_PARAM": return new Corba.BAD_PARAM("", minor, completed);
                case "NO_MEMORY": return new Corba.NO_MEMORY("", minor, completed);
                case "IMP_LIMIT": return new Corba.IMP_LIMIT("", minor, completed);
                case "COMM_FAILURE": return new Corba.COMM_FAILURE("", minor, completed);
                case "INV_OBJREF": return new Corba.INV_OBJREF("", minor, completed);
                case "NO_PERMISSION": return new Corba.NO_PERMISSION("", minor, completed);
                case "INTERNAL": return new Corba.INTERNAL("", minor, completed);
                case "MARSHAL": return new Corba.MARSHAL("", minor, completed);
                case "INITIALIZE": return new Corba.INITIALIZE("", minor, completed);
                case "NO_IMPLEMENT": return new Corba.NO_IMPLEMENT("", minor, completed);
                case "BAD_TYPECODE": return new Corba.BAD_TYPECODE("", minor, completed);
                case "BAD_OPERATION": return new Corba.BAD_OPERATION("", minor, completed);
                case "NO_RESOURCES": return new Corba.NO_RESOURCES("", minor, completed);
                case "NO_RESPONSE": return new Corba.NO_RESPONSE("", minor, completed);
                case "PERSIST_STORE": return new Corba.PERSIST_STORE("", minor, completed);
                case "BAD_INV_ORDER": return new Corba.BAD_INV_ORDER("", minor, completed);
                case "TRANSIENT": return new Corba.TRANSIENT("", minor, completed);
                case "FREE_MEM": return new Corba.FREE_MEM("", minor, completed);
                case "INV_IDENT": return new Corba.INV_IDENT("", minor, completed);
                case "INV_FLAG": return new Corba.INV_FLAG("", minor, completed);
                case "INTF_REPOS": return new Corba.INTF_REPOS("", minor, completed);
                case "BAD_CONTEXT": return new Corba.BAD_CONTEXT("", minor, completed);
                case "OBJ_ADAPTER": return new Corba.OBJ_ADAPTER("", minor, completed);
                case "DATA_CONVERSION": return new Corba.DATA_CONVERSION("", minor, completed);
                case "OBJECT_NOT_EXIST": return new Corba.OBJECT_NOT_EXIST("", minor, completed);
                case "TRANSACTION_REQUIRED": return new Corba.TRANSACTION_REQUIRED("", minor, completed);
                case "TRANSACTION_ROLLEDBACK": return new Corba.TRANSACTION_ROLLEDBACK("", minor, completed);
                case "INVALID_TRANSACTION": return new Corba.INVALID_TRANSACTION("", minor, completed);
                case "INV_POLICY": return new Corba.INV_POLICY("", minor, completed);
                case "CODESET_INCOMPATIBLE": return new Corba.CODESET_INCOMPATIBLE("", minor, completed);
                case "REBIND": return new Corba.REBIND("", minor, completed);
                case "TIMEOUT": return new Corba.TIMEOUT("", minor, completed);
                case "TRANSACTION_UNAVAILABLE": return new Corba.TRANSACTION_UNAVAILABLE("", minor, completed);
                case "TRANSACTION_MODE": return new Corba.TRANSACTION_MODE("", minor, completed);
                case "BAD_QOS": return new Corba.BAD_QOS("", minor, completed);
                case "INVALID_ACTIVITY": return new Corba.INVALID_ACTIVITY("", minor, completed);
                case "ACTIVITY_COMPLETED": return new Corba.ACTIVITY_COMPLETED("", minor, completed);
                case "ACTIVITY_REQUIRED": return new Corba.ACTIVITY_REQUIRED("", minor, completed);
                case "THREAD_CANCELLED": return new Corba.THREAD_CANCELLED("", minor, completed);
                default: return new Corba.UNKNOWN("", minor, completed);
            }
        }
    }
}
