// Serves Example::Echo and binds it in the naming service as "Echo".
//
//   ./server -ORBInitRef NameService=corbaloc::localhost:2809/NameService

#include <iostream>
#include <string>

#include "CosNaming.hh"
#include "Echo.hh"

class EchoImpl : public CORBA::servant_traits<Example::Echo>::base_type {
 public:
  explicit EchoImpl(IDL::traits<CORBA::ORB>::ref_type orb) : orb_(std::move(orb)) {}

  std::string echo(const std::string& message) override {
    std::cout << "server: " << message << std::endl;
    return message;
  }

  void shutdown() override { orb_->shutdown(false); }

 private:
  IDL::traits<CORBA::ORB>::ref_type orb_;
};

int main(int argc, char* argv[]) {
  try {
    auto orb = CORBA::ORB_init(argc, argv);
    auto poa = IDL::traits<PortableServer::POA>::narrow(
        orb->resolve_initial_references("RootPOA"));
    poa->the_POAManager()->activate();
    auto echo = CORBA::make_reference<EchoImpl>(orb)->_this();

    auto root = IDL::traits<CosNaming::NamingContext>::narrow(
        orb->resolve_initial_references("NameService"));
    root->rebind(CosNaming::Name{CosNaming::NameComponent("Echo", "")}, echo);
    std::cout << "server: ready" << std::endl;

    orb->run();
    orb->destroy();
  } catch (const CORBA::Exception& e) {
    std::cerr << "server: " << e << std::endl;
    return 1;
  }
  return 0;
}
