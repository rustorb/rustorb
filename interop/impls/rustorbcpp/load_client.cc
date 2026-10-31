// rustorb C++ load client for interop/run-load-local.py: calls an Echo
// server (resolved from the naming service) as fast as it can for a while
// and prints the rate of each phase.
//
//   load_client --naming CORBALOC --name PATH [--seconds S] [--threads N]
//               [--timeout S] [ORB options]
//
// The phases, each --seconds long, are those of the Rust load client
// (impls/rustorb/src/load_client.rs) but for `connections`: `small`
// (echo_long from N threads on one connection), `string` (echo_string of
// 100 characters) and `large` (echo_octets of 1 MiB from 4 threads). Each
// line reads `PHASE calls=N errors=N rate=N/s`; a failed call makes the exit
// status 1. When the Echo object is not found within --timeout seconds (30
// by default) the exit status is 2.

#include <atomic>
#include <chrono>
#include <cstdio>
#include <functional>

#include "common.h"

using EchoRef = IDL::traits<InteropTest::Echo>::ref_type;

// Runs call from `workers` threads until `seconds` pass and prints the
// totals. Returns whether every call succeeded.
static bool phase(const char* label, double seconds, int workers,
                  const std::function<bool(long)>& call) {
  std::atomic<bool> stop{false};
  std::atomic<long> calls{0};
  std::atomic<long> errors{0};
  auto start = std::chrono::steady_clock::now();
  std::vector<std::thread> threads;
  for (int w = 0; w < workers; ++w) {
    threads.emplace_back([&]() {
      long i = 0;
      while (!stop.load(std::memory_order_relaxed)) {
        bool ok;
        try {
          ok = call(i);
        } catch (const CORBA::Exception& e) {
          std::cerr << label << ": " << e << std::endl;
          ok = false;
        }
        (ok ? calls : errors).fetch_add(1, std::memory_order_relaxed);
        ++i;
      }
    });
  }
  std::this_thread::sleep_for(std::chrono::duration<double>(seconds));
  stop = true;
  for (auto& t : threads) t.join();
  double took = std::chrono::duration<double>(std::chrono::steady_clock::now() - start).count();
  std::printf("%s calls=%ld errors=%ld rate=%.0f/s\n", label, calls.load(), errors.load(),
              static_cast<double>(calls.load()) / took);
  std::fflush(stdout);
  return errors.load() == 0;
}

int main(int argc, char** argv) {
  // --seconds and --threads are this client's; the rest go to parse_args.
  double seconds = 5;
  int threads = 8;
  std::vector<char*> rest{argv[0]};
  for (int i = 1; i < argc; ++i) {
    std::string a = argv[i];
    if ((a == "--seconds" || a == "--threads") && i + 1 < argc) {
      const char* v = argv[++i];
      if (a == "--seconds") seconds = std::atof(v);
      else threads = std::atoi(v);
    } else {
      rest.push_back(argv[i]);
    }
  }
  std::vector<std::string> orb_args;
  Options opt = parse_args(static_cast<int>(rest.size()), rest.data(), orb_args);
  try {
    auto orb = orb_init(orb_args);
    auto root = root_context(orb, opt.timeout);
    CosNaming::Name name = to_name(opt.name);
    EchoRef echo = retry("the Echo server", [&]() {
      auto e = IDL::traits<InteropTest::Echo>::narrow(root->resolve(name));
      if (!e) throw CORBA::OBJECT_NOT_EXIST();
      e->echo_long(1);
      return e;
    }, opt.timeout, 2);

    bool ok = phase("small", seconds, threads, [&](long i) {
      return echo->echo_long(static_cast<int32_t>(i)) == static_cast<int32_t>(i);
    });
    const std::string text(100, 'x');
    ok &= phase("string", seconds, threads, [&](long) { return echo->echo_string(text) == text; });
    InteropTest::OctetSeq data(1 << 20);
    for (size_t i = 0; i < data.size(); ++i) data[i] = static_cast<uint8_t>(i);
    ok &= phase("large", seconds, 4, [&](long) { return echo->echo_octets(data) == data; });
    orb->destroy();
    return ok ? 0 : 1;
  } catch (const CORBA::Exception& e) {
    std::cerr << "CORBA exception: " << e << std::endl;
    return 1;
  }
}
