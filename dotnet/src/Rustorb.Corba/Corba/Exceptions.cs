// The exception classes of CORBA: Corba.UserException (IDL4 to C# A.1.1) and
// Corba.SystemException, which the mapping leaves to the platform.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

namespace Corba
{
    /// <summary>CORBA::CompletionStatus.</summary>
    public enum CompletionStatus
    {
        COMPLETED_YES,
        COMPLETED_NO,
        COMPLETED_MAYBE,
    }

    /// <summary>The base of the classes of IDL exceptions (IDL4 to C# A.1.1).</summary>
    public class UserException : System.Exception
    {
        public UserException() { }
        public UserException(string message) : base(message) { }

        /// <summary>The repository id of the exception (rustorb).</summary>
        public virtual string _id() => "IDL:omg.org/CORBA/UserException:1.0";
    }

    /// <summary>The base of the system exceptions (rustorb: not in the
    /// IDL4 to C# mapping).</summary>
    public abstract class SystemException : System.Exception
    {
        /// <summary>The minor code.</summary>
        public uint minor { get; set; }
        /// <summary>Whether the operation completed.</summary>
        public CompletionStatus completed { get; set; }

        protected SystemException(string reason, uint minor, CompletionStatus completed)
            : base(reason)
        {
            this.minor = minor;
            this.completed = completed;
        }

        /// <summary>The repository id, as IDL:omg.org/CORBA/TRANSIENT:1.0.</summary>
        public abstract string _id();

        public override string Message
        {
            get
            {
                string name = GetType().Name;
                string reason = base.Message;
                string text = $"CORBA.{name} (minor 0x{minor:x}, {completed})";
                return string.IsNullOrEmpty(reason) ? text : text + ": " + reason;
            }
        }
    }
}
