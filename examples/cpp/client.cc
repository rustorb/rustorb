// Looks "Echo" up in the naming service and calls it.
//
//   ./client -ORBInitRef NameService=corbaloc::localhost:2809/NameService [--shutdown]

#include <iostream>
#include <string>

#include "CosNaming.hh"
#include "Echo.hh"

int main(int argc, char* argv[]) {
  try {
    auto orb = CORBA::ORB_init(argc, argv);
    bool shutdown = false;
    for (int i = 1; i < argc; ++i) {
      if (std::string(argv[i]) == "--shutdown") shutdown = true;
    }

    auto root = IDL::traits<CosNaming::NamingContextExt>::narrow(
        orb->resolve_initial_references("NameService"));
    auto echo = IDL::traits<Example::Echo>::narrow(root->resolve_str("Echo"));

    std::cout << "client: " << echo->echo("hello from C++") << std::endl;
    if (shutdown) echo->shutdown();
    orb->destroy();
  } catch (const CORBA::Exception& e) {
    std::cerr << "client: " << e << std::endl;
    return 1;
  }
  return 0;
}
