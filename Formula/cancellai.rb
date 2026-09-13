class Cancellai < Formula
  desc "Safely reclaim disk space from old Codex CLI and Claude Code session data"
  homepage "https://github.com/matteo-dritara/homebrew-cancellai"
  url "https://github.com/matteo-dritara/homebrew-cancellai/archive/refs/tags/v1.13.3.tar.gz"
  sha256 "6fb174cd13ff6a1db4d7cd5384486bf4fe1c49354bce841bfa8439f0d79767ee"
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
