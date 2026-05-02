class Agenttrace < Formula
  desc "TUI observability for AI coding agent sessions, cost, latency, and anomalies"
  homepage "https://github.com/luoyuctl/agenttrace"
  url "https://github.com/luoyuctl/agenttrace.git", branch: "master"
  version "0.3.34"
  license "MIT"

  depends_on "go" => :build

  def install
    system "go", "build", "-ldflags=-s -w", "-o", bin/"agenttrace", "./cmd/agenttrace"
  end

  test do
    assert_match "agenttrace v0.3.34", shell_output("#{bin}/agenttrace --version")
  end
end
