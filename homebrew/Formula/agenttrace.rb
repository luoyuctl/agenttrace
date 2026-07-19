class Agenttrace < Formula
  desc "TUI observability for AI coding agent sessions, cost, latency, and anomalies"
  homepage "https://github.com/luoyuctl/agenttrace"
  url "https://github.com/luoyuctl/agenttrace.git", branch: "master"
  version "0.7.0"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args(path: "crates/agenttrace-cli")
  end

  test do
    assert_match "agenttrace v0.7.0", shell_output("#{bin}/agenttrace --version")
  end
end
