# frozen_string_literal: true

# CORBA exceptions (RCLM): CORBA::Exception, the system exceptions with
# their minor code and completion status, CORBA::UserException, and the
# exceptions of the local interfaces (ORB, TypeCode, POA, POAManager,
# Current), which the native extension raises through system_exception
# and local_exception below.
#
# Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

module CORBA
  COMPLETED_YES = 0
  COMPLETED_NO = 1
  COMPLETED_MAYBE = 2

  class Exception < StandardError
    def self._id
      self::ID
    end

    def _id
      self.class._id
    end
  end

  class SystemException < CORBA::Exception
    attr_reader :minor, :completed

    def initialize(reason = "", minor = 0, completed = CORBA::COMPLETED_NO)
      @reason = reason.to_s
      @minor = minor
      @completed = completed
      super(message_text)
    end

    def message_text
      done = %w[COMPLETED_YES COMPLETED_NO COMPLETED_MAYBE][@completed] || @completed.to_s
      text = format("%s (minor 0x%x, %s)", self.class.name.sub("CORBA::", ""), @minor, done)
      @reason.empty? ? text : "#{text}: #{@reason}"
    end

    def ==(other)
      other.class == self.class && other.minor == @minor && other.completed == @completed
    end
  end

  SYSTEM_EXCEPTIONS = {}

  %w[UNKNOWN BAD_PARAM NO_MEMORY IMP_LIMIT COMM_FAILURE INV_OBJREF NO_PERMISSION INTERNAL
     MARSHAL INITIALIZE NO_IMPLEMENT BAD_TYPECODE BAD_OPERATION NO_RESOURCES NO_RESPONSE
     PERSIST_STORE BAD_INV_ORDER TRANSIENT FREE_MEM INV_IDENT INV_FLAG INTF_REPOS BAD_CONTEXT
     OBJ_ADAPTER DATA_CONVERSION OBJECT_NOT_EXIST TRANSACTION_REQUIRED TRANSACTION_ROLLEDBACK
     INVALID_TRANSACTION INV_POLICY CODESET_INCOMPATIBLE REBIND TIMEOUT TRANSACTION_UNAVAILABLE
     TRANSACTION_MODE BAD_QOS INVALID_ACTIVITY ACTIVITY_COMPLETED ACTIVITY_REQUIRED
     THREAD_CANCELLED].each do |name|
    klass = Class.new(SystemException)
    klass.const_set(:ID, "IDL:omg.org/CORBA/#{name}:1.0")
    const_set(name, klass)
    SYSTEM_EXCEPTIONS[klass::ID] = klass
  end

  # A user exception: generated classes derive from it, with attr_accessor
  # members and a positional initialize.
  class UserException < CORBA::Exception
    def initialize(*args)
      super(self.class.name.to_s)
      self.class::MEMBERS.each_with_index { |m, i| instance_variable_set("@#{m}", args[i]) }
    end

    def _values
      self.class::MEMBERS.map { |m| instance_variable_get("@#{m}") }
    end

    def ==(other)
      other.class == self.class && other._values == _values
    end
  end

  # PolicyError (the reason is a PolicyErrorCode).
  BAD_POLICY = 0
  UNSUPPORTED_POLICY = 1
  BAD_POLICY_TYPE = 2
  BAD_POLICY_VALUE = 3
  UNSUPPORTED_POLICY_VALUE = 4

  class PolicyError < UserException
    ID = "IDL:omg.org/CORBA/PolicyError:1.0"
    MEMBERS = %w[reason].freeze
    attr_accessor :reason
  end
end

module Rustorb
  module Native
    # Called by the extension: the exception object to raise for a system
    # exception.
    def self.system_exception(id, minor, completed)
      klass = CORBA::SYSTEM_EXCEPTIONS[id] || CORBA::UNKNOWN
      klass.new("", minor, completed)
    end

    # rorb_exception.local codes, filled in as the classes are defined.
    LOCAL_EXCEPTIONS = {}

    def self.local_exception(code, index)
      maker = LOCAL_EXCEPTIONS[code]
      return CORBA::INTERNAL.new("local exception #{code}") unless maker

      maker.call(index)
    end
  end
end
