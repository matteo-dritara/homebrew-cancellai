class Cancellai < Formula
  desc "Safely reclaim disk space from old Codex CLI and Claude Code session data"
  homepage "https://github.com/matteo-dritara/homebrew-cancellai"
  url "https://github.com/matteo-dritara/homebrew-cancellai/archive/refs/tags/v1.0.1.tar.gz"
  sha256 "b2fdb252970ad33c9c44624836253f1f244e4a2590060e17d946cda39f88392f"
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
