# frozen_string_literal: true

# What the rustorb Ruby client, server and naming check share: the options
# of every implementation, ORB_init with the settings of the interop
# scripts, and the naming service with retries (as impls/rustorbpy/common.py).
#
# build.sh puts the rustorb runtime (with the extension built in the
# interop image) in lib/ and the code of rustorb-idl -bruby in gen/.
#
# Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

here = File.expand_path("..", __dir__)
$LOAD_PATH.unshift(File.join(here, "gen"), File.join(here, "lib"), File.join(here, "rt"))

require "corba"
require "CosNamingC"

if ENV["RUSTORB_DII"] && !ENV["RUSTORB_DII"].empty?
  # rustorbrb-dii: every call through the DII, every servant through the DSI.
  require "dii_adapters"
  DiiAdapters.install
end

module Interop
  OPTIONS = {
    "--naming" => :naming, "--name" => :name, "--host" => :host, "--port" => :port,
    "--timeout" => :timeout, "--suite" => :suite
  }.freeze

  module_function

  # [options, ORB arguments]: --naming, --name, --host, --port, --timeout,
  # --suite and the flags named in flags; every other argument is the ORB's.
  def parse_args(argv, flags = [])
    opts = { host: nil, port: 0, timeout: 30.0, suite: "basic" }
    rest = []
    args = argv.dup
    until args.empty?
      a = args.shift
      key, value = a.split("=", 2)
      if OPTIONS.key?(key)
        value ||= args.shift or abort("#{key} needs a value")
        opts[OPTIONS[key]] = value
      elsif flags.include?(a)
        opts[a.sub(/\A--/, "").tr("-", "_").to_sym] = true
      else
        rest << a
      end
    end
    opts[:port] = Integer(opts[:port])
    opts[:timeout] = Float(opts[:timeout])
    [opts, rest]
  end

  def orb_init(opts, orb_args)
    argv = orb_args + ["-ORBInitRef", "NameService=#{opts[:naming]}"]
    max_giop = ENV["ORBmaxGIOPVersion"]
    argv += ["-ORBmaxGIOPVersion", max_giop] if max_giop && !max_giop.empty?
    frag = ENV["INTEROP_FRAGMENT_SIZE"]
    argv += ["-ORBgiopFragmentSize", frag] if frag && !frag.empty?
    ssl = ENV["INTEROP_SSL"]
    ssl = nil if ssl&.empty?
    if ssl
      argv += ["-ORBsslCAFile", File.join(ssl, "ca.pem"),
               "-ORBsslKeyFile", File.join(ssl, "server-bundle.pem"),
               "-ORBsslVerifyClient", "required"]
    end
    unix = ENV["INTEROP_UNIX"]
    unix = nil if unix&.empty?
    if opts[:port].positive? && unix
      argv += ["-ORBendPoint", "giop:unix:#{unix}/#{opts[:host]}-#{opts[:port]}.sock"]
    elsif opts[:port].positive?
      argv += ["-ORBendPoint", "giop:#{ssl ? 'ssl' : 'tcp'}:#{opts[:host]}:#{opts[:port]}"]
    end
    CORBA.ORB_init(argv)
  end

  def to_name(path)
    path.split("/").reject(&:empty?).map { |c| CosNaming::NameComponent.new(c, "") }
  end

  def now = Process.clock_gettime(Process::CLOCK_MONOTONIC)

  RETRY = [CORBA::TRANSIENT, CORBA::COMM_FAILURE, CORBA::OBJECT_NOT_EXIST,
           CosNaming::NamingContext::NotFound].freeze

  # Calls the block until it succeeds or timeout seconds have passed.
  def retry_for(what, timeout)
    deadline = now + timeout
    begin
      yield
    rescue *RETRY
      if now >= deadline
        warn "timed out waiting for #{what}"
        raise
      end
      sleep 0.2
      retry
    end
  end

  def root_context(orb, timeout)
    retry_for("the naming service", timeout) do
      obj = orb.resolve_initial_references("NameService")
      raise CORBA::TRANSIENT if obj.nil?

      CosNaming::NamingContext._narrow(obj)
    end
  end

  # A PERSISTENT / USER_ID POA (fixed object keys, so that GIOP captures
  # are the same from run to run).
  def persistent_poa(orb, name)
    root = orb.resolve_initial_references("RootPOA")
    poa = root.create_POA(name, root.the_POAManager, [
      root.create_lifespan_policy(PortableServer::PERSISTENT),
      root.create_id_assignment_policy(PortableServer::USER_ID)
    ])
    root.the_POAManager.activate
    poa
  end
end
