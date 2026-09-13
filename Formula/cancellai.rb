class Cancellai < Formula
  desc "Safely reclaim disk space from old Codex CLI and Claude Code session data"
  homepage "https://github.com/matteo-dritara/homebrew-cancellai"
  url "https://github.com/matteo-dritara/homebrew-cancellai/archive/refs/tags/v1.13.2.tar.gz"
  sha256 "76e1794ef9ce842c867c20787682f293090dc985f8cfdad70bd32863ebaa73c9"
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
