require_relative "DemoC"

module POA
  module Demo
    class Calc < PortableServer::Servant
      IDS = ["IDL:Demo/Calc:1.0"].freeze

      def self._operations
        @_operations ||= begin
          ops = ::Demo::Calc._ops
          { "add" => [:add, ops["add"]], "split" => [:split, ops["split"]],
            "_get_label" => [:label, ops["_get_label"]], "_set_label" => [:label=, ops["_set_label"]],
            "fail" => [:fail, ops["fail"]], "ping" => [:ping, ops["ping"]], "echo" => [:echo, ops["echo"]],
            "me" => [:me, ops["me"]] }.freeze
        end
      end
    end
  end
end
