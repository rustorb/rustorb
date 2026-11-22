# Hand-written in the shape rustorb-idl -bruby generates, for the runtime tests.
require "corba"

module Demo
  class Point < CORBA::Portable::Struct
    ID = "IDL:Demo/Point:1.0"
    MEMBERS = %w[x y].freeze
    attr_accessor :x, :y

    def self._tc
      @_tc ||= CORBA::TypeCode.create_struct_tc(ID, "Point", [["x", CORBA._tc_long], ["y", CORBA._tc_long]])
    end
  end
  Rustorb.register_type(Point)

  class Oops < CORBA::UserException
    ID = "IDL:Demo/Oops:1.0"
    MEMBERS = %w[why].freeze
    attr_accessor :why

    def self._tc
      @_tc ||= CORBA::TypeCode.create_exception_tc(ID, "Oops", [["why", CORBA._tc_string]])
    end
  end
  Rustorb.register_type(Oops)

  module Calc
    ID = "IDL:Demo/Calc:1.0"
    extend Rustorb::InterfaceModule

    def self._ops
      @_ops ||= {
        "add" => Rustorb::Operation.new("add", result: CORBA._tc_long,
                                        params: [[:in, CORBA._tc_long], [:in, CORBA._tc_long]]),
        "split" => Rustorb::Operation.new("split", params: [[:in, Demo::Point._tc], [:out, CORBA._tc_long],
                                                            [:inout, CORBA._tc_long]]),
        "_get_label" => Rustorb::Operation.new("_get_label", result: CORBA._tc_string),
        "_set_label" => Rustorb::Operation.new("_set_label", params: [[:in, CORBA._tc_string]]),
        "fail" => Rustorb::Operation.new("fail", params: [[:in, CORBA._tc_string]], exceptions: [Demo::Oops]),
        "ping" => Rustorb::Operation.new("ping", oneway: true),
        "echo" => Rustorb::Operation.new("echo", result: CORBA._tc_any, params: [[:in, CORBA._tc_any]]),
        "me" => Rustorb::Operation.new("me", result: Calc._tc)
      }.freeze
    end

    def add(a, b) = _invoke(Calc._ops["add"], [a, b])
    def split(p, y) = _invoke(Calc._ops["split"], [p, y])
    def label = _invoke(Calc._ops["_get_label"], [])
    def label=(v)
      _invoke(Calc._ops["_set_label"], [v])
    end
    def fail(why) = _invoke(Calc._ops["fail"], [why])
    def ping = _invoke(Calc._ops["ping"], [])
    def echo(a) = _invoke(Calc._ops["echo"], [a])
    def me = _invoke(Calc._ops["me"], [])

    Rustorb.stub_class(self)
  end
end
