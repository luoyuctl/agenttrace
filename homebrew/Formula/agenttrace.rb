# Homebrew Formula for agenttrace
# Usage: brew install luoyuctl/tap/agenttrace
# Formula source: https://github.com/luoyuctl/homebrew-tap/blob/main/Formula/agenttrace.rb

class Agenttrace < Formula
  desc "AI Agent Session Analyzer — find hanging, token waste & quality regressions"
  homepage "https://github.com/luoyuctl/agenttrace"
  version "0.0.4"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/luoyuctl/agenttrace/releases/download/v0.0.4/agenttrace-darwin-arm64"
      sha256 "f597822dd78e1290755ab7302e5dab5d72fc6c2a424d884a9d43b32056e6a7f1"
    else
      url "https://github.com/luoyuctl/agenttrace/releases/download/v0.0.4/agenttrace-darwin-amd64"
      sha256 "9073e6e7af5ac827dcfd8f3e4e835fed498c1db79a9fb4a4f109310bc00eb46e"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/luoyuctl/agenttrace/releases/download/v0.0.4/agenttrace-linux-arm64"
      sha256 "8fd18350f2e88b9a245b9b19396c3949eb0bfe1dc03edde3d0e0ace20e523273"
    else
      url "https://github.com/luoyuctl/agenttrace/releases/download/v0.0.4/agenttrace-linux-amd64"
      sha256 "760e05e6ba8053e22dca43b76ebdd6fcf24896d4c691a28aac9b886d6fbc7ab3"
    end
  end

  def install
    bin.install Dir["agenttrace-*"].first => "agenttrace"
  end

  test do
    assert_match "agenttrace v0.0", shell_output("#{bin}/agenttrace --list-models 2>&1")
  end
end
