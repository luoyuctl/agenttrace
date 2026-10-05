class Agenttrace < Formula
  desc "TUI observability for AI coding agent sessions, cost, latency, and anomalies"
  homepage "https://github.com/luoyuctl/agenttrace"
  head "https://github.com/luoyuctl/agenttrace.git", branch: "master"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args(path: "crates/agenttrace-cli")
  end

  test do
    assert_match "agenttrace v", shell_output("#{bin}/agenttrace --version")
  end
end
