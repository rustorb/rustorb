// Serves Example::Echo and calls it through its stringified reference.

#include <iostream>
#include <string>

#include "Echo.hh"

class EchoImpl : public CORBA::servant_traits<Example::Echo>::base_type {
 public:
  std::string echo(const std::string& s) override { return s; }
};

int main(int argc, char* argv[]) {
  auto orb = CORBA::ORB_init(argc, argv);
  auto poa = IDL::traits<PortableServer::POA>::narrow(
      orb->resolve_initial_references("RootPOA"));
  poa->the_POAManager()->activate();
  auto servant = CORBA::make_reference<EchoImpl>();
  std::string ior = orb->object_to_string(servant->_this());

  auto echo = IDL::traits<Example::Echo>::narrow(orb->string_to_object(ior));
  std::string got = echo->echo("hello");
  std::cout << got << std::endl;
  orb->destroy();
  return got == "hello" ? 0 : 1;
}
