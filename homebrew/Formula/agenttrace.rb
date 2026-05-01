# Homebrew Formula for agenttrace
# Usage: brew install luoyuctl/tap/agenttrace
# Formula source: https://github.com/luoyuctl/homebrew-tap/blob/main/Formula/agenttrace.rb

class Agenttrace < Formula
  desc "AI Agent Session Analyzer — find hanging, token waste & quality regressions"
  homepage "https://github.com/luoyuctl/agenttrace"
  version "4.0.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/luoyuctl/agenttrace/releases/download/v4.0.0/agenttrace-darwin-arm64"
      sha256 "PLACEHOLDER_DARWIN_ARM64_SHA256"
    else
      url "https://github.com/luoyuctl/agenttrace/releases/download/v4.0.0/agenttrace-darwin-amd64"
      sha256 "PLACEHOLDER_DARWIN_AMD64_SHA256"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/luoyuctl/agenttrace/releases/download/v4.0.0/agenttrace-linux-arm64"
      sha256 "PLACEHOLDER_LINUX_ARM64_SHA256"
    else
      url "https://github.com/luoyuctl/agenttrace/releases/download/v4.0.0/agenttrace-linux-amd64"
      sha256 "PLACEHOLDER_LINUX_AMD64_SHA256"
    end
  end

  def install
    bin.install Dir["agenttrace-*"].first => "agenttrace"
  end

  test do
    assert_match "agenttrace v4", shell_output("#{bin}/agenttrace --list-models 2>&1")
  end
end
