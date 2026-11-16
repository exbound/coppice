// Writes the .btlog oracle for spec §8 (F11, F12).
//
// For each self-authored scenario below, the harness ticks a tree under
// BehaviorTree.CPP 4.9.0 and writes three files into the output directory:
//
//   <name>.btlog        written by the library's own FileLogger2
//   <name>.xml          WriteTreeToXML(tree, true, true), the XML the logger
//                       embeds
//   <name>.transitions  one line per transition, "<ns> <uid> <status>", where
//                       <ns> is the absolute time in nanoseconds since the
//                       epoch, recorded by a second status logger on the same
//                       tree. Both loggers are notified with the same time
//                       point (BT 4.9.0 src/tree_node.cpp:189).
//
// Usage: btlog_harness <output-directory>

#include <behaviortree_cpp/bt_factory.h>
#include <behaviortree_cpp/loggers/abstract_logger.h>
#include <behaviortree_cpp/loggers/bt_file_logger_v2.h>
#include <behaviortree_cpp/xml_parsing.h>

#include <chrono>
#include <cstdint>
#include <filesystem>
#include <fstream>
#include <functional>
#include <iostream>
#include <ratio>
#include <sstream>
#include <string>
#include <thread>
#include <type_traits>
#include <vector>

namespace fs = std::filesystem;

// The logger takes T0 from system_clock and transition times from
// high_resolution_clock; the oracle assumes they are one clock, counting
// nanoseconds, as in GCC's libstdc++. Where high_resolution_clock is
// steady_clock (libc++, MSVC), this harness does not compile.
static_assert(std::is_same_v<std::chrono::high_resolution_clock, std::chrono::system_clock>);
static_assert(std::is_same_v<BT::Duration::period, std::nano>);

namespace
{

// An action that stays RUNNING for `ticks` ticks, then succeeds.
class Pending : public BT::StatefulActionNode
{
public:
  Pending(const std::string& name, const BT::NodeConfig& config)
    : BT::StatefulActionNode(name, config)
  {}

  static BT::PortsList providedPorts()
  {
    return { BT::InputPort<int>("ticks") };
  }

  BT::NodeStatus onStart() override
  {
    left_ = getInput<int>("ticks").value();
    return left_ > 0 ? BT::NodeStatus::RUNNING : BT::NodeStatus::SUCCESS;
  }

  BT::NodeStatus onRunning() override
  {
    --left_;
    return left_ > 0 ? BT::NodeStatus::RUNNING : BT::NodeStatus::SUCCESS;
  }

  void onHalted() override
  {}

private:
  int left_ = 0;
};

struct Row
{
  std::int64_t ns;
  std::uint16_t uid;
  int status;
};

// Records every transition with its absolute time, including those to IDLE.
class Recorder : public BT::StatusChangeLogger
{
public:
  explicit Recorder(BT::TreeNode* root) : BT::StatusChangeLogger(root)
  {
    setTimestampType(BT::TimestampType::absolute);
    enableTransitionToIdle(true);
  }

  void callback(BT::Duration timestamp, const BT::TreeNode& node, BT::NodeStatus,
                BT::NodeStatus status) override
  {
    rows.push_back({ static_cast<std::int64_t>(timestamp.count()), node.UID(),
                     static_cast<int>(status) });
  }

  void flush() override
  {}

  std::vector<Row> rows;
};

struct Scenario
{
  std::string name;
  std::string xml;
  std::function<void(BT::Tree&)> drive;
};

std::string wrap(const std::string& body)
{
  return "<root BTCPP_format=\"4\" main_tree_to_execute=\"Main\">\n" + body + "</root>\n";
}

std::string wide(int children)
{
  std::ostringstream s;
  s << "<BehaviorTree ID=\"Main\">\n<Sequence name=\"many\">\n";
  for(int i = 0; i < children; ++i)
  {
    s << "<AlwaysSuccess name=\"leaf" << i << "\"/>\n";
  }
  s << "</Sequence>\n</BehaviorTree>\n";
  return wrap(s.str());
}

void ticks(BT::Tree& tree, int n)
{
  for(int i = 0; i < n; ++i)
  {
    tree.tickOnce();
  }
}

std::vector<Scenario> scenarios()
{
  return {
    // Nothing ticks: a header and no records.
    { "quiet",
      wrap("<BehaviorTree ID=\"Main\"><AlwaysSuccess/></BehaviorTree>\n"),
      [](BT::Tree&) {} },
    // RUNNING, SUCCESS, and the IDLE resets after a tick completes.
    { "sequence",
      wrap("<BehaviorTree ID=\"Main\">\n<Sequence name=\"steps\">\n"
           "<AlwaysSuccess/>\n<AlwaysSuccess name=\"second\"/>\n"
           "</Sequence>\n</BehaviorTree>\n"),
      [](BT::Tree& t) { ticks(t, 2); } },
    // FAILURE, then IDLE after FAILURE.
    { "fallback",
      wrap("<BehaviorTree ID=\"Main\">\n<Fallback>\n"
           "<AlwaysFailure/>\n<AlwaysSuccess/>\n"
           "</Fallback>\n</BehaviorTree>\n"),
      [](BT::Tree& t) { ticks(t, 1); } },
    // RUNNING across ticks, then halted: RUNNING to IDLE.
    { "halted",
      wrap("<BehaviorTree ID=\"Main\">\n<Sequence>\n"
           "<Pending ticks=\"5\"/>\n<AlwaysSuccess/>\n"
           "</Sequence>\n</BehaviorTree>\n"),
      [](BT::Tree& t) {
        ticks(t, 2);
        t.haltTree();
      } },
    // RUNNING across ticks, then SUCCESS.
    { "running",
      wrap("<BehaviorTree ID=\"Main\">\n<Sequence>\n"
           "<Pending ticks=\"3\"/>\n<AlwaysFailure/>\n"
           "</Sequence>\n</BehaviorTree>\n"),
      [](BT::Tree& t) { ticks(t, 4); } },
    // A skipped node: it keeps its IDLE status, so it logs nothing.
    { "skipped",
      wrap("<BehaviorTree ID=\"Main\">\n<Sequence>\n"
           "<AlwaysFailure _skipIf=\"true\"/>\n<AlwaysSuccess/>\n"
           "</Sequence>\n</BehaviorTree>\n"),
      [](BT::Tree& t) { ticks(t, 2); } },
    // A subtree: nested _fullpath values in the XML.
    { "subtree",
      wrap("<BehaviorTree ID=\"Main\">\n<Sequence>\n"
           "<SubTree ID=\"Inner\" name=\"inner\"/>\n<AlwaysSuccess/>\n"
           "</Sequence>\n</BehaviorTree>\n"
           "<BehaviorTree ID=\"Inner\">\n<Fallback>\n"
           "<AlwaysFailure/>\n<AlwaysSuccess/>\n"
           "</Fallback>\n</BehaviorTree>\n"),
      [](BT::Tree& t) { ticks(t, 1); } },
    // Many records over many ticks.
    { "repeat",
      wrap("<BehaviorTree ID=\"Main\">\n<Repeat num_cycles=\"20\">\n"
           "<Sequence>\n<AlwaysSuccess/>\n<Pending ticks=\"1\"/>\n</Sequence>\n"
           "</Repeat>\n</BehaviorTree>\n"),
      [](BT::Tree& t) { ticks(t, 30); } },
    // Node uids above 255, so both bytes of the uid field are used.
    { "wide", wide(300), [](BT::Tree& t) { ticks(t, 1); } },
  };
}

void write_text(const fs::path& path, const std::string& text)
{
  std::ofstream out(path, std::ios::binary);
  out << text;
  if(!out)
  {
    throw std::runtime_error("cannot write " + path.string());
  }
}

void run(const fs::path& dir, BT::BehaviorTreeFactory& factory, const Scenario& s)
{
  BT::Tree tree = factory.createTreeFromText(s.xml);
  const fs::path btlog = dir / (s.name + ".btlog");
  const std::string xml = BT::WriteTreeToXML(tree, true, true);
  std::vector<Row> rows;
  {
    BT::FileLogger2 log(tree, btlog);
    Recorder rec(tree.rootNode());
    s.drive(tree);
    rows = rec.rows;
    // The logger's destructor stops its writer thread without draining the
    // queue (BT 4.9.0 src/loggers/bt_file_logger_v2.cpp:97-99,135-161), so
    // records still queued would be lost. Wait until the writer has flushed
    // every record before leaving the scope.
    const std::uintmax_t full = 31 + xml.size() + 9 * rows.size();
    const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(10);
    while(fs::file_size(btlog) != full)
    {
      if(std::chrono::steady_clock::now() > deadline)
      {
        throw std::runtime_error(s.name + ": the logger did not write every record");
      }
      std::this_thread::sleep_for(std::chrono::milliseconds(5));
    }
  }
  write_text(dir / (s.name + ".xml"), xml);
  std::ostringstream lines;
  for(const Row& r : rows)
  {
    lines << r.ns << ' ' << r.uid << ' ' << r.status << '\n';
  }
  write_text(dir / (s.name + ".transitions"), lines.str());
}

}  // namespace

int main(int argc, char** argv)
{
  if(argc != 2)
  {
    std::cerr << "usage: btlog_harness <output-directory>\n";
    return 2;
  }
  try
  {
    const fs::path dir(argv[1]);
    fs::create_directories(dir);
    BT::BehaviorTreeFactory factory;
    factory.registerNodeType<Pending>("Pending");
    for(const Scenario& s : scenarios())
    {
      run(dir, factory, s);
      std::cout << s.name << '\n';
    }
  }
  catch(const std::exception& e)
  {
    std::cerr << "btlog_harness: " << e.what() << '\n';
    return 1;
  }
  return 0;
}
