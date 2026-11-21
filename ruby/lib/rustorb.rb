# frozen_string_literal: true

# rustorb for Ruby: the CORBA Ruby language mapping (RCLM) on the rustorb
# core, through its C ABI (the rustorb_ext native extension).
#
# Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

require "rustorb/rustorb_ext"
require "rustorb/version"
require "rustorb/exceptions"
require "rustorb/typecode"
require "rustorb/types"
require "rustorb/marshal"
require "rustorb/object"
require "rustorb/orb"
require "rustorb/portable_server"
require "rustorb/dynamic_any"
require "rustorb/dii"
require "rustorb/servant_managers"
require "rustorb/values"
require "rustorb/messaging"

module CORBA
  # The names RCLM gives the bases of generated structs and unions.
  module Portable
    Struct = Rustorb::Struct
    Union = Rustorb::Union
  end
end
