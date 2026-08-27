class Cancellai < Formula
  desc "Safely reclaim disk space from old Codex CLI and Claude Code session data"
  homepage "https://github.com/matteo-dritara/homebrew-cancellai"
  url "https://github.com/matteo-dritara/homebrew-cancellai/archive/refs/tags/v1.0.2.tar.gz"
  sha256 "ba6c18d985ab0722b8b22b13d032003b46a53ba1480ef70fd53fbc07f50efc77"
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
