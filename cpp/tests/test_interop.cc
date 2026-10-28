// The interop test cases (interop/impls/rustorbcpp/cases.h) in one process:
// a server ORB with the Echo servant on one thread and a client ORB calling
// it over IIOP on the loopback interface, by stringified reference.

#include <iostream>
#include <thread>

#include "cases.h"
#include "echo_impl.h"

int main() {
  try {
    int sargc = 3;
    char sarg0[] = "server", sarg1[] = "-ORBendPoint", sarg2[] = "giop:tcp:127.0.0.1:";
    char* sargv[] = {sarg0, sarg1, sarg2, nullptr};
    auto server_orb = CORBA::ORB_init(sargc, sargv);
    auto root = IDL::traits<PortableServer::POA>::narrow(
        server_orb->resolve_initial_references("RootPOA"));
    root->the_POAManager()->activate();
    auto servant = CORBA::make_reference<EchoImpl>(server_orb);
    auto oid = root->activate_object(servant);
    std::string ior = server_orb->object_to_string(root->id_to_reference(oid));
    std::thread server([&] { server_orb->run(); });

    int cargc = 1;
    char carg0[] = "client";
    char* cargv[] = {carg0, nullptr};
    auto client_orb = CORBA::ORB_init(cargc, cargv);
    auto client_poa = IDL::traits<PortableServer::POA>::narrow(
        client_orb->resolve_initial_references("RootPOA"));
    client_poa->the_POAManager()->activate();
    auto echo = IDL::traits<InteropTest::Echo>::narrow(client_orb->string_to_object(ior));
    if (!echo) {
      std::cout << "not an InteropTest::Echo: " << ior << std::endl;
      return 1;
    }

    Runner r;
    run_cases(r, echo, client_poa);
    echo->shutdown();
    server.join();
    r.summary();

    server_orb->destroy();
    client_orb->destroy();
    return r.failed().empty() ? 0 : 1;
  } catch (const CORBA::Exception& e) {
    std::cerr << "CORBA exception: " << e << std::endl;
    return 1;
  }
}
