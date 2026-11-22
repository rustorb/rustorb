# frozen_string_literal: true

require_relative "lib/rustorb/version"

Gem::Specification.new do |s|
  s.name = "rustorb"
  s.version = Rustorb::VERSION
  s.summary = "CORBA for Ruby (the OMG Ruby Language Mapping) on the rustorb ORB"
  s.description = <<~TEXT
    The Ruby binding of rustorb, a CORBA ORB written in Rust: CORBA, PortableServer
    and CosNaming as the OMG Ruby Language Mapping (RCLM) has them, over the rustorb
    core through its C ABI. Stubs and skeletons are generated with rustorb-idl -bruby.
  TEXT
  s.homepage = "https://github.com/rustorb/rustorb"
  s.licenses = ["MIT", "Apache-2.0"]
  s.authors = ["rustorb contributors"]
  s.required_ruby_version = ">= 3.0"
  s.files = Dir["lib/**/*.rb", "ext/rustorb_ext/*.{c,rb}", "README.md"]
  s.extensions = ["ext/rustorb_ext/extconf.rb"]
  s.require_paths = ["lib"]
  s.add_dependency "bigdecimal"
  s.metadata = {
    "source_code_uri" => "https://github.com/rustorb/rustorb/tree/main/ruby",
    "documentation_uri" => "https://github.com/rustorb/rustorb/blob/main/docs/ruby-binding.md"
  }
end
