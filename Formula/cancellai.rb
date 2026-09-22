class Cancellai < Formula
  desc "Safely reclaim disk space from old Codex CLI and Claude Code session data"
  homepage "https://github.com/matteo-dritara/homebrew-cancellai"
  url "https://github.com/matteo-dritara/homebrew-cancellai/archive/refs/tags/v1.16.0.tar.gz"
  sha256 "f7a20b4405253c3cd0021b975d4eb77633f7ce7760f5afe38637c32d147bbcac"
  license "MIT"

  depends_on "python3"

  def install
    bin.install "cancellai.py" => "cancellai"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/cancellai --version")
    assert_match "cancellAI status", shell_output("#{bin}/cancellai status")
  end
end
