# frozen_string_literal: true

# Servant managers (PortableServer::ServantActivator, ServantLocator) and
# adapter activators (PortableServer::AdapterActivator): local objects the
# application derives from, which the POA calls on its upcall threads
# (through the rustorb_ext callbacks, with the GVL).
#
# Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

module PortableServer
  # Raised by incarnate or preinvoke to send the client to another object.
  class ForwardRequest < CORBA::UserException
    ID = "IDL:omg.org/PortableServer/ForwardRequest:1.0"
    MEMBERS = %w[forward_reference].freeze
    attr_accessor :forward_reference
  end

  # What the C side makes of a servant manager's answer.
  module ManagerOutcome
    module_function

    def servant(s, key = nil)
      unless s.is_a?(Servant)
        return [:system, CORBA::OBJ_ADAPTER::ID, 0, CORBA::COMPLETED_NO] if s.nil?

        return [:system, CORBA::BAD_PARAM::ID, 0, CORBA::COMPLETED_NO]
      end

      PortableServer.ensure_dispatchers
      ids = s.respond_to?(:_ids) ? s._ids : s.class::IDS
      [:servant, s, ids, key || 0]
    end

    def raised(e)
      case e
      when ForwardRequest
        ref = e.forward_reference
        return [:system, CORBA::BAD_PARAM::ID, 0, CORBA::COMPLETED_NO] unless ref.is_a?(CORBA::Object)

        [:forward, ref._native_ref]
      when CORBA::SystemException then [:system, e._id, e.minor, e.completed]
      else
        warn "rustorb: servant manager raised #{e.class}: #{e.message}" if $VERBOSE
        [:system, CORBA::UNKNOWN::ID, 0, CORBA::COMPLETED_MAYBE]
      end
    end
  end

  # The base of ServantActivator and ServantLocator.
  class ServantManager
    def _is_a?(id) = self.class.ancestors.any? { |k| k.const_defined?(:ID, false) && k::ID == id }
    alias _is_a _is_a?
  end

  # incarnate(oid, adapter) returns the servant of the object (or raises
  # ForwardRequest or a system exception); etherealize(oid, adapter,
  # servant, cleanup_in_progress, remaining_activations) is told when the
  # POA lets it go. For a POA with USE_SERVANT_MANAGER and RETAIN.
  class ServantActivator < ServantManager
    ID = "IDL:omg.org/PortableServer/ServantActivator:1.0"

    def incarnate(_oid, _adapter)
      raise CORBA::NO_IMPLEMENT.new("#{self.class}#incarnate", 0, CORBA::COMPLETED_NO)
    end

    def etherealize(_oid, _adapter, _servant, _cleanup_in_progress, _remaining_activations); end

    def _rustorb_incarnate(oid, poa)
      ManagerOutcome.servant(incarnate(oid, POA._wrap(poa)))
    rescue ::Exception => e # rubocop:disable Lint/RescueException - every failure is an answer
      raise if e.is_a?(SystemExit) || e.is_a?(Interrupt)

      ManagerOutcome.raised(e)
    end

    def _rustorb_etherealize(oid, poa, servant, cleanup, remaining)
      etherealize(oid, POA._wrap(poa), servant, cleanup, remaining)
    rescue StandardError => e
      warn "rustorb: etherealize raised #{e.class}: #{e.message}" if $VERBOSE
    end
  end

  # preinvoke(oid, adapter, operation) returns the servant of this request
  # and a cookie ([servant, cookie], or the servant alone for a nil
  # cookie), or raises ForwardRequest or a system exception;
  # postinvoke(oid, adapter, operation, cookie, servant) follows the
  # request. For a POA with USE_SERVANT_MANAGER and NON_RETAIN.
  class ServantLocator < ServantManager
    ID = "IDL:omg.org/PortableServer/ServantLocator:1.0"

    def preinvoke(_oid, _adapter, _operation)
      raise CORBA::NO_IMPLEMENT.new("#{self.class}#preinvoke", 0, CORBA::COMPLETED_NO)
    end

    def postinvoke(_oid, _adapter, _operation, _cookie, _servant); end

    def _rustorb_preinvoke(oid, poa, operation)
      r = preinvoke(oid, POA._wrap(poa), operation)
      servant, cookie = r.is_a?(Array) ? r : [r, nil]
      key = 0
      unless cookie.nil?
        @__rustorb_cookie_lock ||= Mutex.new
        @__rustorb_cookie_lock.synchronize do
          @__rustorb_cookies ||= {}
          @__rustorb_cookie_next = (@__rustorb_cookie_next || 0) + 1
          key = @__rustorb_cookie_next
          @__rustorb_cookies[key] = cookie
        end
      end
      ManagerOutcome.servant(servant, key)
    rescue ::Exception => e # rubocop:disable Lint/RescueException - every failure is an answer
      raise if e.is_a?(SystemExit) || e.is_a?(Interrupt)

      ManagerOutcome.raised(e)
    end

    def _rustorb_postinvoke(oid, poa, operation, key, servant)
      cookie = nil
      if key != 0
        @__rustorb_cookie_lock.synchronize { cookie = @__rustorb_cookies.delete(key) }
      end
      postinvoke(oid, POA._wrap(poa), operation, cookie, servant)
    rescue StandardError => e
      warn "rustorb: postinvoke raised #{e.class}: #{e.message}" if $VERBOSE
    end
  end

  # unknown_adapter(parent, name) creates the POA name under parent and
  # returns true, or returns false.
  class AdapterActivator
    ID = "IDL:omg.org/PortableServer/AdapterActivator:1.0"

    def _is_a?(id) = id == ID
    alias _is_a _is_a?

    def unknown_adapter(_parent, _name) = false

    def _rustorb_unknown_adapter(parent, name)
      unknown_adapter(POA._wrap(parent), name) ? true : false
    rescue StandardError => e
      warn "rustorb: unknown_adapter raised #{e.class}: #{e.message}" if $VERBOSE
      false
    end
  end

  class POA
    # A ServantActivator (RETAIN) or ServantLocator (NON_RETAIN).
    def set_servant_manager(manager)
      PortableServer.ensure_dispatchers
      case manager
      when ServantActivator then @native.set_servant_activator(manager)
      when ServantLocator then @native.set_servant_locator(manager)
      else raise CORBA::BAD_PARAM.new("not a ServantActivator or ServantLocator", 0, CORBA::COMPLETED_NO)
      end
      nil
    end

    def get_servant_manager = @native.get_servant_manager

    def the_activator = @native.the_activator

    def the_activator=(activator)
      unless activator.nil? || activator.is_a?(AdapterActivator)
        raise CORBA::BAD_PARAM.new("not an AdapterActivator", 0, CORBA::COMPLETED_NO)
      end

      PortableServer.ensure_dispatchers
      @native.set_the_activator(activator)
    end
    alias _set_the_activator the_activator=
  end
end
