// omniORB C++ program of the rustorb / omniORB comparison benchmark
// (interop/bench, docs/omniorb-comparison.md). The same cases, options and
// output as the Rust program (interop/impls/rustorb/src/bench.rs); the
// rustorb C++ program (../rustorb-cpp/bench.cc) is this one in the IDL to
// C++11 mapping.
//
//   bench serve  [ORB options]                 serves Bench::Target, prints its IOR
//   bench client --ior IOR [options] [ORB options]
//   bench inproc [options] [ORB options]       servant and caller in one ORB

#include <algorithm>
#include <atomic>
#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <functional>
#include <iostream>
#include <stdexcept>
#include <string>
#include <thread>
#include <vector>

#include "Bench.hh"

using Clock = std::chrono::steady_clock;

class TargetImpl : public POA_Bench::Target {
 public:
  explicit TargetImpl(CORBA::ORB_ptr orb) : orb_(CORBA::ORB::_duplicate(orb)) {}
  void ping() override {}
  CORBA::Long echo_long(CORBA::Long v) override { return v; }
  char* echo_string(const char* v) override { return CORBA::string_dup(v); }
  Bench::Octets* echo_octets(const Bench::Octets& v) override { return new Bench::Octets(v); }
  CORBA::ULong put_octets(const Bench::Octets& v) override { return v.length(); }
  Bench::Octets* get_octets(CORBA::ULong n) override {
    Bench::Octets* r = new Bench::Octets(n);
    r->length(n);
    if (n) std::fill(r->get_buffer(), r->get_buffer() + n, CORBA::Octet(7));
    return r;
  }
  Bench::Samples* echo_samples(const Bench::Samples& v) override { return new Bench::Samples(v); }
  CORBA::Any* echo_any(const CORBA::Any& v) override { return new CORBA::Any(v); }
  void post(CORBA::Long) override { posted_.fetch_add(1, std::memory_order_relaxed); }
  CORBA::ULong take_posted() override { return posted_.exchange(0, std::memory_order_relaxed); }
  void shutdown() override { orb_->shutdown(false); }

 private:
  CORBA::ORB_var orb_;
  std::atomic<CORBA::ULong> posted_{0};
};

struct Opts {
  std::string mode, ior;
  std::vector<std::string> cases;
  double seconds = 2, warmup = 0.3;
};

static const char* kDefaultCases =
    "ping,long,string,octets:64,octets:1024,octets:16384,octets:262144,octets:1048576,"
    "octets:8388608,put:1048576,get:1048576,samples:100,any,oneway,threads:4";

static std::vector<std::string> split(const std::string& s) {
  std::vector<std::string> out;
  size_t i = 0;
  while (i <= s.size()) {
    size_t j = s.find(',', i);
    if (j == std::string::npos) j = s.size();
    if (j > i) out.push_back(s.substr(i, j - i));
    i = j + 1;
  }
  return out;
}

static double secs(Clock::duration d) { return std::chrono::duration<double>(d).count(); }

static void report(const std::string& c, std::vector<double>& lat, double took, double bytes,
                   size_t warm) {
  std::sort(lat.begin(), lat.end());
  size_t n = lat.size();
  auto pct = [&](double p) { return lat[static_cast<size_t>((n - 1) * p)]; };
  double sum = 0;
  for (double v : lat) sum += v;
  double rate = n / took;
  std::printf(
      "{\"case\":\"%s\",\"calls\":%zu,\"seconds\":%.3f,\"rate\":%.1f,\"mean_us\":%.2f,"
      "\"p50_us\":%.2f,\"p90_us\":%.2f,\"p99_us\":%.2f,\"max_us\":%.2f,\"mb_per_s\":%.2f,"
      "\"total_calls\":%zu}\n",
      c.c_str(), n, took, rate, sum / n, pct(0.5), pct(0.9), pct(0.99), pct(1.0),
      rate * bytes / 1e6, n + warm);
  std::fflush(stdout);
}

// Calls `call` until o.seconds pass (after o.warmup), timing each call.
static void timed(const std::string& c, const Opts& o, double bytes,
                  const std::function<void()>& call) {
  auto warm_end = Clock::now() + std::chrono::duration_cast<Clock::duration>(
                                     std::chrono::duration<double>(o.warmup));
  size_t warm = 0;
  for (; Clock::now() < warm_end; ++warm) call();
  std::vector<double> lat;
  lat.reserve(1 << 16);
  auto start = Clock::now();
  auto end = start + std::chrono::duration_cast<Clock::duration>(
                         std::chrono::duration<double>(o.seconds));
  for (;;) {
    auto t = Clock::now();
    call();
    auto now = Clock::now();
    lat.push_back(secs(now - t) * 1e6);
    if (now >= end) break;
  }
  report(c, lat, secs(Clock::now() - start), bytes, warm);
}

static void check(bool ok, const char* what) {
  if (!ok) throw std::runtime_error(std::string(what) + " mismatch");
}

static void run_case(Bench::Target_ptr t, const std::string& c, const Opts& o) {
  std::string name = c, arg;
  size_t colon = c.find(':');
  if (colon != std::string::npos) {
    name = c.substr(0, colon);
    arg = c.substr(colon + 1);
  }
  CORBA::ULong n = arg.empty() ? 0 : static_cast<CORBA::ULong>(std::stoul(arg));
  if (name == "first") {
    // The first call on a fresh reference: the connection is made.
    auto t0 = Clock::now();
    t->ping();
    double us = secs(Clock::now() - t0) * 1e6;
    std::printf("{\"case\":\"first\",\"calls\":1,\"p50_us\":%.2f,\"mean_us\":%.2f}\n", us, us);
    std::fflush(stdout);
  } else if (name == "ping") {
    timed(c, o, 0, [&] { t->ping(); });
  } else if (name == "long") {
    CORBA::Long i = 0;
    timed(c, o, 0, [&] {
      ++i;
      check(t->echo_long(i) == i, "echo_long");
    });
  } else if (name == "string") {
    std::string s(n ? n : 100, 'x');
    timed(c, o, 0, [&] {
      CORBA::String_var r = t->echo_string(s.c_str());
      check(std::strlen(r.in()) == s.size(), "echo_string");
    });
  } else if (name == "octets" || name == "put") {
    Bench::Octets v(n);
    v.length(n);
    if (n) std::fill(v.get_buffer(), v.get_buffer() + n, CORBA::Octet(1));
    if (name == "octets") {
      timed(c, o, 2.0 * n, [&] {
        Bench::Octets_var r = t->echo_octets(v);
        check(r->length() == n, "echo_octets");
      });
    } else {
      timed(c, o, n, [&] { check(t->put_octets(v) == n, "put_octets"); });
    }
  } else if (name == "get") {
    timed(c, o, n, [&] {
      Bench::Octets_var r = t->get_octets(n);
      check(r->length() == n, "get_octets");
    });
  } else if (name == "samples") {
    Bench::Samples v(n);
    v.length(n);
    for (CORBA::ULong i = 0; i < n; ++i) {
      v[i].id = static_cast<CORBA::Long>(i);
      v[i].x = 1.5;
      v[i].y = 2.5;
      v[i].z = 3.5;
      v[i].name = CORBA::string_dup(("sample-" + std::to_string(i)).c_str());
    }
    timed(c, o, 0, [&] {
      Bench::Samples_var r = t->echo_samples(v);
      check(r->length() == n, "echo_samples");
    });
  } else if (name == "any") {
    Bench::Sample s;
    s.id = 1;
    s.x = 1.5;
    s.y = 2.5;
    s.z = 3.5;
    s.name = CORBA::string_dup("sample");
    CORBA::Any a;
    a <<= s;
    timed(c, o, 0, [&] {
      CORBA::Any_var r = t->echo_any(a);
      const Bench::Sample* p;
      check((r.in() >>= p) && p->id == 1, "echo_any");
    });
  } else if (name == "oneway") {
    t->take_posted();
    auto start = Clock::now();
    auto end = start + std::chrono::duration_cast<Clock::duration>(
                           std::chrono::duration<double>(o.seconds));
    unsigned long sent = 0;
    while (Clock::now() < end) {
      for (int k = 0; k < 64; ++k) t->post(static_cast<CORBA::Long>(sent++));
    }
    double send_took = secs(Clock::now() - start);
    unsigned long got = t->take_posted();
    auto deadline = Clock::now() + std::chrono::seconds(10);
    while (got < sent && Clock::now() < deadline) {
      std::this_thread::sleep_for(std::chrono::milliseconds(1));
      got += t->take_posted();
    }
    double took = secs(Clock::now() - start);
    std::printf(
        "{\"case\":\"%s\",\"calls\":%lu,\"seconds\":%.3f,\"rate\":%.1f,\"send_rate\":%.1f,"
        "\"received\":%lu,\"total_calls\":%lu}\n",
        c.c_str(), sent, took, sent / took, sent / send_took, got, sent);
    std::fflush(stdout);
    if (got != sent) throw std::runtime_error("oneway: lost calls");
  } else if (name == "threads") {
    std::atomic<bool> stop{false}, failed{false};
    std::atomic<unsigned long> calls{0};
    std::vector<std::thread> hs;
    for (CORBA::ULong w = 0; w < std::max<CORBA::ULong>(n, 1); ++w) {
      hs.emplace_back([&, w] {
        CORBA::Long i = static_cast<CORBA::Long>(w) * 1000000;
        try {
          while (!stop.load(std::memory_order_relaxed)) {
            ++i;
            if (t->echo_long(i) != i) {
              failed = true;
              break;
            }
            calls.fetch_add(1, std::memory_order_relaxed);
          }
        } catch (...) {
          failed = true;
        }
      });
    }
    std::this_thread::sleep_for(std::chrono::duration<double>(o.warmup));
    unsigned long c0 = calls.load();
    auto start = Clock::now();
    std::this_thread::sleep_for(std::chrono::duration<double>(o.seconds));
    unsigned long c1 = calls.load();
    double took = secs(Clock::now() - start);
    stop = true;
    for (auto& h : hs) h.join();
    if (failed) throw std::runtime_error("threads: a call failed");
    std::printf(
        "{\"case\":\"%s\",\"calls\":%lu,\"seconds\":%.3f,\"rate\":%.1f,\"total_calls\":%lu}\n",
        c.c_str(), c1 - c0, took, (c1 - c0) / took, calls.load());
    std::fflush(stdout);
  } else {
    throw std::runtime_error("unknown case " + c);
  }
}

int main(int argc, char** argv) {
  if (argc < 2) {
    std::cerr << "usage: bench serve|client|inproc [options]" << std::endl;
    return 2;
  }
  Opts o;
  o.mode = argv[1];
  std::string cases = kDefaultCases;
  std::vector<char*> rest{argv[0]};
  for (int i = 2; i < argc; ++i) {
    std::string a = argv[i];
    if ((a == "--ior" || a == "--cases" || a == "--seconds" || a == "--warmup") && i + 1 < argc) {
      std::string v = argv[++i];
      if (a == "--ior") o.ior = v;
      else if (a == "--cases") cases = v;
      else if (a == "--seconds") o.seconds = std::atof(v.c_str());
      else o.warmup = std::atof(v.c_str());
    } else {
      rest.push_back(argv[i]);
    }
  }
  o.cases = split(cases);
  int orb_argc = static_cast<int>(rest.size());
  rest.push_back(nullptr);
  try {
    CORBA::ORB_var orb = CORBA::ORB_init(orb_argc, rest.data());
    Bench::Target_var target;
    if (o.mode == "serve" || o.mode == "inproc") {
      CORBA::Object_var obj = orb->resolve_initial_references("RootPOA");
      PortableServer::POA_var poa = PortableServer::POA::_narrow(obj);
      PortableServer::Servant_var<TargetImpl> servant = new TargetImpl(orb);
      PortableServer::ObjectId_var id = poa->activate_object(servant);
      CORBA::Object_var ref = poa->id_to_reference(id);
      poa->the_POAManager()->activate();
      target = Bench::Target::_narrow(ref);
      if (o.mode == "serve") {
        CORBA::String_var ior = orb->object_to_string(ref);
        std::printf("%s\n", ior.in());
        std::fflush(stdout);
        orb->run();
        orb->destroy();
        return 0;
      }
    } else if (o.mode == "client") {
      CORBA::Object_var obj = orb->string_to_object(o.ior.c_str());
      target = Bench::Target::_unchecked_narrow(obj);
    } else {
      std::cerr << "unknown mode " << o.mode << std::endl;
      return 2;
    }
    for (const auto& c : o.cases) {
      try {
        run_case(target, c, o);
      } catch (const CORBA::Exception& e) {
        std::cerr << "bench: " << c << ": " << e._name() << std::endl;
        return 1;
      } catch (const std::exception& e) {
        std::cerr << "bench: " << c << ": " << e.what() << std::endl;
        return 1;
      }
    }
    orb->destroy();
  } catch (const CORBA::Exception& e) {
    std::cerr << "bench: " << e._name() << std::endl;
    return 1;
  }
  return 0;
}
