// Client call timeouts of the C++ binding (rustorb::setClientCallTimeout
// and friends), against a server of the test's own that reads requests and
// never answers them.

#include <arpa/inet.h>
#include <netinet/in.h>
#include <sys/socket.h>
#include <unistd.h>

#include <atomic>
#include <chrono>
#include <iostream>
#include <string>
#include <thread>

#include "rustorb/corba.h"

static int failures = 0;

#define CHECK(cond)                                                              \
  do {                                                                           \
    if (!(cond)) {                                                               \
      std::cout << "FAIL line " << __LINE__ << ": " #cond << std::endl;          \
      ++failures;                                                                \
    }                                                                            \
  } while (0)

// Accepts connections and counts the CancelRequest messages read.
class Sink {
 public:
  Sink() {
    fd_ = socket(AF_INET, SOCK_STREAM, 0);
    sockaddr_in addr{};
    addr.sin_family = AF_INET;
    addr.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    bind(fd_, reinterpret_cast<sockaddr*>(&addr), sizeof addr);
    listen(fd_, 16);
    socklen_t len = sizeof addr;
    getsockname(fd_, reinterpret_cast<sockaddr*>(&addr), &len);
    port_ = ntohs(addr.sin_port);
    std::thread([this] {
      for (;;) {
        int c = accept(fd_, nullptr, nullptr);
        if (c < 0) return;
        std::thread([this, c] { read_messages(c); }).detach();
      }
    }).detach();
  }
  int port() const { return port_; }
  int cancels() const { return cancels_; }

 private:
  void read_messages(int c) {
    std::string buf;
    char chunk[4096];
    for (;;) {
      ssize_t n = recv(c, chunk, sizeof chunk, 0);
      if (n <= 0) return;
      buf.append(chunk, static_cast<size_t>(n));
      while (buf.size() >= 12) {
        auto b = [&](size_t i) { return static_cast<uint32_t>(static_cast<unsigned char>(buf[i])); };
        uint32_t size = (b(6) & 1) ? b(8) | b(9) << 8 | b(10) << 16 | b(11) << 24
                                   : b(11) | b(10) << 8 | b(9) << 16 | b(8) << 24;
        if (buf.size() < 12 + size) break;
        if (b(7) == 2) ++cancels_;
        buf.erase(0, 12 + size);
      }
    }
  }

  int fd_;
  int port_;
  std::atomic<int> cancels_{0};
};

using ObjectRef = CORBA::object_reference<CORBA::Object>;

// Milliseconds until _non_existent on obj throws TIMEOUT; -1 if it throws
// something else.
static long time_out(const ObjectRef& obj) {
  auto start = std::chrono::steady_clock::now();
  try {
    obj->_non_existent();
  } catch (const CORBA::TIMEOUT& e) {
    if (e.completed() != CORBA::CompletionStatus::COMPLETED_MAYBE) return -1;
    return static_cast<long>(std::chrono::duration_cast<std::chrono::milliseconds>(
                                 std::chrono::steady_clock::now() - start)
                                 .count());
  } catch (const CORBA::Exception& e) {
    std::cout << "unexpected exception: " << e << std::endl;
  }
  return -1;
}

static bool about(long ms, long expected) { return ms >= expected && ms < expected + 400; }

int main(int argc, char* argv[]) {
  auto orb = CORBA::ORB_init(argc, argv);
  Sink sink;
  std::string url = "corbaloc::127.0.0.1:" + std::to_string(sink.port()) + "/Sink";
  ObjectRef obj = orb->string_to_object(url);

  rustorb::setClientCallTimeout(200);
  CHECK(about(time_out(obj), 200));
  rustorb::setClientCallTimeout(orb, 30000);

  // The thread's timeout overrides the ORB's, on that thread only.
  rustorb::setClientThreadCallTimeout(300);
  CHECK(about(time_out(obj), 300));
  long other = 0;
  std::thread([&] {
    rustorb::setClientThreadCallTimeout(100);
    other = time_out(obj);
  }).join();
  CHECK(about(other, 100));

  // The reference's overrides the thread's, also for a narrowed copy; 0
  // unsets it.
  rustorb::setClientCallTimeout(obj, 100);
  CHECK(about(time_out(IDL::traits<CORBA::Object>::narrow(obj)), 100));
  CHECK(about(time_out(orb->string_to_object(url)), 300));
  rustorb::setClientCallTimeout(obj, 0);
  CHECK(about(time_out(obj), 300));
  rustorb::setClientThreadCallTimeout(0);
  rustorb::setClientCallTimeout(200);
  CHECK(about(time_out(obj), 200));

  std::this_thread::sleep_for(std::chrono::milliseconds(200));
  CHECK(sink.cancels() == 7);

  rustorb::setClientConnectTimeout(orb, 1000);
  CHECK(rorb_orb_connect_timeout(orb->_rorb_handle()) == 1000000);
  rustorb::setClientConnectTimeout(0);
  CHECK(rorb_orb_connect_timeout(orb->_rorb_handle()) == 0);
  bool bad_param = false;
  try {
    rustorb::setClientCallTimeout(ObjectRef(), 1);
  } catch (const CORBA::BAD_PARAM&) {
    bad_param = true;
  }
  CHECK(bad_param);

  orb->destroy();
  if (failures) {
    std::cout << failures << " failure(s)" << std::endl;
    return 1;
  }
  std::cout << "all timeout tests passed" << std::endl;
  return 0;
}
